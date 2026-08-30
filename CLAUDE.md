# Mutter

Local-first, multi-language speech-to-text dictation app with a second system-audio-capture mode ("like Granola"). macOS-only for v1.

**Plan of record:** `docs/mutter-project-plan.md` — read this before making any architecture or scope decision. It has already been through office-hours diagnostic, two rounds of adversarial review, and a full engineering review (architecture, code quality, tests, performance) plus an independent outside-voice pass. Don't re-litigate decisions already made there; if a decision looks wrong, say so explicitly and point to the section, rather than silently overriding it.

**Original spec:** `docs/mutter-idea-dump.md` — the user's own freeform requirements. Source of truth for scope.

**Design system:** `DESIGN.md` — glassmorphic UI tokens (colors, typography, spacing, motion) for the pill HUD and dashboard.

## Hard constraints (do not violate)

- **No Swift, no AppKit code.** Everything UI-facing is a Tauri webview (plain HTML/CSS/vanilla JS, no framework). Native macOS APIs are called from Rust via Tauri/bindings, never via hand-written Swift/Obj-C source — a build-time Objective-C shim for ScreenCaptureKit is the one exception, and it's still not Swift.
- **Zero network calls after model download.** The entire pitch is local-first. No default-on telemetry, no remote crash reporting (see `docs/mutter-project-plan.md` Section 11 — error logging is local-only, metadata-only, never transmitted).
- **Zero paid resources for v1.** Ad-hoc code signing, not paid Apple Developer notarization, until/unless the validation gate is met and the app leaves the user's own machine (Section 11/12).
- **No payment, account, or licensing system.** Ever, in v1.

## Architecture at a glance

- `TranscriptionEngine` trait (audio → text) and `TextProcessor` trait (text → text, used by grammar cleanup) are separate — do not conflate them (this was a real bug caught in eng review, plan Section 10).
- `PermissionGate<T>` is one generic state machine (NotRequested/Denied/Granted/Unavailable) shared by mic, Accessibility, and system-audio permissions — not three hand-rolled implementations.
- `EngineError` is a typed enum (`ModelNotLoaded`, `UnsupportedLanguage`, `InferenceFailed`, `Timeout`), not a generic boxed error.
- Native FFI/bridge calls (whisper-rs, the ScreenCaptureKit bridge) are wrapped in `std::panic::catch_unwind` — a panic must never take down the whole app.
- Toggle hotkey, not push-to-talk: press once to start, press again to stop.
- Language is auto-detected from audio, never manually selected in settings.

## Testing

Rust: `cargo test` (once Cargo.toml exists and dependencies are wired). No test framework config exists yet at scaffold time — this section should be updated once Phase 0/1 lands real tests.

Full test strategy and coverage requirements: `docs/mutter-project-plan.md` Section 11 (Test strategy) and Section 21 (Failure Modes).

## Current state

Scaffold only, as of 2026-08-29. No feature logic implemented yet. Native-integration modules (`capture/`, `engine/whisper.rs`, `engine/apple_speech.rs`, `injection.rs`) are compiling stubs. `engine/mod.rs` (traits + `EngineError`), `permissions.rs` (`PermissionGate<T>`), and `cancel.rs` (Escape state machine) are written for real since they were fully specified before any code existed.

Next: Phase 0 spikes per the roadmap (plan Section 15) — engine benchmark (Whisper vs. Apple's on-device Speech framework), pill-window feasibility, ScreenCaptureKit Rust-binding approach — before Phase 1's core loop gets built out.

Rust/Cargo are now installed on the dev machine and `cargo check` builds the scaffold clean (verified 2026-08-29).
