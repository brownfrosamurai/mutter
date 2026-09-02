# Mutter

Local-first, multi-language speech-to-text dictation for macOS. Toggle a hotkey, speak, toggle again — text lands at your cursor. A second mode captures system audio (meetings, videos), Granola-style. Everything runs on-device: no subscriptions, no cloud, no network calls after the model downloads once.

Built for dictating to AI coding agents and general dictation. v1 ships English; Whisper's multilingual model auto-detects and transcribes other languages too (Yoruba, Spanish, Italian, French, Arabic were in the original scope, now parked pending a human-supplied accuracy benchmark — see `TODOS.md`).

## Status

Working end-to-end: hotkey → capture → transcribe (Whisper, on-device) → grammar cleanup → text injection → local history, across all three windows (dashboard, pill HUD, recovery). See [`CLAUDE.md`](CLAUDE.md) for the full running history of what's built and what's still open.

## Get started

**[`docs/tutorial-getting-started.md`](docs/tutorial-getting-started.md)** — clone, build, run, dictate something. Start here.

## Documentation

- **Tutorial:** [Getting started](docs/tutorial-getting-started.md)
- **How-to guides:** [Add a Settings toggle](docs/howto-add-a-settings-toggle.md) · [Add or change a Tauri command](docs/howto-add-a-tauri-command.md) · [Run the integration tests](docs/howto-run-integration-tests.md) · [Build and sign a release](docs/howto-build-and-sign-a-release.md)
- **Reference:** [Architecture](docs/reference-architecture.md) · [Tauri commands (IPC)](docs/reference-commands.md) · [Settings schema](docs/reference-settings.md) · [History database schema](docs/reference-history-schema.md)
- **Explanation:** [Session orchestration](docs/explanation-session-orchestration.md) · [Native glass-shell windows](docs/explanation-glass-shell.md) · [Permission gates](docs/explanation-permission-gate.md) · [Grammar cleanup pipeline](docs/explanation-grammar-pipeline.md)
- [`docs/mutter-project-plan.md`](docs/mutter-project-plan.md) — the original plan of record: architecture, phased roadmap, test strategy, validation gate
- [`docs/mutter-idea-dump.md`](docs/mutter-idea-dump.md) — original freeform spec
- [`DESIGN.md`](DESIGN.md) — visual design tokens for the pill HUD and dashboard
- [`CLAUDE.md`](CLAUDE.md) — hard constraints and a full dated history of every non-trivial decision made in this repo, for anyone (human or AI) working in it

## Requirements

- macOS 14+ (Sonoma or later), Apple Silicon
- Rust + Cargo
- Node.js 20+

## Development

```bash
npm install --prefix frontend
cd src-tauri && cargo tauri dev
```

See [`docs/tutorial-getting-started.md`](docs/tutorial-getting-started.md) for the full walkthrough, including granting permissions and your first dictation.

Tests: `cd src-tauri && cargo test` (78 fast unit tests). Slower real-inference integration tests are `#[ignore]`d by default — see [How to run the integration tests](docs/howto-run-integration-tests.md).
