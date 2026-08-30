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

`cd src-tauri && cargo test` — 34 tests, all passing (cancel state machine, permission gates incl. live OS-backed smoke tests, mic capture resampling/downmix, whisper model-tier helpers, grammar cleanup, history store incl. migrations/aggregates/pagination). `cargo clippy --all-targets` and `cargo fmt --check` are both clean and required in CI (`.github/workflows/ci.yml`).

Full test strategy and coverage requirements: `docs/mutter-project-plan.md` Section 11 (Test strategy) and Section 21 (Failure Modes).

## Current state (as of 2026-08-30)

**Phases 0-2, 5, and 7 are implemented for real and verified as far as this environment allows; Phases 3-4 and 6 are substantially done; Phase 8 is mostly human-gated.** Per the user's directive to build the whole roadmap end-to-end without stopping between phases. Check here before assuming any module is still a stub — very little still is.

**Phase 0 (all three spikes resolved):**

- *Pill feasibility* — works. Live-verified via targeted window capture: real per-pixel transparency, backdrop blur, true capsule shape. No rectangular-HUD fallback needed.
- *whisper-rs integration* — the native whisper.cpp/ggml build compiles clean on this machine; no CLI shell-out or raw FFI needed. `engine/whisper.rs` lazily downloads+loads a resident `WhisperContext`, runs catch_unwind-guarded inference, returns the detected language via `engine::Transcript { text, language }` (a deliberate deviation from the original bare-`String` return type, documented inline — `TextProcessor::process` and the dashboard's language breakdown both need it).
- *ScreenCaptureKit binding* — a build-time Objective-C shim (`native/system_audio_shim.{h,m}`), not an `objc2` crate bridge. Compiles clean against the real SDK headers and links successfully into the full crate.
- *Multi-language accuracy benchmark* — **cannot be completed by the agent** (no microphone/ears here). This is the user's own Section 17 assignment, still outstanding.

**Phase 1 (core loop) + Phase 2 (cancel/history):** `session.rs` orchestrates hotkey → capture → cancel state machine → engine → grammar cleanup → injection → history as a single-threaded actor, with a separate sequential worker for transcription so a slow segment never blocks new capture (Section 3's auto-transcribe-and-continue on the 120s mic cap). Real Tauri events drive the pill UI live (`pill.js` actually listens, not just documents the contract). `capture/mic.rs` (cpal), `injection.rs` (AXUIElement + clipboard/CGEvent fallback), `engine/grammar.rs` (Section 5 Option A), and `history/` (rusqlite, backup-then-migrate, running aggregates) are all real, not stubs.

**Phase 4 (system-audio):** wired into `session.rs` — its own toggle (mutually exclusive with mic dictation, not a variant of the mic `Phase`), sharing the same `segment_worker` pipeline per Section 9. Does not get its own Escape-cancel flow (the plan doesn't specify one for it; stopping via the toggle hotkey is the only way to end a capture).

**Phase 5 (dashboard):** wired to real data via Tauri commands (`get_metrics`, `get_language_breakdown`, `get_history_page`, `copy_history_text`, `get_permission_status`, `quit_app`) — verified live by temporarily making the dashboard visible and screenshotting each panel. The Latency section is a deliberate placeholder: it's from the reference mockup used to design the UI, not from Section 8's actual metric list, and wasn't worth building without the plan calling for it.

**Phase 6 (adapter formalization):** substantially achieved as a side effect of how Phase 0-1 were built — `TranscriptionEngine`/`TextProcessor` are already the generic swap-by-config traits Section 10 wants. What's NOT done: a settings UI control to actually switch engines (moot until `AppleSpeechEngine` is real) and `AppleSpeechEngine` itself, both blocked on the human-only Section 6 benchmark.

**Phase 7 (hardening):** real daily-rotating file logging (`logging.rs`, verified live), `permissions.rs` wired to real OS APIs (`AXIsProcessTrusted`, `CGPreflightScreenCaptureAccess`, a new `native/permissions_shim.m` for AVFoundation mic status — all three verified live via the dashboard), CI runs fmt+clippy+check+test. NOT done, deliberately: signed builds and Ed25519 update-signature verification (need a real signing identity/update keys that don't exist yet), and a fixture-audio integration test (needs a checked-in audio sample and a ~500MB model download, not added as a CI-blocking step).

**What genuinely cannot be verified by the agent, stated plainly — these need a human:**

- An actual hotkey-press → pill-shows → record → transcribe → insert cycle. A synthetic CGEvent from this unprivileged, non-Accessibility-trusted dev environment does not trigger the global shortcut (macOS filtering synthetic input from untrusted processes) — the app boots clean and hotkeys register (`RUST_LOG=info` confirms), but only a **real physical key press** (`Cmd+Shift+Space`) can confirm the full loop fires.
- Screen Recording-gated ScreenCaptureKit behavior, and the accuracy benchmark above.
- The Section 11 "recovery screen naming the backup path" on a migration failure isn't built — a failure today just logs loudly and disables the session for that run, rather than launching against a half-migrated schema (which the backup-then-migrate logic itself, tested, still prevents).

**Stable ad-hoc signing identity (Section 11, needed from Phase 1 onward to avoid repeated Accessibility TCC re-prompts during dev):** `tauri.conf.json` sets `bundle.macOS.signingIdentity: "-"`, but plain `-` ad-hoc signing is NOT stable across rebuilds (the signature hash changes with the binary). A genuinely stable identity needs a locally-generated self-signed code-signing certificate in the user's Keychain — a one-time step touching system security state that the agent deliberately did not do on its own. To fix: Keychain Access → Certificate Assistant → Create a Certificate → type "Code Signing" → reference its name in `signingIdentity`.

Rust/Cargo are installed on the dev machine; whisper-rs's native build compiles cleanly.

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
