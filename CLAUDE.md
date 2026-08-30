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

**Full implementation underway as of 2026-08-29** (user directive: build the entire roadmap end-to-end, phase by phase, without stopping between phases). This section is kept current at each phase milestone — check here first before assuming a module is still a stub.

Phase 0 spikes:
- **Pill feasibility: resolved, works.** Live-verified via `cargo tauri dev` + targeted window capture — real per-pixel window transparency, backdrop blur, and a true capsule shape with no square window frame. No fallback to a rectangular HUD needed.
- **whisper-rs integration path: resolved, whisper-rs bindings.** The native whisper.cpp/ggml build compiles cleanly on this machine (cmake/clang present) — no CLI shell-out or raw FFI needed (plan Section 14). `engine/whisper.rs` is a real implementation: lazy model download+load (curl, no HTTP-client dependency added), resident `WhisperContext`, catch_unwind-guarded inference, language auto-detection returned via the new `Transcript` struct (see below).
- **Multi-language accuracy benchmark: cannot be completed by the agent.** No microphone/ears in this environment — needs real audio samples in all six languages (Yoruba especially) and human judgment. This is explicitly the user's own Section 17 assignment ("run the Phase 0 engine benchmark yourself, informally"), still outstanding.
- **ScreenCaptureKit Rust-binding spike: in progress.**

**Architecture deviation from the original spec, documented per the user's instruction to flag rather than silently override:** `TranscriptionEngine::transcribe` originally returned `Result<String, EngineError>` with no way to carry the detected language out. `TextProcessor::process` needs a `language` argument (Section 10) and the dashboard needs a per-language breakdown (Section 8) — neither was derivable from a bare `String`. Fixed by introducing `engine::Transcript { text, language }` as the return type. Audio-in stays language-free; only the output shape changed.

Rust/Cargo are installed on the dev machine and whisper-rs's native build compiles cleanly (verified 2026-08-29).

## Skill routing

When the user's request matches an available skill, invoke it via the Skill tool. When in doubt, invoke the skill.

Key routing rules:

- Product ideas/brainstorming → invoke /office-hours
- Strategy/scope → invoke /plan-ceo-review
- Architecture → invoke /plan-eng-review
- Design system/plan review → invoke /design-consultation or /plan-design-review
- Full review pipeline → invoke /autoplan
- Bugs/errors → invoke /investigate
- QA/testing site behavior → invoke /qa or /qa-only
- Code review/diff check → invoke /review
- Visual polish → invoke /design-review
- Ship/deploy/PR → invoke /ship or /land-and-deploy
- Save progress → invoke /context-save
- Resume context → invoke /context-restore
- Author a backlog-ready spec/issue → invoke /spec
