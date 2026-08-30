// Pill HUD controller. STUB — Phase 1 work.
//
// Will listen for Tauri events from the Rust backend (hotkey.rs / cancel.rs)
// to drive #pill's data-state attribute, elapsed timer, and cancel
// countdown. No backend wiring exists yet; this just documents the intended
// states and drives the static markup for visual QA.

const STATES = ["loading", "listening", "transcribing", "canceling", "done"];

const STATUS_TEXT = {
  loading: "Warming up engine…",
  listening: "Listening…",
  transcribing: "Whisper processing…",
  canceling: "Stopping recording",
  done: "Done",
};

const SUBTEXT = {
  loading: "Initial lazy-load latency (once per session)",
  transcribing: "large-v3 (locally)",
};

function setState(state) {
  if (!STATES.includes(state)) {
    console.warn(`[pill] unknown state: ${state}`);
    return;
  }
  const pill = document.getElementById("pill");
  const status = document.getElementById("pill-status");
  const subtext = document.getElementById("pill-subtext");
  const countdown = document.getElementById("pill-countdown");

  pill.dataset.state = state;
  status.textContent = STATUS_TEXT[state];

  const sub = SUBTEXT[state];
  subtext.textContent = sub ?? "";
  subtext.hidden = !sub;

  countdown.hidden = state !== "canceling";
}

function setElapsed(totalSeconds) {
  const m = Math.floor(totalSeconds / 60);
  const s = totalSeconds % 60;
  document.getElementById("pill-timer").textContent = `${m}:${String(s).padStart(2, "0")}`;
}

function setCountdown(secondsRemaining) {
  document.getElementById("pill-countdown").textContent = String(secondsRemaining);
}

// TODO(Phase 1): subscribe to Tauri events, e.g.
//   import { listen } from "@tauri-apps/api/event";
//   await listen("mutter://pill-state", (event) => setState(event.payload.state));
//   await listen("mutter://elapsed-seconds", (event) => setElapsed(event.payload.seconds));
//   await listen("mutter://cancel-countdown", (event) => setCountdown(event.payload.secondsRemaining));
// TODO(Phase 1): wire #pill-pause / #pill-cancel click handlers to
//   Tauri commands once the cancel state machine (cancel.rs) is invokable
//   from the frontend.
