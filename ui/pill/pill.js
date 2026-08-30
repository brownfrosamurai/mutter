// Pill HUD controller.
//
// Listens for Tauri events emitted by session.rs to drive #pill's
// data-state attribute, elapsed timer, and cancel countdown. Uses the
// global `window.__TAURI__` API (tauri.conf.json's `app.withGlobalTauri`)
// rather than an ES module import — this app has no bundler/build step by
// design (CLAUDE.md: plain HTML/CSS/vanilla JS, no framework).

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
  transcribing: "whisper-small (locally)",
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

function connectToBackend() {
  const tauri = window.__TAURI__;
  if (!tauri) {
    // Not running inside the Tauri webview (e.g. opened as a plain file
    // for visual QA) — leave the static markup as-is.
    console.warn("[pill] window.__TAURI__ not present; running without backend events");
    return;
  }

  tauri.event.listen("mutter://pill-state", (event) => setState(event.payload.state));
  tauri.event.listen("mutter://elapsed-seconds", (event) => setElapsed(event.payload.seconds));
  tauri.event.listen("mutter://cancel-countdown", (event) =>
    setCountdown(event.payload.secondsRemaining),
  );

  document.getElementById("pill-cancel").addEventListener("click", () => {
    tauri.core.invoke("cancel_recording").catch((err) => {
      console.error("[pill] cancel_recording failed", err);
    });
  });

  // #pill-pause has no backend action yet — the current design only has a
  // full toggle-stop and an Escape-driven cancel (see session.rs), no
  // separate pause/resume-while-listening concept. Left as a visual
  // placeholder matching the reference mockup rather than wired to
  // nothing/a fake action.
}

connectToBackend();
