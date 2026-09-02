# Getting started: build and run Mutter

You'll go from a fresh clone to a real, working dictation — speaking into any text field and watching Mutter transcribe and insert it — running entirely on your own Mac, no cloud calls involved.

## What you'll need

- macOS 14 (Sonoma) or later, on Apple Silicon
- Rust + Cargo (`rustc --version` to check; [rustup.rs](https://rustup.rs) if you don't have it)
- Node.js 20+ (`node --version` to check)
- The Tauri CLI: `cargo install tauri-cli`
- About 500MB of free disk space for the first-run model download

## Step 1: Install dependencies

```bash
git clone https://github.com/brownfrosamurai/mutter.git
cd mutter
npm install --prefix frontend
```

This installs the frontend's npm packages. You don't need to build the frontend separately — the next command does that for you automatically.

## Step 2: Run it

```bash
cd src-tauri
cargo tauri dev
```

The first run compiles the whole Rust dependency tree (whisper.cpp, candle, ScreenCaptureKit bindings) — expect several minutes. Once it finishes, you won't see a window: Mutter is a menu-bar-only app (no Dock icon, `ActivationPolicy::Accessory`). Look for its icon in the menu bar, top-right of your screen.

Click it. You'll see a menu with **Open Dashboard**, **Start Listening**, and **Quit**. Click **Open Dashboard** — a small floating window appears, showing (empty, on a fresh install) Metrics, History, and Settings panels behind a sidebar of icons. That's your first real, visible result: the app is running, real native window vibrancy is rendering, and the whole Rust ↔ React IPC bridge is live.

## Step 3: Grant permissions

Open **Settings** in the dashboard (the gear icon in the sidebar). You'll see three permission rows: Microphone, Accessibility, Screen Recording. Click **Grant** on any of them — each now triggers a real macOS system prompt directly (mic's own `AVCaptureDevice` prompt, Accessibility's "would like to control this computer" trust alert, Screen Recording's capture-access prompt); allow it. If a row shows Denied and the native prompt stops reappearing (real macOS TCC behavior for Accessibility/Screen Recording after repeat denials), an "Open System Settings" link appears next to it as a fallback — use that to add Mutter under Privacy & Security manually. Screen Recording only matters if you plan to use system-audio capture (dictating from a meeting or video) — you can skip it for now.

The first time you launch a fresh build, an **Onboarding** window appears before the dashboard and walks through this same permission sequence automatically — it fires all three requests in order the moment you reach its last step, so you may already see the native prompts before ever opening Settings. Settings' Permissions section (this step) is what you'd use to grant anything skipped or denied during that first run.

Accessibility is what lets Mutter type text directly into whatever app has focus; without it, transcripts still work but land on your clipboard instead of being typed automatically.

## Step 4: Dictate something

Click into any text field — TextEdit, a Notes window, a terminal, whatever's open — then press **⌘⇧Space** (the default mic-dictation hotkey). A small floating pill appears near the bottom of your screen, showing a waveform and an elapsed-time counter: you're recording.

Say something, then press **⌘⇧Space** again to stop. The very first time you do this, the pill switches to a "Warming up engine…" state for a while — Mutter is downloading the Whisper Small speech model (~500MB) and loading it into memory, a one-time cost. Every dictation after this one skips straight to transcription.

Once it's done, the pill briefly shows "Done" and disappears, and your spoken words appear as typed text right where your cursor was.

## What you built

You now have a real, fully local dictation pipeline running: a global hotkey triggers audio capture, Whisper transcribes it on-device, a grammar-cleanup pass tidies the output, and it's typed directly into your focused app — with everything (audio, transcript, model weights) staying on your machine. Open the dashboard's **Metrics** panel to see your session logged: word count, WPM, and a 7-day activity chart, all computed from the same local SQLite history database your dictation just wrote to.

From here:

- [`reference-architecture.md`](reference-architecture.md) — how the pieces you just used actually fit together
- [`reference-settings.md`](reference-settings.md) — the toggles in Settings, and what each one actually does
- [How to add a new Settings toggle](howto-add-a-settings-toggle.md) or [How to add or change a Tauri command](howto-add-a-tauri-command.md) — if you're about to make a change
- `CLAUDE.md` — the project's full running history of decisions and real bugs found along the way; worth skimming before touching anything non-trivial
