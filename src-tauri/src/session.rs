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
//! toggle-hotkey session continues uninterrupted. The pill never changes
//! state at this handoff (2026-08-30: previously flickered to
//! "transcribing" and back — removed along with the pill's other
//! processing-state visuals, see "Pill has no processing-state visuals"
//! below).
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
//!   not a bug. Because capture keeps running during `CancelPending`, it
//!   can still hit the 120s cap mid-countdown — the main loop's `at_cap`
//!   branch handles that explicitly (auto-continue, silently, no pill-state
//!   flicker over the countdown UI) rather than only reacting to it during
//!   `Listening`; see that match arm's comment for why skipping it there
//!   would be a silent data-loss bug, not just a missed UI update.
//! - *Cancel scope*: Section 7 says "Recording or transcribing → Escape".
//!   This implementation only offers cancel during `Listening`, not
//!   `Transcribing` — by the time capture has stopped and audio has been
//!   handed to the engine, canceling would need whisper.cpp abort-callback
//!   plumbing to interrupt in-flight inference. Not worth that complexity
//!   for a multi-second window; Escape during `Transcribing` is a no-op.
//!
//! **Phase 4: system-audio capture** shares the same `segment_worker` (and
//! therefore the same engine/grammar/injection/history pipeline) as mic
//! dictation, per Section 9, but is its own toggle (`system_audio_active`,
//! a plain bool) rather than a variant of `Phase` — the two capture sources
//! are mutually exclusive (starting one while the other is active is
//! ignored, logged, not queued) but otherwise independent. It reuses
//! `Phase::Transcribing` for its own post-stop "wait for the final segment,
//! then hide the pill" bookkeeping rather than inventing a parallel state
//! machine for what's the same wait. System-audio does not get its own
//! Escape-cancel flow — Section 7's cancel countdown was specified for
//! "recording" generically without calling out system-audio, and adding a
//! second cancel path without the plan naming one wasn't judged worth it;
//! stopping via the toggle hotkey is the only way to end a system-audio
//! capture in this version.
//!
//! **Pill has no processing-state visuals — user-directed, 2026-08-30.**
//! The pill only ever shows two states now: `listening` (recording) and
//! `done`. There is no visible `loading` or `transcribing` state: the pill
//! window is hidden the instant recording stops (`hide_pill`, right after
//! `warm_up_model_if_needed`) and only reappears once `segment_worker`
//! reports the final segment done (`reveal_pill` + `emit_state(..,
//! "done")`), then auto-hides again after a beat. The one deliberate
//! exception: if this is the process's first-ever transcription,
//! `warm_up_model_if_needed` still emits `loading` onto the *still-visible*
//! pill before the hide happens — a fresh install's real model-load wait
//! can be long enough that hiding through it would look like a hang
//! (Section 6, Performance Issue 8's original reasoning still applies to
//! that one case, even though "no processing visuals" applies everywhere
//! else). `Phase::CancelPending` (the Escape countdown) is unaffected —
//! it's a decision the user is actively making, not background processing,
//! so it keeps its own distinct, visible state.

use std::future::pending;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::{mpsc, oneshot};

use crate::cancel::CancelStateMachine;
use crate::capture::mic::{CaptureError, MicCapture};
use crate::capture::system_audio::{SystemAudioCapture, SystemAudioCaptureError};
use crate::engine::{TextProcessor, TranscriptionEngine};
use crate::history::{HistoryEntry, HistoryStore};
use crate::hotkey::HotkeyMode;
use crate::injection;
use crate::vibrancy;

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
    /// Wall-clock hotkey-press -> first-audio-frame latency for *this*
    /// segment, in milliseconds — `None` for continuation segments (the
    /// 120s auto-continue cap restarts capture without a corresponding user
    /// press, so there's no "press" instant to diff against) and for the
    /// system-audio path (a different capture mechanism, not instrumented
    /// here). See `capture::mic::MicCapture::first_frame_at` and this
    /// module's `run()` for where the two instants actually get diffed.
    recording_latency_ms: Option<f64>,
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
    /// The response carries the audio buffer plus `first_frame_at()` read
    /// *before* `capture.stop()` clears the capture's internal state (a
    /// `MicCapture` after `stop()` has no state to read it from) — see
    /// `CaptureHandle::stop()` for how the caller turns this into a
    /// recording-latency measurement.
    Stop(oneshot::Sender<(Vec<f32>, Option<std::time::Instant>)>),
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

    async fn stop(&self) -> (Vec<f32>, Option<std::time::Instant>) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(CaptureCommand::Stop(tx)).is_err() {
            return (Vec::new(), None);
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
                    // Read before stop() — stop() clears MicCapture's
                    // internal state, after which first_frame_at() can only
                    // ever return None.
                    let first_frame_at = capture.first_frame_at();
                    let audio = capture.stop();
                    running = false;
                    let _ = respond.send((audio, first_frame_at));
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

// --- System-audio actor: same shape as the mic capture actor above.
// `SystemAudioCapture` is technically `Send` (raw pointers only, no
// `cpal::Stream`-style internals), but `start()`/`stop()` still *block* the
// calling thread — potentially for however long a Screen Recording
// permission dialog takes to resolve — so they still don't belong on a
// tokio worker thread. A second small actor, not a generic one shared with
// mic's, since the two capture types have different concrete error types
// and there are only ever going to be two of these (Section 9's "shares
// the downstream pipeline" is about the segment_worker below, not this).

enum SystemAudioCommand {
    Start(oneshot::Sender<Result<(), SystemAudioCaptureError>>),
    Stop(oneshot::Sender<Vec<f32>>),
}

struct SystemAudioAtCap;

struct SystemAudioHandle {
    tx: std::sync::mpsc::Sender<SystemAudioCommand>,
}

impl SystemAudioHandle {
    async fn start(&self) -> Result<(), SystemAudioCaptureError> {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(SystemAudioCommand::Start(tx)).is_err() {
            return Err(SystemAudioCaptureError::StartFailed(
                "system-audio actor thread is gone".into(),
            ));
        }
        rx.await.unwrap_or(Err(SystemAudioCaptureError::StartFailed(
            "system-audio actor thread dropped the response channel".into(),
        )))
    }

    async fn stop(&self) -> Vec<f32> {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(SystemAudioCommand::Stop(tx)).is_err() {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }
}

fn spawn_system_audio_actor(
    at_cap_tx: mpsc::UnboundedSender<SystemAudioAtCap>,
) -> SystemAudioHandle {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<SystemAudioCommand>();

    std::thread::spawn(move || {
        let mut capture = SystemAudioCapture::new();
        let mut running = false;

        loop {
            match cmd_rx.recv_timeout(CAP_POLL_INTERVAL) {
                Ok(SystemAudioCommand::Start(respond)) => {
                    let result = capture.start();
                    running = result.is_ok();
                    let _ = respond.send(result);
                }
                Ok(SystemAudioCommand::Stop(respond)) => {
                    let audio = capture.stop();
                    running = false;
                    let _ = respond.send(audio);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if running && capture.is_at_cap() {
                        running = false;
                        if at_cap_tx.send(SystemAudioAtCap).is_err() {
                            break;
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    SystemAudioHandle { tx: cmd_tx }
}

// --- Orchestrator ---

pub fn spawn<R: Runtime>(
    app: AppHandle<R>,
    engine: Arc<dyn TranscriptionEngine>,
    grammar: Arc<dyn TextProcessor>,
    history: Arc<HistoryStore>,
    engine_name: &'static str,
    paste_automatically: Arc<AtomicBool>,
    restore_clipboard: Arc<AtomicBool>,
) -> SessionHandle {
    let (segment_tx, segment_rx) = mpsc::unbounded_channel::<SegmentJob>();
    let (tx, rx) = mpsc::unbounded_channel::<SessionCommand>();

    // `tauri::async_runtime::spawn`, not `tokio::spawn` — this fn is called
    // synchronously from `App::setup`, before Tauri's runtime is ambiently
    // "entered" the way a bare `tokio::spawn` requires ("there is no
    // reactor running" otherwise). Tauri's wrapper spawns onto the runtime
    // it manages internally regardless of the calling context.
    tauri::async_runtime::spawn(run(app, engine.clone(), tx.clone(), rx, segment_tx));
    tauri::async_runtime::spawn(segment_worker(
        engine,
        grammar,
        history,
        engine_name,
        segment_rx,
        paste_automatically,
        restore_clipboard,
    ));

    SessionHandle { tx }
}

async fn run<R: Runtime>(
    app: AppHandle<R>,
    engine: Arc<dyn TranscriptionEngine>,
    tx: mpsc::UnboundedSender<SessionCommand>,
    mut rx: mpsc::UnboundedReceiver<SessionCommand>,
    segment_tx: mpsc::UnboundedSender<SegmentJob>,
) {
    let mut phase = Phase::Idle;
    let mut cancel_fsm = CancelStateMachine::new();
    let mut elapsed_secs: u64 = 0;
    let mut cancel_remaining: u8 = 0;
    let mut final_done_rx: Option<oneshot::Receiver<()>> = None;
    // Set only at a genuine hotkey-triggered start (never at an
    // auto-continue restart, which has no corresponding user press to
    // measure against) — `.take()`n at whichever `capture.stop().await`
    // call closes out that same segment, whether that's a genuine stop or
    // the segment getting cut short by the 120s cap. Backs "Recording
    // Latency"; see `SegmentJob::recording_latency_ms`.
    let mut segment_started_at: Option<std::time::Instant> = None;
    // Section 6, Performance Issue 8: only the very first hand-off to the
    // engine in the process lifetime should show "loading" instead of
    // "transcribing" — see `warm_up_model_if_needed`.
    let mut model_ready = false;

    let (at_cap_tx, mut at_cap_rx) = mpsc::unbounded_channel::<CaptureAtCap>();
    let capture = spawn_capture_actor(at_cap_tx);

    let (sa_at_cap_tx, mut sa_at_cap_rx) = mpsc::unbounded_channel::<SystemAudioAtCap>();
    let system_audio = spawn_system_audio_actor(sa_at_cap_tx);
    let mut system_audio_active = false;

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
                        if system_audio_active {
                            tracing::info!("system-audio hotkey pressed — stopping");
                            let audio = system_audio.stop().await;
                            system_audio_active = false;
                            phase = Phase::Transcribing; // reuses the mic Phase's Transcribing/done handling
                            warm_up_model_if_needed(&app, &engine, &mut model_ready, true).await;
                            hide_pill(&app);
                            let (done_tx, done_rx) = oneshot::channel();
                            let _ = segment_tx.send(SegmentJob {
                                audio,
                                recording_latency_ms: None, // system-audio path — not instrumented
                                done_tx: Some(done_tx),
                            });
                            final_done_rx = Some(done_rx);
                        } else if phase != Phase::Idle {
                            tracing::debug!(
                                "system-audio hotkey ignored — mic dictation is active"
                            );
                        } else {
                            tracing::info!("system-audio hotkey pressed — starting");
                            match system_audio.start().await {
                                Ok(()) => {
                                    system_audio_active = true;
                                    show_pill(&app);
                                }
                                Err(e) => tracing::error!(
                                    error = %e,
                                    "failed to start system-audio capture"
                                ),
                            }
                        }
                    }
                    SessionCommand::HotkeyPressed(HotkeyMode::MicDictation) if system_audio_active => {
                        tracing::debug!("mic-dictation hotkey ignored — system-audio capture is active");
                    }
                    SessionCommand::HotkeyPressed(HotkeyMode::MicDictation) => match phase {
                        Phase::Idle => {
                            tracing::info!("mic-dictation hotkey pressed — starting");
                            if start_listening(&app, &capture).await {
                                phase = Phase::Listening;
                                elapsed_secs = 0;
                                segment_started_at = Some(std::time::Instant::now());
                                register_escape(&app, tx.clone());
                            }
                        }
                        Phase::Listening => {
                            tracing::info!("mic-dictation hotkey pressed — stopping");
                            unregister_escape(&app);
                            let (audio, first_frame_at) = capture.stop().await;
                            let recording_latency_ms = recording_latency(&mut segment_started_at, first_frame_at);
                            cancel_fsm = CancelStateMachine::new();
                            phase = Phase::Transcribing;
                            warm_up_model_if_needed(&app, &engine, &mut model_ready, true).await;
                            hide_pill(&app);
                            let (done_tx, done_rx) = oneshot::channel();
                            let _ = segment_tx.send(SegmentJob {
                                audio,
                                recording_latency_ms,
                                done_tx: Some(done_tx),
                            });
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
                match phase {
                    // Auto-continue on the 120s cap never touches pill
                    // state at all — recording never conceptually stops
                    // from the user's point of view here, so the pill
                    // just keeps showing "listening" straight through the
                    // segment hop (same "stay invisible" principle the
                    // CancelPending+at_cap branch below already applies).
                    Phase::Listening if at_cap.is_some() => {
                        let (audio, first_frame_at) = capture.stop().await;
                        let recording_latency_ms = recording_latency(&mut segment_started_at, first_frame_at);
                        warm_up_model_if_needed(&app, &engine, &mut model_ready, true).await;
                        let _ = segment_tx.send(SegmentJob { audio, recording_latency_ms, done_tx: None });
                        // The new segment about to start is an auto-continue,
                        // not a user press — segment_started_at stays None
                        // (already cleared by recording_latency above) so
                        // *its* eventual stop reports no recording latency.
                        if start_listening(&app, &capture).await {
                            emit_state(&app, "listening");
                        } else {
                            phase = Phase::Idle;
                            unregister_escape(&app);
                        }
                    }
                    // The 120s cap can be hit while a cancel countdown is
                    // pending too — module docs: capture keeps buffering in
                    // the background during CancelPending, it's never
                    // paused. Without handling it here, capture/mic.rs's
                    // audio callback (which gates on its own `at_cap` flag)
                    // silently stops accepting new samples forever once it
                    // hits the cap, and this channel's single at-cap
                    // notification is only ever sent once per capture
                    // start/stop cycle — so if the branch above is the only
                    // consumer, a countdown that later resumes (second
                    // Escape -> Listening) looks like it's still recording
                    // while actually capturing nothing until the next
                    // toggle-stop, a silent data-loss bug. Auto-continue
                    // exactly like the Listening case, but without emitting
                    // pill-state changes — the cap-driven segment hop must
                    // stay invisible to the pending cancel/resume decision
                    // (no flicker to "transcribing"/"listening" over top of
                    // the countdown UI).
                    Phase::CancelPending if at_cap.is_some() => {
                        let (audio, first_frame_at) = capture.stop().await;
                        let recording_latency_ms = recording_latency(&mut segment_started_at, first_frame_at);
                        warm_up_model_if_needed(&app, &engine, &mut model_ready, false).await;
                        let _ = segment_tx.send(SegmentJob { audio, recording_latency_ms, done_tx: None });
                        if capture.start().await.is_err() {
                            tracing::error!(
                                "failed to restart mic capture after cap during cancel-pending"
                            );
                            cancel_fsm = CancelStateMachine::new();
                            phase = Phase::Idle;
                            unregister_escape(&app);
                            hide_pill(&app);
                        }
                    }
                    _ => {}
                }
            }
            sa_at_cap = sa_at_cap_rx.recv() => {
                // Same "never touches pill state" reasoning as the mic
                // dictation cap-hop above — system-audio's own auto-continue
                // is equally invisible to the user.
                if sa_at_cap.is_some() && system_audio_active {
                    let audio = system_audio.stop().await;
                    warm_up_model_if_needed(&app, &engine, &mut model_ready, true).await;
                    let _ = segment_tx.send(SegmentJob {
                        audio,
                        recording_latency_ms: None, // system-audio path — not instrumented
                        done_tx: None,
                    });
                    match system_audio.start().await {
                        Ok(()) => {
                            show_pill(&app);
                        }
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                "failed to restart system-audio capture after cap"
                            );
                            system_audio_active = false;
                        }
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
                        segment_started_at = None; // no segment survives to report a latency for
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
                    // The pill was hidden for the transcription gap (see
                    // the stop-recording branches above) — reveal it again
                    // for the brief "done" confirmation before the usual
                    // delayed hide.
                    reveal_pill(&app);
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
    paste_automatically: Arc<AtomicBool>,
    restore_clipboard: Arc<AtomicBool>,
) {
    while let Some(job) = rx.recv().await {
        let duration_secs = job.audio.len() as f64 / TARGET_SAMPLE_RATE;

        let inference_started_at = std::time::Instant::now();
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
        let inference_latency_ms = inference_started_at.elapsed().as_secs_f64() * 1000.0;

        if !text.is_empty() {
            // AppSettings::paste_automatically off -> clipboard-only mode:
            // skip insert_at_cursor's AX-insert/synthetic-paste path
            // entirely and just put the text on the clipboard for the
            // user to paste themselves (reuses copy_to_clipboard, the
            // exact same command the dashboard's History "copy" button
            // already calls — not new clipboard-handling code).
            if paste_automatically.load(Ordering::Relaxed) {
                let text_for_insert = text.clone();
                let restore = restore_clipboard.load(Ordering::Relaxed);
                match tokio::task::spawn_blocking(move || {
                    injection::insert_at_cursor(&text_for_insert, restore)
                })
                .await
                {
                    Ok(Ok(_method)) => {}
                    Ok(Err(e)) => tracing::error!(error = %e, "text injection failed"),
                    Err(e) => tracing::error!(error = %e, "insertion task panicked"),
                }
            } else if let Err(e) = injection::copy_to_clipboard(&text) {
                tracing::error!(error = %e, "clipboard-only copy failed");
            }

            let entry = HistoryEntry {
                timestamp: unix_now_secs(),
                duration_secs,
                text,
                language,
                engine: engine_name.to_string(),
                recording_latency_ms: job.recording_latency_ms,
                inference_latency_ms: Some(inference_latency_ms),
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

/// Consumes `segment_started_at` (a genuine hotkey-press instant, or `None`
/// for an auto-continue segment) and diffs it against `first_frame_at` (the
/// instant this same segment's audio callback first actually ran, or `None`
/// if it never got the chance to before being stopped). `.take()`s the
/// former unconditionally — whichever segment this call is closing out is
/// the one `segment_started_at` was tracking, so it must not leak into
/// whatever segment starts next.
fn recording_latency(
    segment_started_at: &mut Option<std::time::Instant>,
    first_frame_at: Option<std::time::Instant>,
) -> Option<f64> {
    segment_started_at
        .take()
        .zip(first_frame_at)
        .map(|(started, first)| first.saturating_duration_since(started).as_secs_f64() * 1000.0)
}

fn unix_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Pays the engine's one-time model-load cost (if any) before its first
/// real use in this process, showing a distinct "loading" pill state while
/// it happens so a slow first load (a ~500MB download, on a fresh install)
/// doesn't look indistinguishable from a hang under a generic "transcribing"
/// label. `visible: false` is used only for the cap-during-cancel-pending
/// hand-off, which deliberately never changes pill state (see that call
/// site) — the warm-up still has to happen exactly once, just silently.
async fn warm_up_model_if_needed<R: Runtime>(
    app: &AppHandle<R>,
    engine: &Arc<dyn TranscriptionEngine>,
    model_ready: &mut bool,
    visible: bool,
) {
    if *model_ready {
        return;
    }
    if visible {
        emit_state(app, "loading");
    }
    if let Err(e) = engine.ensure_ready().await {
        tracing::error!(error = %e, "engine failed to warm up — the segment worker's own transcribe() call will surface this error");
    }
    *model_ready = true;
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

/// Makes the pill visible and correctly positioned, without touching its
/// displayed state — separated from `show_pill` (below) so the "done"
/// handler can reveal the pill and emit "done" onto it, instead of
/// `show_pill`'s bundled "listening" state (2026-08-30: the pill now hides
/// during transcription and needs to reappear showing "done", not
/// "listening" — see module docs' "Only two visible processing-free
/// states" note).
///
/// Applies the last-known content width (see `PILL_LAST_CONTENT_WIDTH`)
/// right before showing — 2026-08-30 fix: `apply_pill_layout` used to run
/// unconditionally on every `set_pill_vibrancy_layout` IPC call, including
/// the one `pill.js` fires immediately on page load while the window is
/// still hidden (`visible: false` in `tauri.conf.json`). Resizing and
/// remasking a *hidden* vibrant window turned out to leave a real,
/// persistent WindowServer compositing ghost — a rectangular blur artifact
/// with no backing window at all, confirmed via `CGWindowListCopyWindowInfo`
/// reporting only the caller's real windows while the ghost stayed visible
/// on screen regardless, and confirmed tied to this process specifically
/// since killing it cleared the ghost immediately. `apply_pill_layout` now
/// only ever touches the real window (size, position, vibrancy mask) while
/// it's visible or about to become visible right here — never while
/// genuinely hidden.
fn reveal_pill<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window(PILL_WINDOW) {
        // `.show()` first, then layout — `apply_pill_layout` only acts
        // while `is_visible()` is true (that's the whole fix, see its own
        // docs), and that flag flips synchronously on `.show()`/`orderFront`
        // well before any actual frame renders, so this ordering doesn't
        // reintroduce a visible flash of the stale size/position.
        let _ = win.show();
        let width = *PILL_LAST_CONTENT_WIDTH
            .lock()
            .expect("pill content width lock poisoned");
        apply_pill_layout(&win, width);
    }
}

fn show_pill<R: Runtime>(app: &AppHandle<R>) {
    reveal_pill(app);
    emit_state(app, "listening");
}

/// Bottom-center, clear of the macOS Dock — recomputed on every show
/// rather than once at startup, since a monitor's work area can change
/// between sessions (external display connect/disconnect, Dock resize,
/// Dock auto-hide toggled). `Monitor::work_area()` already excludes both
/// the Dock and the menu bar — it's a thin wrapper over `NSScreen`'s
/// `visibleFrame` (see tauri-runtime-wry's macOS monitor impl) — so this
/// needs no native shim of its own, unlike the ScreenCaptureKit/permissions
/// integrations that genuinely require one.
///
/// This dock-anchored default only applies until the user drags the pill
/// somewhere else (2026-08-30: the pill is now draggable via
/// `data-tauri-drag-region` on `#pill`, see `pill/index.html`) — see
/// `PILL_USER_POSITIONED` below for how a real user drag is told apart
/// from our own programmatic `set_position` calls.
const PILL_BOTTOM_MARGIN: i32 = 16;

/// pill.css's `#pill { height: 36px }` — the one dimension that never
/// changes across states, unlike width (see `resize_pill_to_content`).
pub(crate) const PILL_HEIGHT: f64 = 36.0;

/// Set once the user has dragged the pill to a position of their own —
/// after that, `reveal_pill`'s dock-anchored default is skipped entirely
/// (the pill stays exactly where they left it, including across hide/show
/// cycles) and `resize_pill_to_content`'s width-driven repositioning
/// preserves the *current* horizontal center instead of snapping back to
/// dock-center. Never reset — a drag is a durable user preference for the
/// rest of the process's life, not a one-shot override.
static PILL_USER_POSITIONED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// True for the duration of a `set_position` call this module makes
/// itself. `lib.rs`'s `WindowEvent::Moved` handler (the only way to
/// observe a native OS-level drag, since `data-tauri-drag-region` moves
/// the window directly at the WindowServer level with no JS drag-start/
/// drag-end event of our own) checks this flag to tell "we just moved it"
/// apart from "the user just dragged it" — without this, our *own*
/// dock-anchoring or resize-recentering calls would immediately
/// mis-mark themselves as a user drag the moment they run.
static PILL_PROGRAMMATIC_MOVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn set_pill_position<R: Runtime>(win: &tauri::WebviewWindow<R>, x: i32, y: i32) {
    PILL_PROGRAMMATIC_MOVE.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
}

/// Called from `lib.rs`'s `on_window_event` handler for the pill window on
/// every `WindowEvent::Moved`. A move this module itself just triggered
/// (via `set_pill_position`) clears the programmatic flag and is ignored;
/// any other move is a real user drag, which permanently switches the
/// pill out of dock-anchored mode.
pub(crate) fn handle_pill_moved() {
    if PILL_PROGRAMMATIC_MOVE.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    PILL_USER_POSITIONED.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn position_pill_above_dock<R: Runtime>(win: &tauri::WebviewWindow<R>) {
    if PILL_USER_POSITIONED.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Ok(Some(monitor)) = win.current_monitor().or_else(|_| win.primary_monitor()) else {
        return;
    };
    let Ok(win_size) = win.outer_size() else {
        return;
    };
    let work_area = monitor.work_area();

    let x = work_area.position.x + (work_area.size.width as i32 - win_size.width as i32) / 2;
    let y = work_area.position.y + work_area.size.height as i32
        - win_size.height as i32
        - PILL_BOTTOM_MARGIN;

    set_pill_position(win, x, y);
}

/// Resizes the pill window to exactly fit `#pill`'s own current width,
/// then repositions it — bottom-center above the Dock by default, or (once
/// the user has dragged the pill, see `PILL_USER_POSITIONED`) keeping its
/// *current* horizontal center and vertical position fixed instead, so a
/// user-chosen spot survives every state-driven width change rather than
/// snapping back to the dock the next time the pill's content resizes.
///
/// Not cosmetic tuning: a fixed-width window left dead space around a
/// narrower pill state (e.g. `done`'s icon+status vs. `listening`'s
/// waveform+timer+controls), and native vibrancy's own blur radius bled a
/// visible soft "wisp" past the pill's rounded end into that dead space —
/// a real platform limitation (see `vibrancy.rs`'s module docs) that no
/// amount of mask-geometry tuning could fully hide, since the blur bleeds
/// from a hard alpha edge regardless of how tightly it's drawn. Shrinking
/// the window itself to have no dead space left removes the space the
/// blur had to bleed into, rather than trying to mask around it.
///
/// Called from `lib.rs`'s `set_pill_vibrancy_layout` command, which
/// pill.js's `ResizeObserver` on `#pill` drives on load and on every state
/// change (the pill window itself is `resizable: false` in
/// tauri.conf.json, meaning user-*resize*-by-dragging-an-edge is off, but
/// programmatic resize from here still works, and `data-tauri-drag-region`
/// separately makes the whole pill draggable *by position* — different
/// axis, not gated by `resizable`).
pub(crate) fn resize_pill_to_content<R: Runtime>(
    win: &tauri::WebviewWindow<R>,
    content_width: f64,
) {
    let width = content_width.ceil().max(1.0);

    if PILL_USER_POSITIONED.load(std::sync::atomic::Ordering::SeqCst) {
        // Preserve the user's chosen horizontal center and vertical
        // position — read the window's own current frame (before this
        // resize) rather than tracking a separately-stored point, so this
        // stays correct no matter how many resizes happen in a row.
        if let Ok(current) = win.outer_position() {
            if let Ok(current_size) = win.outer_size() {
                let center_x = current.x + current_size.width as i32 / 2;
                // LogicalSize -> physical width needs the window's own
                // scale factor; outer_size/outer_position are already
                // physical, so convert our logical target width the same
                // way before computing the new physical x.
                let scale = win.scale_factor().unwrap_or(1.0);
                let new_physical_width = (width * scale).round() as i32;
                let x = center_x - new_physical_width / 2;
                let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                    width,
                    PILL_HEIGHT,
                )));
                set_pill_position(win, x, current.y);
                return;
            }
        }
    }

    let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
        width,
        PILL_HEIGHT,
    )));
    position_pill_above_dock(win);
}

/// The most recent content width `pill.js` has reported, regardless of
/// whether the window was visible at the time — `reveal_pill` reads this to
/// apply the correct size right before showing, since `apply_pill_layout`
/// itself is a no-op while hidden (see its own docs for why). 190.0 matches
/// `tauri.conf.json`'s own pre-JS fallback width, so the very first
/// `reveal_pill` call (if it somehow raced ahead of any layout report at
/// all) still has a sane value rather than an arbitrary default.
static PILL_LAST_CONTENT_WIDTH: std::sync::Mutex<f64> = std::sync::Mutex::new(190.0);

/// Applies `content_width` to the real pill window — resize, reposition,
/// and re-mask its vibrancy layer to match — but only while the window is
/// actually visible or about to become visible (see `reveal_pill`, the
/// other caller). Never touches the real window while it's genuinely
/// hidden.
///
/// This guard is the actual fix for a real bug (2026-08-30): resizing and
/// remasking a *hidden* vibrant window left a persistent WindowServer
/// compositing ghost on screen — a rectangular blur artifact with no
/// backing window (`CGWindowListCopyWindowInfo` never reported it, even
/// while it stayed visibly rendered), confirmed tied to this process by
/// killing it and watching the ghost clear immediately. `pill.js` reports
/// a layout on every page load regardless of window visibility (the
/// webview loads and runs JS even while `visible: false`), which used to
/// mean every single app launch triggered exactly this — not a rare edge
/// case.
pub(crate) fn apply_pill_layout<R: Runtime>(win: &tauri::WebviewWindow<R>, content_width: f64) {
    *PILL_LAST_CONTENT_WIDTH
        .lock()
        .expect("pill content width lock poisoned") = content_width;

    if !win.is_visible().unwrap_or(false) {
        return;
    }

    resize_pill_to_content(win, content_width);

    let rect = vibrancy::Rect {
        x: 0.0,
        y: 0.0,
        width: content_width.ceil().max(1.0),
        height: PILL_HEIGHT,
    };
    vibrancy::mask_to_shape(
        win,
        vibrancy::Shape {
            radius: vibrancy::capsule_radius(rect),
            rect,
        },
    );
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
    update_tray_listening_indicator(app, state == "listening");
}

/// Flips the tray's "Start Listening"/"Stop Listening" item to match
/// whether a recording is actually in progress, every time the pill's own
/// state changes — `state == "listening"` is the one state that means a
/// real recording is running; every other state (`loading`, `canceling`,
/// `done`) reverts it. A no-op if the tray item was never built (shouldn't
/// happen in practice — `lib.rs`'s `setup()` always manages one — but this
/// runs on every state transition, so failing loudly here would be a poor
/// trade for a purely cosmetic indicator).
fn update_tray_listening_indicator<R: Runtime>(app: &AppHandle<R>, listening: bool) {
    let Some(item) = app.try_state::<crate::ListeningMenuItem<R>>() else {
        return;
    };
    let (text, icon) = if listening {
        ("Stop Listening", Some(crate::tray_listening_danger_icon()))
    } else {
        ("Start Listening", None)
    };
    let _ = item.0.set_text(text);
    let _ = item.0.set_icon(icon);
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
