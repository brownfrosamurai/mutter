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

**T7 (mid-flight engine-change abort-and-restart) and the engine-selection UI — deliberately deferred, user-confirmed 2026-08-30.** Both need a second real `TranscriptionEngine` to mean anything; `AppleSpeechEngine` is still a pure stub blocked on the Section 6 benchmark. Building either now would be exactly the failure mode this session already caught once (the whisper `detect_language` bug): untested plumbing with nothing real to verify it against. Revisit once a second engine exists.

**Phase 7 (hardening):** real daily-rotating file logging (`logging.rs`, verified live), `permissions.rs` wired to real OS APIs (`AXIsProcessTrusted`, `CGPreflightScreenCaptureAccess`, a new `native/permissions_shim.m` for AVFoundation mic status — all three verified live via the dashboard), CI runs fmt+clippy+check+test. A real Section 11 recovery screen (`ui/recovery/`) now exists: on `HistoryError::MigrationFailed` the pill/dashboard windows close, hotkeys/session never start, and a dedicated window opens naming the exact backup path — verified live by corrupting the real DB, booting the app, screenshotting the rendered recovery screen with the real backup path, and restoring the original DB. Finding this also caught a real bug: `history::open_at` used to back up the DB file on *every* launch when the file existed, even when no migration ran — meaning normal daily use would accumulate one backup file per app start forever. Fixed to only back up (and call `to_latest()`) when the schema version is actually behind latest (`migrations::LATEST_VERSION`), with a regression test (`reopening_an_up_to_date_db_does_not_create_a_backup_file`). Signed builds and Ed25519 update-signature verification are now both real — see the two entries right below — leaving only the actual distribution activation (a real GitHub remote + release manifest) genuinely open.

**Fixture-audio integration test** (`src-tauri/tests/fixture_audio.rs`) now exists and is real — a checked-in synthesized-speech WAV (`tests/fixtures/sample-en.wav`, macOS `say` → `afconvert`, ~170KB) run through the actual `WhisperEngine`. `#[ignore]`d and deliberately not wired into CI (a ~500MB model download per run isn't worth making a required gate for v1); run it explicitly with `cargo test --test fixture_audio -- --ignored --nocapture`. Running it for the first time caught a real, critical bug: `whisper.rs` was calling `params.set_detect_language(true)` in addition to `set_language(None)` — but per whisper.cpp's own source, `detect_language` means "detect the language and return immediately," skipping transcription entirely. Every real dictation would have silently produced an empty transcript forever; auto-detection alone (leaving `language` as `None`) already triggers detection as part of the normal transcription pass and doesn't need that flag. Fixed and reverified live: the fixture now round-trips to real text ("Testing 1-2-3. This is a fixture recording for Mutter's integration test.") with language `en` detected correctly.

**Also fixed while building the recovery screen:** `history::open_at` used to back up the DB file unconditionally on every launch whenever the file existed, even when no migration ran — meaning ordinary daily use would accumulate a new backup file on every single app start, forever. Now it only backs up (and calls `to_latest()`) when the schema version is actually behind latest, with a regression test guarding it.

**Two more real bugs found by re-auditing `session.rs` after the whisper fix above (same instinct: untested integration paths deserve suspicion):**

- The 120s auto-continue cap can be hit while a cancel countdown (`Phase::CancelPending`) is pending — capture deliberately keeps buffering in the background during a countdown (module docs). The `at_cap` handling only reacted to this during `Phase::Listening`; during `CancelPending` the notification was silently dropped, `capture/mic.rs`'s audio callback then permanently stops accepting new samples once its own cap flag is set, and a subsequent "resume" (second Escape) would look like it's still recording while actually capturing nothing — a silent data-loss bug, not just a missed UI update. Fixed: the cap is now handled identically during `CancelPending`, auto-continuing the segment without disturbing the countdown UI.
- `TranscriptionEngine` gained a real `ensure_ready()` method (default no-op, overridden by `WhisperEngine` to force its lazy model load) and `session.rs`'s `run()` now calls it — with a visible pill `"loading"` state — before the very first hand-off to the engine in the process lifetime. Previously the plan's own Performance Issue 8 requirement ("first transcription pays load latency, shown via the pill's loading state") was simply never wired: a fresh install's first-ever dictation would sit under a generic "transcribing" label for however long the ~500MB model download took, indistinguishable from a hang. Verified live via the fixture-audio test calling `ensure_ready()` then `transcribe()` in sequence.

**Grammar cleanup (Section 5) decision — confirmed with the user 2026-08-29:** Option A (rule-based cleanup) only, for v1. Option B (local-LLM cleanup) is deliberately not built — logged as a possible future item, not committed to — pending real signal from Phase 8 dogfooding on whether it's actually needed.

**T12 (Section 15 — "validate terminal text-injection before Phase 3 begins") is now done, confirmed 2026-08-30.** Mic/Accessibility/Screen Recording all show Granted on this dev machine (already granted before this check — not something the agent did). Using the debug-only "Manual QA: Test Injection" tool (dashboard Settings panel, `debug_test_injection` command), the user tested `injection::insert_at_cursor()` live against both TextEdit and a real Terminal window: text landed correctly in both, and — the actual point of T12 — three lines of test text pasted into the terminal arrived as literal multi-line text in one shot (bracketed-paste respected), not executed line-by-line. The agent's own earlier attempt at this same test via synthetic clicks/keystrokes reported success but produced no visible paste in either app; the user's real click worked immediately, confirming that was a limitation of synthetic keyboard-event delivery in this sandboxed dev environment (synthetic mouse clicks are delivered fine here, as proven wiring the dashboard's titlebar buttons; synthetic *keyboard* events from an unprivileged test-harness process are not, consistent with macOS gating synthetic keystroke injection behind the posting process's own Accessibility grant) — not a bug in `injection.rs`.

**What genuinely cannot be verified by the agent, stated plainly — these need a human:**

- An actual hotkey-press → pill-shows → record → transcribe → insert cycle. A synthetic CGEvent from this unprivileged, non-Accessibility-trusted dev environment does not trigger the global shortcut (macOS filtering synthetic input from untrusted processes) — the app boots clean and hotkeys register (`RUST_LOG=info` confirms), but only a **real physical key press** (`Cmd+Shift+Space`) can confirm the full loop fires. (Text injection itself is no longer part of this gap — see T12 above; what's left here is specifically the global-hotkey-to-capture handoff.)
- Screen Recording-gated ScreenCaptureKit behavior, and the accuracy benchmark above.

**Stable signing identity — done 2026-08-30.** The user created a self-signed "Mutter Dev Signing" certificate in Keychain Access and set it to "Always Trust" for Code Signing (the certificate exists but `security find-identity -v -p codesigning` won't list it as valid until that trust step happens — a real gotcha, not a mistake, worth remembering if this is ever redone). `tauri.conf.json`'s `bundle.macOS.signingIdentity` now references it by name instead of `"-"`. Verified: `codesign -dv` on a real `cargo tauri build --debug` output shows `Authority=Mutter Dev Signing`, not `adhoc`.

**Important nuance found while verifying this — the stable signature does NOT cover the normal dev loop.** Both plain `cargo build` and `cargo tauri dev` produce a binary signed `adhoc,linker-signed` regardless of `signingIdentity` — that setting only takes effect in Tauri's actual bundling step (`cargo tauri build`), which is slower (full compile + `.app` + `.dmg` packaging, ~20-30s+) and not what this whole session's testing has used. Practical upshot: day-to-day iteration (`cargo build` + run the raw binary, as done throughout this session) still re-signs ad-hoc every time and will still prompt for permissions again — the stable identity only pays off once you (a) run `cargo tauri build --debug`, (b) mount the resulting `.dmg` and drag `Mutter.app` to `/Applications` (or anywhere persistent), and (c) run *that* installed copy repeatedly. That installed copy's signature won't change across future rebuilds-and-reinstalls, so permissions granted to it stick — this is the right point to switch to for Phase 8's two-week daily-use validation, not for quick iteration.

**Ed25519 update-signing keypair — done 2026-08-30 (T2).** Generated via `cargo tauri signer generate`, private key at `~/.mutter-signing/update-key.pem` (outside the repo, `chmod 600`, no password — a deliberate simplicity choice for a single-developer v1 with no CI-automated publishing yet; add a password and move it to a proper secret store before any CI workflow ever touches it). The public key is pinned in `tauri.conf.json`'s `plugins.updater.pubkey`. Verified for real, not just generated-and-assumed-correct: wrote a small scratch program using `minisign-verify` (the exact crate `tauri-plugin-updater` uses internally) that signed the real `.dmg` from a `cargo tauri build --debug` run and confirmed (a) the valid signature verifies against the pinned pubkey, and (b) a one-byte-tampered copy of the same file correctly fails verification. `plugins.updater.active` stays `false` and `endpoints` stays `[]` — this repo has no `git remote` at all yet, so there's genuinely nowhere to point a real update manifest; that's Section 12 distribution work (a real GitHub repo + Releases + CI publishing), not this task, and shouldn't be faked with a placeholder URL.

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
