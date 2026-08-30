// Dashboard controller. STUB — tab switching only, no live data yet
// (Phase 5 work). Metrics/history population requires the local history
// store (Phase 2) and the running-aggregate bookkeeping (Section 8) to
// exist first.

const PANEL_TITLE = {
  metrics: "Stats",
  history: "History",
  settings: "Settings",
};

document.querySelectorAll(".nav-btn[data-tab]").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".nav-btn[data-tab]").forEach((b) => b.classList.remove("active"));
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));

    btn.classList.add("active");
    document.getElementById(`panel-${btn.dataset.tab}`).classList.add("active");
    document.getElementById("panel-title").textContent = PANEL_TITLE[btn.dataset.tab];
  });
});

// TODO(Phase 1): wire #quit-btn to the Tauri process-exit command once the
// tray/menu-bar shell (lib.rs setup) registers it.

// TODO(Phase 5): populate #metric-sessions / #metric-words /
// #metric-time-saved, #language-list, #latency-table, and #history-list from
// the Rust backend via Tauri commands, using the paginated + running-
// aggregate approach in docs/mutter-project-plan.md Section 8.
