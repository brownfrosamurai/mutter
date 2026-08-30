# Mutter

Local-first, multi-language speech-to-text dictation for macOS. Toggle a hotkey, speak, toggle again — text lands at your cursor. A second mode captures system audio (meetings, videos), Granola-style. Everything runs on-device: no subscriptions, no cloud, no network calls after the model downloads once.

Built for dictating to AI coding agents and general dictation, in English, Yoruba, Spanish, Italian, French, and Arabic.

## Status

Scaffold only — no feature logic implemented yet. See `CLAUDE.md` for current state and next steps.

## Docs

- [`docs/mutter-project-plan.md`](docs/mutter-project-plan.md) — the plan of record: architecture, phased roadmap, test strategy, validation gate.
- [`docs/mutter-idea-dump.md`](docs/mutter-idea-dump.md) — original freeform spec.
- [`DESIGN.md`](DESIGN.md) — visual design tokens for the pill HUD and dashboard.
- [`CLAUDE.md`](CLAUDE.md) — hard constraints and architecture-at-a-glance for anyone (human or AI) working in this repo.

## Requirements

- macOS 14+ (Sonoma or later)
- Rust + Cargo (not yet installed as of this scaffold — see `CLAUDE.md`)
- Node.js (for Tauri's build tooling)

## Development

Not yet runnable — Phase 0 spikes (engine benchmark, pill-window feasibility, ScreenCaptureKit binding) come before a working `cargo tauri dev`.
