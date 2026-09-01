# Why session orchestration works the way it does

`session.rs` is the actual toggle-hotkey core loop: hotkey → mic capture → the Escape/cancel state machine → the transcription engine → grammar cleanup → text injection → history. This document explains the shape of that design — the actor/channel architecture, and the handful of edge cases (auto-continue during a cancel countdown, capture threading) that drove real decisions, not just the happy path.

## The core problem: `cpal::Stream` is not `Send`

`MicCapture` wraps a `cpal::Stream`, and `cpal::Stream` is not `Send` — it cannot cross an `.await` point inside an async task. But the session orchestrator needs to be async (it's juggling timers, channel receives, and a cancel countdown concurrently via `tokio::select!`).

The fix is **not** making the orchestrator sync, or wrapping the stream in something that fakes `Send` — it's isolating the non-`Send` state on its own dedicated, plain OS thread (`spawn_capture_actor`), which owns `MicCapture` for its entire lifetime and never crosses an `.await` point itself. The async orchestrator talks to it only through a `std::sync::mpsc` channel of `CaptureCommand`s (`Start`/`Stop`) and gets back plain `Vec<f32>` buffers — never the stream. `SystemAudioCapture` gets the identical treatment (`spawn_system_audio_actor`) even though it happens to be `Send` (it's raw FFI pointers, no `cpal` internals) — its `start()`/`stop()` still *block* the calling thread for however long ScreenCaptureKit's async setup or a permission dialog takes, which is just as wrong to run on a tokio worker thread.

## Why a single-threaded actor over an mpsc channel, not shared mutable state

The alternative to "one actor reading one command channel in a loop" would be something like an `Arc<Mutex<SessionState>>` that hotkey callbacks and timers all lock and mutate directly. The actor design was chosen instead because the state machine here (`Phase::Idle/Listening/Transcribing/CancelPending`, crossed with the separate `CancelStateMachine`, crossed with `system_audio_active`) has enough simultaneous concerns — a 1-second elapsed-time tick, a cancel countdown tick, a capture-buffer-cap notification, a "final segment done" signal — that reasoning about them as one sequential `tokio::select!` loop is far simpler than reasoning about which of several concurrent lock-holders might interleave. Hotkey presses and Escape presses become messages fed into one queue; there is never a moment where two code paths are deciding what the current phase means at the same time.

## Why transcription runs on a separate worker from capture

`segment_worker` is a second, independent task, fed by its own channel (`SegmentJob`s). This matters for one specific reason: **a slow transcription must never block new capture from starting.** The primary use case this app was built for — dictating a long spec or bug report to an AI coding agent — routinely runs past the mic buffer's 120-second cap. When that happens, the buffered audio is handed off to `segment_worker` and a *new* capture segment starts immediately; the user's toggle-hotkey session continues uninterrupted while the first segment is still being transcribed in the background.

`segment_worker` is deliberately a **single FIFO queue**, not a task spawned per segment. This is what guarantees a multi-segment session inserts text in the order it was actually spoken — if segments transcribed concurrently and finished out of order, a long dictation session could paste its middle before its beginning.

## The cancel state machine is separate from `Phase`

`cancel.rs`'s `CancelStateMachine` (`Recording → CancelPending → {Discarded | Resumed}`) is a small, independently-testable state machine with no knowledge of hotkeys, capture, or the engine — it only knows about Escape presses and countdown expiry. `session.rs`'s own `Phase` enum drives it, but the two are not merged into one type, because they answer different questions: `Phase` is "what is the session doing right now" (idle, listening, transcribing, or mid-cancel), while `CancelState` is "where is this particular cancel flow" (which only exists during `Phase::CancelPending`). Keeping them separate is what let `cancel.rs`'s tests exist as pure unit tests with zero Tauri/async dependencies.

## Two deliberate narrowings of the original spec

The plan of record (`docs/mutter-project-plan.md` Section 7) left some latitude that this implementation resolved concretely:

- **Cancel does not pause capture.** The spec noted that pausing during the cancel countdown was "simpler" but left the choice open. This implementation takes the *even* simpler path: capture keeps buffering in the background through the whole countdown, untouched. `Resumed` (a second Escape before the countdown expires) is then trivially correct — it "resumes exactly where it left off" because capture was never interrupted. The only observable difference from a true pause is that audio spoken *during* the countdown itself is retained on resume, which is a reasonable superset of the spec's intent, not a bug.
- **Cancel is only offered while `Listening`, not `Transcribing`.** By the time capture has stopped and audio has been handed to the engine, canceling would need whisper.cpp abort-callback plumbing to interrupt in-flight inference — judged not worth the complexity for what's normally a multi-second window. Escape during `Transcribing` is a documented no-op.

## The auto-continue-during-cancel edge case (a real bug this design had to catch)

Because capture keeps running unattended through `CancelPending` (see above), it can still hit the 120-second buffer cap *while a cancel countdown is in progress*. The main loop's `at_cap` handling has an explicit branch for exactly this (`Phase::CancelPending if at_cap.is_some()`), separate from the `Phase::Listening` branch. Without it, `capture/mic.rs`'s audio callback — which permanently stops accepting new samples once its own cap flag is set — would silently stop recording forever the moment the cap was hit mid-countdown, and a subsequent "resume" (a second Escape) would look like it's still recording while actually capturing nothing. This was found and fixed by re-auditing the session code after an unrelated bug, on the theory that untested integration paths deserve extra suspicion — see `CLAUDE.md`'s history for the full incident.

The fix's other half is what it does *not* do: the cap-triggered segment hop during `CancelPending` never emits a pill-state change. The countdown UI must stay visually undisturbed by a background bookkeeping event the user never asked about.

## Why the pill has no "transcribing" visual

`session.rs`'s stop-recording paths call `hide_pill()` the instant capture stops, and only call `reveal_pill()` again once `segment_worker` reports the final segment done. The gap between those two calls is normally under two seconds; showing a spinner for that gap just to remove it a moment later reads as flicker, not information. The one deliberate exception is `loading` — the very first transcription in a process's lifetime pays a real model-load cost (a ~500MB download on a fresh install), which can be long enough that hiding through it would look like a hang. `warm_up_model_if_needed` is what decides, per call, whether to emit that one visible exception.

## Related

- [`reference-architecture.md`](reference-architecture.md) — the module map this document assumes
- [`explanation-permission-gate.md`](explanation-permission-gate.md)
- [`explanation-grammar-pipeline.md`](explanation-grammar-pipeline.md) — what `segment_worker` hands text to before injection
