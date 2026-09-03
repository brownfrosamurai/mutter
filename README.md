# Mutter

[![CI](https://github.com/brownfrosamurai/mutter/actions/workflows/ci.yml/badge.svg)](https://github.com/brownfrosamurai/mutter/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Local-first, multi-language speech-to-text dictation for macOS. Toggle a hotkey, speak, toggle again — text lands at your cursor. A second mode captures system audio (meetings, videos), Granola-style. Everything runs on-device: no subscriptions, no cloud, no network calls after the model downloads once.

**No notarized release yet.** GitHub Releases (below) are ad-hoc signed, not notarized — macOS Gatekeeper will complain on first launch; right-click → Open past the warning, or build from source instead.

Built for dictating to AI coding agents and general dictation. v1 ships English; Whisper's multilingual model auto-detects and transcribes other languages too (Yoruba, Spanish, Italian, French, Arabic were in the original scope, now parked pending a human-supplied accuracy benchmark — see `TODOS.md`).

## Status

Working end-to-end: hotkey → capture → transcribe (Whisper, on-device) → grammar cleanup → text injection → local history, across all three windows (dashboard, pill HUD, recovery). See [`CLAUDE.md`](CLAUDE.md) for the full running history of what's built and what's still open.

## Get started

**Download a release:** grab the `.dmg` from [Releases](https://github.com/brownfrosamurai/mutter/releases/latest), open it, and drag Mutter to Applications. Since it's ad-hoc signed rather than notarized, macOS will refuse a plain double-click the first time — **right-click Mutter.app → Open**, then confirm in the dialog that appears.

**Or build from source:** **[`docs/tutorial-getting-started.md`](docs/tutorial-getting-started.md)** — clone, build, run, dictate something.

### Granting permissions after install

Mutter needs Microphone, Accessibility, and Screen Recording access to work. On first launch, an **Onboarding** window walks through requesting all three automatically — just click through it and allow each system prompt.

If a prompt doesn't appear, or you need to grant something later, do it manually: click Mutter's icon in the menu bar → **Open Dashboard** → the gear icon (**Settings**) → the **Permissions** section, and click **Grant** next to each row (or **Open System Settings** if the native prompt stops reappearing, which is normal macOS behavior after a repeat denial). Screen Recording is only needed for system-audio capture (dictating from a meeting or video) — you can skip it if you're only using mic dictation.

You can also grant everything directly through macOS itself, without opening Mutter's own Settings: **System Settings → Privacy & Security**, then enable Mutter under each of **Microphone**, **Accessibility**, and **Screen Recording**.

**If you've updated to a newer version and dictation stops working** (mic doesn't respond, text doesn't get typed in), re-grant permissions using either method above — this is expected, not a bug. Each release build is signed ad-hoc rather than with a stable Apple Developer certificate, so macOS treats every new build as a different app and doesn't carry permission grants forward across updates (see `TODOS.md` for the full mechanism). Onboarding won't automatically reappear to prompt you again since it only shows up once per install, so the manual Settings path above is the one to use after updating.

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

## Contributing

Outside contributions are welcome — see [`CONTRIBUTING.md`](CONTRIBUTING.md) for the hard constraints, dev setup, and PR checklist. Please open an issue before starting non-trivial work. Security issues: see [`SECURITY.md`](SECURITY.md), not a public issue.

## License

[MIT](LICENSE)
