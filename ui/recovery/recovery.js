// Recovery-screen controller — see index.html's comment and
// docs/mutter-project-plan.md Section 11 for the migration-failure
// contract this window exists to satisfy.

async function loadRecoveryInfo() {
  const tauri = window.__TAURI__;
  if (!tauri) return;
  try {
    const backupPath = await tauri.core.invoke("get_recovery_info");
    document.getElementById("backup-path").textContent =
      backupPath || "(no backup path available)";
  } catch (err) {
    console.error("[recovery] get_recovery_info failed", err);
  }
}

document.getElementById("quit-btn").addEventListener("click", () => {
  window.__TAURI__?.core.invoke("quit_app");
});

loadRecoveryInfo();
