// Pill HUD controller. STUB — Phase 1 work.
//
// Will listen for Tauri events from the Rust backend (hotkey.rs / cancel.rs)
// to drive #pill's data-state attribute and the countdown display. No
// backend wiring exists yet; this just documents the intended states.

const STATES = ["loading", "listening", "transcribing", "canceling", "done"];

const STATUS_TEXT = {
  loading: "Loading…",
  listening: "Listening…",
  transcribing: "Transcribing…",
  canceling: "Cancel in",
  done: "Done",
};

function setState(state) {
  if (!STATES.includes(state)) {
    console.warn(`[pill] unknown state: ${state}`);
    return;
  }
  const pill = document.getElementById("pill");
  const status = document.getElementById("pill-status");
  const countdown = document.getElementById("pill-countdown");

  pill.dataset.state = state;
  status.textContent = STATUS_TEXT[state];
  countdown.hidden = state !== "canceling";
}

function setCountdown(secondsRemaining) {
  document.getElementById("pill-countdown").textContent = String(secondsRemaining);
}

// TODO(Phase 1): subscribe to Tauri events, e.g.
//   import { listen } from "@tauri-apps/api/event";
//   await listen("mutter://pill-state", (event) => setState(event.payload.state));
//   await listen("mutter://cancel-countdown", (event) => setCountdown(event.payload.secondsRemaining));
