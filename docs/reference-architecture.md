# Architecture reference

Complete module map of the Mutter codebase: what each file owns, its public surface, and how the pieces fit together. For *why* the architecture looks this way, see the [explanation](explanation-session-orchestration.md) docs linked throughout. For the exact IPC surface between Rust and the frontend, see [`reference-commands.md`](reference-commands.md).

## Two halves

- **`src-tauri/`** — the Rust backend. Owns every real capability: audio capture, transcription, grammar cleanup, text injection, history storage, permissions, native window vibrancy. Compiles to a single binary (`mutter`) plus a library crate (`mutter_lib`) that both the binary and the test suite link against.
- **`frontend/`** — a Vite + React + TypeScript app, built to a static `dist/` that Tauri embeds into the binary. Four independent HTML entry points (`dashboard.html`, `pill.html`, `recovery.html`, `onboarding.html`), one per native window, each with its own `main.tsx`.

Nothing crosses the boundary except through Tauri's IPC (`invoke`) and events (`emit`/`listen`) — the frontend never touches the filesystem, network, or OS APIs directly.

## Backend module map (`src-tauri/src/`)

| Module | Owns |
|---|---|
| `lib.rs` | App entry (`run()`), window/tray setup, every `#[tauri::command]`, the tauri-specta binding generator |
| `main.rs` | Thin `fn main()` that calls `mutter_lib::run()` |
| `session.rs` | The session orchestrator — the actual hotkey → capture → engine → injection → history pipeline. See [`explanation-session-orchestration.md`](explanation-session-orchestration.md) |
| `cancel.rs` | `CancelStateMachine` — the Escape-key cancel/countdown/resume state machine, independent of `session.rs`'s own phase enum |
| `hotkey.rs` | Global hotkey registration (`tauri-plugin-global-shortcut`), toggle semantics, shortcut-string validation |
| `capture/mic.rs` | Mic capture via `cpal` — its own OS thread, 120s ring buffer, downmix + resample to 16kHz mono |
| `capture/system_audio.rs` | System-audio (loopback) capture via a ScreenCaptureKit Objective-C shim, 300s buffer |
| `engine/mod.rs` | The `TranscriptionEngine` and `TextProcessor` traits, `EngineError`, `Transcript` |
| `engine/whisper.rs` | `WhisperEngine` — whisper.cpp via `whisper-rs`, lazy-loaded, Metal-accelerated; trims leading/trailing silence and suppresses non-speech-tag hallucinations before/during inference (see [`explanation-non-speech-hallucination-fix.md`](explanation-non-speech-hallucination-fix.md)) |
| `engine/apple_speech.rs` | `AppleSpeechEngine` — a stub, not built (see [ADR: why not `AppleSpeechEngine`](#why-applespeechengine-was-never-built)) |
| `engine/grammar.rs` | `RuleBasedCleanup` — Option A, five independently-toggleable rule-based text steps |
| `engine/llm_cleanup.rs` | `LlmCleanup` — Option B, a local Qwen2.5-0.5B GGUF model via `candle` |
| `engine/pipeline.rs` | `GrammarPipeline` — composes Option A (always) with Option B (if toggled on), with fallback |
| `injection.rs` | Text insertion at the cursor — AXUIElement primary path, clipboard+synthetic-paste fallback |
| `history/mod.rs` | `HistoryStore` — SQLite-backed history, metrics, language/activity/latency aggregates |
| `history/migrations/mod.rs` | Versioned schema migrations (`rusqlite_migration`), backup-then-migrate |
| `settings.rs` | `AppSettings` — persisted JSON preferences, `SettingField` enum for the generic toggle command |
| `permissions.rs` | `PermissionGate<T>` — one generic state machine shared by Mic/Accessibility/SystemAudio |
| `vibrancy.rs` | `apply_glass_shell` — real native `NSGlassEffectView`/`NSVisualEffectView` window material. See [`explanation-glass-shell.md`](explanation-glass-shell.md) |
| `logging.rs` | Local-only, metadata-only `tracing` setup (stdout + daily-rotating file) |
| `paths.rs` | `~/Library/Application Support/Mutter/` and its subdirectories |

## Native shims (`src-tauri/native/`)

Not hand-written Swift (the project's hard constraint) — small, build-time-compiled Objective-C files behind a plain C ABI, each wrapping exactly one framework call the Rust side can't reach otherwise:

| Shim | Wraps | Compiled by |
|---|---|---|
| `system_audio_shim.{h,m}` | `ScreenCaptureKit` (`SCStream`, audio-only capture) | `build.rs`'s `build_system_audio_shim()` |
| `permissions_shim.{h,m}` | `AVCaptureDevice` mic-authorization status + request prompt | `build.rs`'s `build_permissions_shim()` |

(The third shim this project once had, `vibrancy_mask_shim.{h,m}`, was retired 2026-09-01 — all four windows now use `window-vibrancy`'s `apply_liquid_glass` instead. See [`explanation-glass-shell.md`](explanation-glass-shell.md).)

## Frontend module map (`frontend/src/`)

| Path | Owns |
|---|---|
| `windows/dashboard/` | The settings/metrics window — `App.tsx` (shell), `panels/Stats.tsx`, `panels/History.tsx`, `panels/Settings.tsx` |
| `windows/pill/` | The floating recording HUD — `Pill.tsx`, one component, four states (`loading`/`listening`/`canceling`/`done`) |
| `windows/recovery/` | The migration-failure recovery screen — shown only when `HistoryStore::open()` fails |
| `windows/onboarding/` | The first-run 2-step flow (`Welcome` → `Ready`) — `Ready` auto-fires all three permission requests sequentially on mount instead of requiring a per-permission Grant click |
| `components/` | Shared UI: `GlassPanel` (the material system), `Toggle`, `SettingRow`, `HotkeyCapture`, `ActivityChart`, `StatTile`, `LatencyTable`, `PermissionRow`, `Sidebar`, `TrafficLights`, `ErrorBoundary` |
| `lib/bindings.ts` | **Generated** — typed `invoke` wrappers + every DTO, from `cargo test --lib export_bindings -- --ignored`. Never hand-edit. |
| `lib/hooks.ts` | `usePermissionsQuery` — the one non-trivial shared data-fetching hook |
| `lib/hotkey.ts` | `toSymbols()` — renders a shortcut string as keycap glyphs (`⌘⇧Space`) |
| `styles/globals.css` | The whole design-token system (see [`DESIGN.md`](../DESIGN.md)) plus the `.glass-panel` material |

## The four windows, at a glance

| Window | `tauri.conf.json` label | Shown when | Decorations |
|---|---|---|---|
| Dashboard | `dashboard` | Tray → "Open Dashboard", or after onboarding completes | Custom (`TrafficLights.tsx`) |
| Pill | `pill` | A recording is active (hotkey pressed) | None — a borderless capsule |
| Recovery | `recovery` | `HistoryStore::open()` returned `MigrationFailed` at startup | None — chromeless, `data-tauri-drag-region` for window dragging (migrated off native decorations 2026-09-01) |
| Onboarding | `onboarding` | First run (`AppSettings.onboarding_completed == false`), unless recovery mode wins | None — chromeless, `data-tauri-drag-region` for window dragging (migrated off native decorations 2026-09-01) |

All four are `transparent: true` with real native vibrancy underneath (see [`explanation-glass-shell.md`](explanation-glass-shell.md)) — none of them are ever destroyed once created; they're shown/hidden for the app's whole lifetime (`ActivationPolicy::Accessory`, no Dock icon, menu-bar-only).

## Data flow: one dictation, start to finish

```
hotkey press
  -> hotkey.rs (global shortcut) -> session::SessionHandle::hotkey_pressed
  -> session.rs's run() actor: Phase::Idle -> Listening
       -> spawns capture on its own OS thread (capture/mic.rs)
       -> shows the pill window (session::show_pill)
hotkey press again (or 120s cap, or system stop)
  -> capture.stop() -> 16kHz mono f32 PCM
  -> handed to segment_worker (a separate sequential task):
       engine.transcribe(audio)         -- engine/whisper.rs
         -> grammar.process(text, lang) -- engine/pipeline.rs (Option A, +B if enabled)
         -> injection::insert_at_cursor -- injection.rs (AX, or clipboard+paste fallback)
         -> history.insert(entry)       -- history/mod.rs (SQLite)
  -> pill briefly shows "done", then auto-hides
```

`segment_worker` is a single FIFO queue, not one task per segment — this is what guarantees a multi-segment session (the 120s auto-continue case) inserts text in the order it was actually spoken, even though capture and transcription run concurrently.

## ADR: why `AppleSpeechEngine` was never built

The `TranscriptionEngine` trait was written to support two backends (`WhisperEngine`, and Apple's on-device `SFSpeechRecognizer` via a stubbed `AppleSpeechEngine`) pending a Phase 0 accuracy benchmark. `tests/language_benchmark.rs` measured Whisper Small at 100% accuracy on English (matching Medium, 3x faster) — English being the only language still in v1 scope after Yoruba/Spanish/Italian/French/Arabic were parked. With the benchmark resolved in Whisper's favor and no second language left to make the case for a fallback engine, `AppleSpeechEngine` stayed a stub by design, not by omission — see `TODOS.md` and `docs/mutter-project-plan.md` Section 6/17 for the full record.

## Related

- [`reference-commands.md`](reference-commands.md) — every Tauri IPC command
- [`reference-settings.md`](reference-settings.md) — the persisted settings schema
- [`reference-history-schema.md`](reference-history-schema.md) — the SQLite schema and aggregates
- [`explanation-session-orchestration.md`](explanation-session-orchestration.md) — why the actor/channel design, and the cancel/auto-continue edge cases
- [`explanation-glass-shell.md`](explanation-glass-shell.md) — the native window-material mechanism
- [`explanation-permission-gate.md`](explanation-permission-gate.md) — why one generic `PermissionGate<T>`
- [`explanation-grammar-pipeline.md`](explanation-grammar-pipeline.md) — Option A/B and why cleanup ended up always-on
- [`explanation-non-speech-hallucination-fix.md`](explanation-non-speech-hallucination-fix.md) — why transcripts don't say "[BLANK_AUDIO]" anymore
- [`tutorial-getting-started.md`](tutorial-getting-started.md) — build and run this from scratch
