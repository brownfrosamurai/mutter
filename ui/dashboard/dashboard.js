// Dashboard controller. Tab switching plus real data fetched from the Rust
// backend (Phase 5) via Tauri commands, using window.__TAURI__ (no
// bundler/ES-module import — see pill.js for the same reasoning).

const PANEL_TITLE = {
  metrics: "Stats",
  history: "History",
  settings: "Settings",
};

const HISTORY_PAGE_SIZE = 50;

document.querySelectorAll(".nav-btn[data-tab]").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".nav-btn[data-tab]").forEach((b) => b.classList.remove("active"));
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));

    btn.classList.add("active");
    document.getElementById(`panel-${btn.dataset.tab}`).classList.add("active");
    document.getElementById("panel-title").textContent = PANEL_TITLE[btn.dataset.tab];

    if (btn.dataset.tab === "metrics") loadMetrics();
    if (btn.dataset.tab === "history") loadHistory();
    if (btn.dataset.tab === "settings") loadPermissionStatus();
  });
});

const PERMISSION_LABEL = {
  granted: "Granted",
  denied: "Denied — enable in System Settings",
  not_requested: "Not yet requested",
  unavailable: "Unavailable on this device",
};

async function loadPermissionStatus() {
  const tauri = window.__TAURI__;
  if (!tauri) return;
  try {
    const status = await tauri.core.invoke("get_permission_status");
    document.getElementById("permission-mic").textContent =
      PERMISSION_LABEL[status.mic] ?? status.mic;
    document.getElementById("permission-accessibility").textContent =
      PERMISSION_LABEL[status.accessibility] ?? status.accessibility;
    document.getElementById("permission-system-audio").textContent =
      PERMISSION_LABEL[status.system_audio] ?? status.system_audio;
  } catch (err) {
    console.warn("[dashboard] get_permission_status failed", err);
  }
}

function formatMinutes(minutes) {
  const sign = minutes < 0 ? "-" : "";
  const abs = Math.abs(minutes);
  if (abs < 60) return `${sign}${Math.round(abs)}m`;
  return `${sign}${(abs / 60).toFixed(1)}h`;
}

function formatDuration(seconds) {
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

async function loadMetrics() {
  const tauri = window.__TAURI__;
  if (!tauri) return;

  try {
    const metrics = await tauri.core.invoke("get_metrics");
    document.getElementById("metric-sessions").textContent = String(metrics.sessions);
    document.getElementById("metric-sessions-sub").textContent =
      metrics.sessions === 0 ? "No data yet" : `${metrics.sessions} sessions total`;

    document.getElementById("metric-words").textContent = metrics.words.toLocaleString();
    document.getElementById("metric-words-sub").textContent =
      metrics.words === 0 ? "No data yet" : `~${Math.round(metrics.average_wpm)} WPM average`;

    document.getElementById("metric-time-saved").textContent = formatMinutes(
      metrics.time_saved_minutes,
    );
    document.getElementById("metric-time-saved-sub").textContent =
      metrics.sessions === 0 ? "No data yet" : "vs. typing, at 40 WPM assumed";
  } catch (err) {
    console.warn("[dashboard] get_metrics failed", err);
  }

  try {
    const languages = await tauri.core.invoke("get_language_breakdown");
    renderLanguages(languages);
  } catch (err) {
    console.warn("[dashboard] get_language_breakdown failed", err);
  }
}

function renderLanguages(languages) {
  const list = document.getElementById("language-list");
  if (!languages || languages.length === 0) {
    list.innerHTML = '<p class="hint">No data yet.</p>';
    return;
  }

  const maxCount = Math.max(...languages.map((l) => l.count));
  list.innerHTML = "";
  for (const { language, count } of languages) {
    const row = document.createElement("div");
    row.className = "language-row";

    const name = document.createElement("span");
    name.className = "language-name";
    name.textContent = language;

    const track = document.createElement("div");
    track.className = "language-bar-track";
    const fill = document.createElement("div");
    fill.className = "language-bar-fill";
    fill.style.width = `${(count / maxCount) * 100}%`;
    track.appendChild(fill);

    const countEl = document.createElement("span");
    countEl.className = "language-count";
    countEl.textContent = String(count);

    row.append(name, track, countEl);
    list.appendChild(row);
  }
}

async function loadHistory() {
  const tauri = window.__TAURI__;
  const list = document.getElementById("history-list");
  const hint = document.getElementById("history-hint");
  if (!tauri) return;

  try {
    const entries = await tauri.core.invoke("get_history_page", {
      page: 0,
      pageSize: HISTORY_PAGE_SIZE,
    });

    if (!entries || entries.length === 0) {
      list.innerHTML = "";
      hint.hidden = false;
      hint.textContent = "No dictations yet.";
      return;
    }

    hint.hidden = true;
    list.innerHTML = "";
    for (const entry of entries) {
      list.appendChild(renderHistoryRow(entry));
    }
  } catch (err) {
    console.warn("[dashboard] get_history_page failed", err);
    hint.hidden = false;
    hint.textContent = "Could not load history.";
  }
}

function renderHistoryRow(entry) {
  const li = document.createElement("li");
  li.className = "history-row";

  const text = document.createElement("span");
  text.className = "history-text";
  text.textContent = entry.text;
  text.title = entry.text;

  const meta = document.createElement("span");
  meta.className = "history-meta";
  meta.textContent = `${formatDuration(entry.duration_secs)} · ${entry.language}`;

  const copyBtn = document.createElement("button");
  copyBtn.className = "history-copy-btn";
  copyBtn.type = "button";
  copyBtn.textContent = "Copy";
  copyBtn.addEventListener("click", () => {
    window.__TAURI__.core
      .invoke("copy_history_text", { text: entry.text })
      .then(() => {
        copyBtn.textContent = "Copied";
        setTimeout(() => (copyBtn.textContent = "Copy"), 1200);
      })
      .catch((err) => console.error("[dashboard] copy_history_text failed", err));
  });

  li.append(text, meta, copyBtn);
  return li;
}

document.getElementById("quit-btn").addEventListener("click", () => {
  window.__TAURI__?.core.invoke("quit_app");
});

// Custom titlebar controls (tauri.conf.json has decorations: false for this
// window — see index.html's #panel-header comment) — window.__TAURI__.window
// is core, not a plugin, so this needs no extra Cargo dependency.
function initTitlebarControls() {
  const tauri = window.__TAURI__;
  if (!tauri) return;
  const appWindow = tauri.window.getCurrentWindow();

  // Hide, not close: this window is reused for the app's lifetime (shown
  // again via the tray's "Open Dashboard"), matching how the tray/toggle
  // hotkey already show/hide the pill rather than destroying it. lib.rs
  // also intercepts the native CloseRequested event the same way, as a
  // fallback for any close path other than this button (e.g. Cmd+W).
  document.getElementById("win-close").addEventListener("click", () => appWindow.hide());
  document.getElementById("win-minimize").addEventListener("click", () => appWindow.minimize());
  document.getElementById("win-maximize").addEventListener("click", () => appWindow.toggleMaximize());
}
initTitlebarControls();

// Initial load — the dashboard opens on the Metrics tab.
loadMetrics();
