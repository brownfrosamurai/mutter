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
- **ScreenCaptureKit Rust-binding spike: resolved, a build-time Objective-C shim.** `native/system_audio_shim.{h,m}` — compiled by `build.rs` via the `cc` crate, linked against ScreenCaptureKit/CoreMedia/CoreAudio/Foundation. Compiles clean against the real macOS SDK headers (`clang -fobjc-arc -Wall -Wextra`, zero warnings) and the full Rust crate links successfully against it (`cargo build`, verified 2026-08-29) — resolving the Section 9/15 binding-path question in favor of the shim over an `objc2`-based crate bridge. `capture/system_audio.rs` wraps it with the same start/stop/is_at_cap shape as `MicCapture`. **Not verified**: actual runtime behavior — needs the user to grant Screen Recording permission via a real system dialog, and this environment has no way to exercise the shareable-content/display lookup live either.

**Architecture deviation from the original spec, documented per the user's instruction to flag rather than silently override:** `TranscriptionEngine::transcribe` originally returned `Result<String, EngineError>` with no way to carry the detected language out. `TextProcessor::process` needs a `language` argument (Section 10) and the dashboard needs a per-language breakdown (Section 8) — neither was derivable from a bare `String`. Fixed by introducing `engine::Transcript { text, language }` as the return type. Audio-in stays language-free; only the output shape changed.

**Phase 1 (core loop): implemented and boots cleanly.** `session.rs` is the orchestrator wiring hotkey -> mic capture -> cancel state machine -> engine -> grammar cleanup -> injection -> history, with real Tauri events driving the pill UI (`mutter://pill-state`/`elapsed-seconds`/`cancel-countdown`, consumed for real by `ui/pill/pill.js` via `window.__TAURI__`, not just documented as a TODO). `capture/mic.rs` (cpal), `injection.rs` (AXUIElement + clipboard/CGEvent fallback), and `engine/grammar.rs` (Section 5 Option A, rules-based) are all real implementations now, not stubs. `hotkey.rs` registers both toggle hotkeys via `tauri-plugin-global-shortcut` — verified live (`RUST_LOG=info`, "global hotkeys registered" logs, app stays up).

**What's real vs. what's structurally-sound-but-unverified, stated plainly:**

- Verified end-to-end at the process level: app boots, hotkeys register, windows stay correctly hidden until shown, tray+menu work, `cargo test` (30 tests) and `cargo check` are clean.
- **Not verified**: an actual hotkey press → pill-shows → mic-records → transcribe → insert cycle. A synthetic CGEvent posted from this (unprivileged, non-Accessibility-trusted) dev environment did not trigger the global shortcut — consistent with macOS filtering synthetic input from untrusted processes, not a code bug, but this means the only way to actually confirm the loop fires is a **real physical key press**, which needs a human at the keyboard. Whoever picks this up next: press `Cmd+Shift+Space`, watch for the pill, check `RUST_LOG=info` output.
- Runtime correctness of the AX text-injection path specifically also still needs the user to grant Accessibility permission via a real macOS system dialog — nothing else can do that.

**Known gap, not silently glossed over:** `HistoryStore::open()` failing on migration is supposed to "refuse to launch normally and show a recovery screen naming the backup path" (Section 11). The backup-then-migrate logic itself is real and tested; the recovery *screen* isn't built — today a failure just logs loudly and disables the session for that run. Real UI work, flagged as still open.

**Stable ad-hoc signing identity (Section 11, needed from Phase 1 onward to avoid repeated Accessibility TCC re-prompts during dev):** `tauri.conf.json` now sets `bundle.macOS.signingIdentity: "-"`, but a plain `-` ad-hoc signature is NOT stable across rebuilds (the signature hash changes with the binary). A genuinely *stable* identity needs a locally-generated self-signed code-signing certificate in the user's Keychain — that's a one-time step touching the system keychain, which the agent deliberately did not do on its own (out of scope for an autonomous filesystem-only agent to modify system security state). Whoever hits repeated Accessibility re-prompts during development: generate a certificate via Keychain Access ("Certificate Assistant" → "Create a Certificate" → type "Code Signing"), then reference its name in `signingIdentity` here.

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
