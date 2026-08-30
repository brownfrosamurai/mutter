//! Session orchestrator: the actual toggle-hotkey core loop, wiring hotkey
//! -> mic capture -> the Escape/cancel state machine -> the transcription
//! engine -> grammar cleanup -> text injection -> history
//! (docs/mutter-project-plan.md Section 3).
//!
//! Runs as a single-threaded actor over an mpsc channel, so the
//! listening/transcribing/cancel-pending state machine never has to reason
//! about concurrent mutation — hotkey presses and Escape presses are just
//! messages fed into one sequential loop. Segment transcription runs on a
//! separate sequential worker task (see `segment_worker`) so a slow
//! transcription never blocks new capture from starting, while still
//! guaranteeing segments are transcribed and inserted in the order they
//! were spoken (a single FIFO worker, not concurrent per-segment tasks).
//!
//! **`cpal::Stream` is not `Send`**, so `MicCapture` cannot live inside this
//! actor's async task (its state would have to cross `.await` points).
//! `spawn_capture_actor` isolates it on its own dedicated, plain OS thread
//! instead, bridged to the async world by channels — the orchestrator only
//! ever sees `Vec<f32>` buffers and `CaptureEvent`s, never the stream
//! itself.
//!
//! **Auto-transcribe-and-continue** (Section 3): the primary named use case
//! — dictating a spec or bug report to an AI coding agent — routinely runs
//! past the 120s mic buffer cap. Hitting the cap does not truncate speech:
//! the buffered segment is handed to the worker for transcription +
//! insertion, a new segment starts capturing immediately, and the
//! toggle-hotkey session continues uninterrupted. The pill briefly flickers
//! to "transcribing" and back to "listening" at the handoff.
//!
//! **Re-entrancy** (Section 3): a toggle press is ignored while a prior
//! recording is still transcribing — implemented via an explicit
//! `Phase::Transcribing` that the toggle-hotkey match arm treats as a no-op,
//! not by leaving a press queued to fire later.
//!
//! **Two documented, deliberate narrowings of the plan's literal Section 7
//! text:**
//! - *Cancel and capture*: Section 7 leaves "pause vs. continue buffering
//!   during the countdown" as a Phase 2 decision, noting pausing is
//!   simpler. This takes the *even* simpler path of not touching the
//!   capture stream at all during `CancelPending` — it keeps buffering in
//!   the background. `Resumed` is then trivially correct ("resumes exactly
//!   where it left off") since capture was never interrupted; the only
//!   observable difference from a true pause is that audio spoken during
//!   the countdown itself is retained on resume — a reasonable superset,
//!   not a bug.
//! - *Cancel scope*: Section 7 says "Recording or transcribing → Escape".
//!   This implementation only offers cancel during `Listening`, not
//!   `Transcribing` — by the time capture has stopped and audio has been
//!   handed to the engine, canceling would need whisper.cpp abort-callback
//!   plumbing to interrupt in-flight inference. Not worth that complexity
//!   for a multi-second window; Escape during `Transcribing` is a no-op.

use std::future::pending;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::{mpsc, oneshot};

use crate::cancel::CancelStateMachine;
use crate::capture::mic::{CaptureError, MicCapture};
use crate::engine::{TextProcessor, TranscriptionEngine};
use crate::history::{HistoryEntry, HistoryStore};
use crate::hotkey::HotkeyMode;
use crate::injection;

const CANCEL_COUNTDOWN_SECS: u8 = 3;
const PILL_WINDOW: &str = "pill";
const ESCAPE_SHORTCUT: &str = "Escape";
const TARGET_SAMPLE_RATE: f64 = 16_000.0;
/// How often the capture-owner thread checks `is_at_cap()` between commands.
const CAP_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug)]
enum SessionCommand {
    HotkeyPressed(HotkeyMode),
    EscapePressed,
}

#[derive(Clone)]
pub struct SessionHandle {
    tx: mpsc::UnboundedSender<SessionCommand>,
}

impl SessionHandle {
    pub fn hotkey_pressed(&self, mode: HotkeyMode) {
        let _ = self.tx.send(SessionCommand::HotkeyPressed(mode));
    }

    /// Manual equivalent of pressing the global Escape hotkey — used by the
    /// pill's cancel button (`#pill-cancel`), since a webview button click
    /// can't itself register as a global-shortcut key-press.
    pub fn escape_pressed(&self) {
        let _ = self.tx.send(SessionCommand::EscapePressed);
    }
}

struct SegmentJob {
    audio: Vec<f32>,
    /// Only populated for the final segment of a session (the one the
    /// user's stop press produced) — fires once that specific segment has
    /// been transcribed, cleaned, inserted, and written to history, so the
    /// main loop knows when it's safe to show "done" and hide the pill.
    /// Auto-continue segments are fire-and-forget (`None`).
    done_tx: Option<oneshot::Sender<()>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Listening,
    Transcribing,
    CancelPending,
}

// --- Capture actor: bridges the non-Send cpal::Stream to the async world ---

enum CaptureCommand {
    Start(oneshot::Sender<Result<(), CaptureError>>),
    Stop(oneshot::Sender<Vec<f32>>),
}

/// Sent from the capture-owner thread when the 120s buffer cap is hit while
/// actively capturing.
struct CaptureAtCap;

struct CaptureHandle {
    tx: std::sync::mpsc::Sender<CaptureCommand>,
}

impl CaptureHandle {
    async fn start(&self) -> Result<(), CaptureError> {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(CaptureCommand::Start(tx)).is_err() {
            return Err(CaptureError::NoInputDevice);
        }
        rx.await.unwrap_or(Err(CaptureError::NoInputDevice))
    }

    async fn stop(&self) -> Vec<f32> {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(CaptureCommand::Stop(tx)).is_err() {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }
}

/// Owns `MicCapture` (and therefore its non-`Send` `cpal::Stream`) on a
/// dedicated plain OS thread for its entire lifetime — it never crosses an
/// `.await` point, so it never needs to be `Send`.
fn spawn_capture_actor(at_cap_tx: mpsc::UnboundedSender<CaptureAtCap>) -> CaptureHandle {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<CaptureCommand>();

    std::thread::spawn(move || {
        let mut capture = MicCapture::new();
        let mut running = false;

        loop {
            match cmd_rx.recv_timeout(CAP_POLL_INTERVAL) {
                Ok(CaptureCommand::Start(respond)) => {
                    let result = capture.start();
                    running = result.is_ok();
                    let _ = respond.send(result);
                }
                Ok(CaptureCommand::Stop(respond)) => {
                    let audio = capture.stop();
                    running = false;
                    let _ = respond.send(audio);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if running && capture.is_at_cap() {
                        // Don't re-notify every 250ms — the session decides
                        // when to actually stop/restart in response, which
                        // naturally clears `running` via the next Start.
                        running = false;
                        if at_cap_tx.send(CaptureAtCap).is_err() {
                            break; // orchestrator is gone
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    CaptureHandle { tx: cmd_tx }
}

// --- Orchestrator ---

pub fn spawn<R: Runtime>(
    app: AppHandle<R>,
    engine: Arc<dyn TranscriptionEngine>,
    grammar: Arc<dyn TextProcessor>,
    history: Arc<HistoryStore>,
    engine_name: &'static str,
) -> SessionHandle {
    let (segment_tx, segment_rx) = mpsc::unbounded_channel::<SegmentJob>();
    let (tx, rx) = mpsc::unbounded_channel::<SessionCommand>();

    // `tauri::async_runtime::spawn`, not `tokio::spawn` — this fn is called
    // synchronously from `App::setup`, before Tauri's runtime is ambiently
    // "entered" the way a bare `tokio::spawn` requires ("there is no
    // reactor running" otherwise). Tauri's wrapper spawns onto the runtime
    // it manages internally regardless of the calling context.
    tauri::async_runtime::spawn(segment_worker(
        engine,
        grammar,
        history,
        engine_name,
        segment_rx,
    ));
    tauri::async_runtime::spawn(run(app, tx.clone(), rx, segment_tx));

    SessionHandle { tx }
}

async fn run<R: Runtime>(
    app: AppHandle<R>,
    tx: mpsc::UnboundedSender<SessionCommand>,
    mut rx: mpsc::UnboundedReceiver<SessionCommand>,
    segment_tx: mpsc::UnboundedSender<SegmentJob>,
) {
    let mut phase = Phase::Idle;
    let mut cancel_fsm = CancelStateMachine::new();
    let mut elapsed_secs: u64 = 0;
    let mut cancel_remaining: u8 = 0;
    let mut final_done_rx: Option<oneshot::Receiver<()>> = None;

    let (at_cap_tx, mut at_cap_rx) = mpsc::unbounded_channel::<CaptureAtCap>();
    let capture = spawn_capture_actor(at_cap_tx);

    loop {
        let ticking = matches!(phase, Phase::Listening | Phase::CancelPending);
        let tick = async {
            if ticking {
                tokio::time::sleep(Duration::from_secs(1)).await;
            } else {
                pending::<()>().await;
            }
        };
        let done = async {
            match final_done_rx.as_mut() {
                Some(rx) => {
                    let _ = rx.await;
                }
                None => pending::<()>().await,
            }
        };

        tokio::select! {
            cmd = rx.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    SessionCommand::HotkeyPressed(HotkeyMode::SystemAudio) => {
                        tracing::warn!(
                            "system-audio hotkey pressed — capture not implemented until Phase 4"
                        );
                    }
                    SessionCommand::HotkeyPressed(HotkeyMode::MicDictation) => match phase {
                        Phase::Idle => {
                            tracing::info!("mic-dictation hotkey pressed — starting");
                            if start_listening(&app, &capture).await {
                                phase = Phase::Listening;
                                elapsed_secs = 0;
                                register_escape(&app, tx.clone());
                            }
                        }
                        Phase::Listening => {
                            tracing::info!("mic-dictation hotkey pressed — stopping");
                            unregister_escape(&app);
                            let audio = capture.stop().await;
                            cancel_fsm = CancelStateMachine::new();
                            phase = Phase::Transcribing;
                            emit_state(&app, "transcribing");
                            let (done_tx, done_rx) = oneshot::channel();
                            let _ = segment_tx.send(SegmentJob { audio, done_tx: Some(done_tx) });
                            final_done_rx = Some(done_rx);
                        }
                        Phase::Transcribing | Phase::CancelPending => {
                            tracing::debug!("mic-dictation hotkey ignored (re-entrancy guard)");
                        }
                    },
                    SessionCommand::EscapePressed => match phase {
                        Phase::Listening => {
                            cancel_fsm.on_escape(); // -> CancelPending
                            phase = Phase::CancelPending;
                            cancel_remaining = CANCEL_COUNTDOWN_SECS;
                            emit_countdown(&app, cancel_remaining);
                            emit_state(&app, "canceling");
                        }
                        Phase::CancelPending => {
                            cancel_fsm.on_escape(); // -> Resumed
                            phase = Phase::Listening;
                            emit_state(&app, "listening");
                        }
                        Phase::Idle | Phase::Transcribing => {
                            // See module docs: cancel is only offered while
                            // actively listening.
                        }
                    },
                }
            }
            at_cap = at_cap_rx.recv() => {
                if at_cap.is_some() && phase == Phase::Listening {
                    let audio = capture.stop().await;
                    emit_state(&app, "transcribing");
                    let _ = segment_tx.send(SegmentJob { audio, done_tx: None });
                    if start_listening(&app, &capture).await {
                        emit_state(&app, "listening");
                    } else {
                        phase = Phase::Idle;
                        unregister_escape(&app);
                    }
                }
            }
            _ = tick => match phase {
                Phase::Listening => {
                    elapsed_secs += 1;
                    emit_elapsed(&app, elapsed_secs);
                }
                Phase::CancelPending => {
                    if cancel_remaining <= 1 {
                        cancel_fsm.on_countdown_expired(); // -> Discarded
                        let _ = capture.stop().await; // discard the buffer
                        unregister_escape(&app);
                        phase = Phase::Idle;
                        hide_pill(&app);
                    } else {
                        cancel_remaining -= 1;
                        emit_countdown(&app, cancel_remaining);
                    }
                }
                Phase::Idle | Phase::Transcribing => {}
            },
            _ = done => {
                if phase == Phase::Transcribing {
                    final_done_rx = None;
                    phase = Phase::Idle;
                    emit_state(&app, "done");
                    hide_pill_after(&app, Duration::from_millis(600)).await;
                }
            }
        }
    }
}

/// Transcribe -> grammar cleanup -> insert -> history, strictly in
/// submission order (a single sequential worker, not one task per segment)
/// so multi-segment sessions (Section 3's auto-continue) never insert text
/// out of the order it was spoken.
async fn segment_worker(
    engine: Arc<dyn TranscriptionEngine>,
    grammar: Arc<dyn TextProcessor>,
    history: Arc<HistoryStore>,
    engine_name: &'static str,
    mut rx: mpsc::UnboundedReceiver<SegmentJob>,
) {
    while let Some(job) = rx.recv().await {
        let duration_secs = job.audio.len() as f64 / TARGET_SAMPLE_RATE;

        let (text, language) = match engine.transcribe(&job.audio).await {
            Ok(t) if t.text.trim().is_empty() => (String::new(), t.language),
            Ok(t) => {
                let cleaned = grammar
                    .process(&t.text, &t.language)
                    .await
                    .unwrap_or(t.text);
                (cleaned, t.language)
            }
            Err(e) => {
                // Section 3: a visible marker, not a silently dropped
                // segment — the user sees exactly where content is missing.
                tracing::error!(error = %e, "segment transcription failed");
                ("[transcription failed]".to_string(), "unknown".to_string())
            }
        };

        if !text.is_empty() {
            let text_for_insert = text.clone();
            match tokio::task::spawn_blocking(move || injection::insert_at_cursor(&text_for_insert))
                .await
            {
                Ok(Ok(_method)) => {}
                Ok(Err(e)) => tracing::error!(error = %e, "text injection failed"),
                Err(e) => tracing::error!(error = %e, "insertion task panicked"),
            }

            let entry = HistoryEntry {
                timestamp: unix_now_secs(),
                duration_secs,
                text,
                language,
                engine: engine_name.to_string(),
            };
            if let Err(e) = history.insert(&entry) {
                tracing::error!(error = %e, "failed to write history entry");
            }
        }

        if let Some(done_tx) = job.done_tx {
            let _ = done_tx.send(());
        }
    }
}

fn unix_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn start_listening<R: Runtime>(app: &AppHandle<R>, capture: &CaptureHandle) -> bool {
    match capture.start().await {
        Ok(()) => {
            show_pill(app);
            true
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to start mic capture");
            false
        }
    }
}

fn show_pill<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window(PILL_WINDOW) {
        let _ = win.show();
    }
    emit_state(app, "listening");
}

fn hide_pill<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window(PILL_WINDOW) {
        let _ = win.hide();
    }
}

async fn hide_pill_after<R: Runtime>(app: &AppHandle<R>, delay: Duration) {
    tokio::time::sleep(delay).await;
    hide_pill(app);
}

fn register_escape<R: Runtime>(app: &AppHandle<R>, tx: mpsc::UnboundedSender<SessionCommand>) {
    let shortcut: Shortcut = match ESCAPE_SHORTCUT.parse() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "invalid escape shortcut spec");
            return;
        }
    };
    if let Err(e) = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let _ = tx.send(SessionCommand::EscapePressed);
            }
        })
    {
        tracing::error!(error = %e, "failed to register escape shortcut");
    }
}

fn unregister_escape<R: Runtime>(app: &AppHandle<R>) {
    if let Ok(shortcut) = ESCAPE_SHORTCUT.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(shortcut);
    }
}

#[derive(Serialize, Clone)]
struct PillState<'a> {
    state: &'a str,
}

#[derive(Serialize, Clone)]
struct Elapsed {
    seconds: u64,
}

#[derive(Serialize, Clone)]
struct CancelCountdown {
    #[serde(rename = "secondsRemaining")]
    seconds_remaining: u8,
}

fn emit_state<R: Runtime>(app: &AppHandle<R>, state: &str) {
    let _ = app.emit_to(PILL_WINDOW, "mutter://pill-state", PillState { state });
}

fn emit_elapsed<R: Runtime>(app: &AppHandle<R>, seconds: u64) {
    let _ = app.emit_to(PILL_WINDOW, "mutter://elapsed-seconds", Elapsed { seconds });
}

fn emit_countdown<R: Runtime>(app: &AppHandle<R>, seconds_remaining: u8) {
    let _ = app.emit_to(
        PILL_WINDOW,
        "mutter://cancel-countdown",
        CancelCountdown { seconds_remaining },
    );
}
