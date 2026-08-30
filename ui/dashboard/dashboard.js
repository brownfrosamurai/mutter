// Dashboard controller. STUB — tab switching only, no live data yet
// (Phase 5 work). Metrics/history population requires the local history
// store (Phase 2) and the running-aggregate bookkeeping (Section 8) to
// exist first.

document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));

    tab.classList.add("active");
    document.getElementById(`panel-${tab.dataset.tab}`).classList.add("active");
  });
});

// TODO(Phase 5): populate #metric-time-saved / #metric-total / #metric-wpm
// and #history-list from the Rust backend via Tauri commands, using the
// paginated + running-aggregate approach in docs/mutter-project-plan.md
// Section 8.
