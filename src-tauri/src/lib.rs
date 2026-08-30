pub mod cancel;
pub mod capture;
pub mod engine;
pub mod history;
pub mod hotkey;
pub mod injection;
pub mod logging;
pub mod paths;
pub mod permissions;
pub mod session;

use std::sync::Arc;

use tauri::menu::MenuBuilder;
use tauri::Manager;

/// Identifies the concrete `TranscriptionEngine` in history rows and logs —
/// see `session::spawn`'s `engine_name` parameter. Not the model tier
/// (`ModelTier::Small`/`Medium`, engine::whisper) — just which engine.
const DEFAULT_ENGINE_NAME: &str = "whisper-small";

/// Manual equivalent of pressing the global Escape hotkey — the pill's
/// cancel button (`#pill-cancel`) invokes this, since a webview button click
/// can't itself register as a global-shortcut key-press.
#[tauri::command]
fn cancel_recording(state: tauri::State<Option<session::SessionHandle>>) -> Result<(), String> {
    match state.inner() {
        Some(handle) => {
            handle.escape_pressed();
            Ok(())
        }
        None => Err("session unavailable — history store failed to open at startup".into()),
    }
}

pub fn run() {
    logging::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![cancel_recording])
        .setup(|app| {
            // Menu-bar-only app, no dock icon — set at runtime rather than
            // via Info.plist, per docs/mutter-project-plan.md Section 4
            // ("Menu-bar icon only, no dock presence").
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let app_handle = app.handle().clone();

            // The core loop (hotkey -> capture -> engine -> injection ->
            // history) only comes up if the history store opens cleanly.
            // Section 11's documented contract on a migration failure is
            // "refuse to launch normally and show a recovery screen naming
            // the backup path" — the recovery *screen* isn't built yet
            // (real UI work, not wired here); today a failure is logged
            // loudly and the session simply doesn't start, so the app
            // still opens (tray + dashboard) without silently pretending
            // dictation works.
            let session_handle: Option<session::SessionHandle> = match history::HistoryStore::open()
            {
                Ok(store) => {
                    let history = Arc::new(store);
                    let engine: Arc<dyn engine::TranscriptionEngine> =
                        Arc::new(engine::whisper::WhisperEngine::new());
                    let grammar: Arc<dyn engine::TextProcessor> =
                        Arc::new(engine::grammar::RuleBasedCleanup);

                    let handle = session::spawn(
                        app_handle.clone(),
                        engine,
                        grammar,
                        history,
                        DEFAULT_ENGINE_NAME,
                    );

                    let hotkey_handle = handle.clone();
                    hotkey::register_hotkeys(&app_handle, move |mode| {
                        hotkey_handle.hotkey_pressed(mode);
                    })?;

                    Some(handle)
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "history store failed to open — dictation disabled this session"
                    );
                    None
                }
            };
            app.manage(session_handle);

            // tauri.conf.json's `app.trayIcon` already creates the tray
            // icon itself at startup (id "main") — this attaches the menu
            // and click handling to it.
            let menu = MenuBuilder::new(app)
                .text("open_dashboard", "Open Dashboard")
                .separator()
                .quit()
                .build()?;
            if let Some(tray) = app.tray_by_id("main") {
                tray.set_menu(Some(menu))?;
                tray.on_menu_event(|app, event| {
                    if event.id() == "open_dashboard" {
                        if let Some(win) = app.get_webview_window("dashboard") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running mutter");
}
