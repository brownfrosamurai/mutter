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

// --- Dashboard IPC (Phase 5, Section 8) ---
//
// DTOs kept separate from the `history` module's own types rather than
// deriving `Serialize` on them directly — the wire shape (camelCase-ish
// field names the frontend expects) is a UI concern, not a storage-layer
// one, and this keeps `history::HistoryEntry`/`Metrics` free of a
// dependency on `serde` they don't otherwise need.

#[derive(serde::Serialize)]
struct MetricsDto {
    sessions: i64,
    words: i64,
    time_saved_minutes: f64,
    average_wpm: f64,
}

#[derive(serde::Serialize)]
struct LanguageStatDto {
    language: String,
    count: i64,
}

#[derive(serde::Serialize)]
struct HistoryEntryDto {
    timestamp: i64,
    duration_secs: f64,
    text: String,
    language: String,
    engine: String,
}

#[tauri::command]
fn get_metrics(
    state: tauri::State<Option<Arc<history::HistoryStore>>>,
) -> Result<MetricsDto, String> {
    let store = state.inner().as_ref().ok_or("history store unavailable")?;
    let m = store
        .metrics(history::DEFAULT_TYPING_WPM)
        .map_err(|e| e.to_string())?;
    Ok(MetricsDto {
        sessions: m.total_transcriptions,
        words: m.total_word_count,
        time_saved_minutes: m.time_saved_minutes,
        average_wpm: m.average_wpm,
    })
}

#[tauri::command]
fn get_language_breakdown(
    state: tauri::State<Option<Arc<history::HistoryStore>>>,
) -> Result<Vec<LanguageStatDto>, String> {
    let store = state.inner().as_ref().ok_or("history store unavailable")?;
    let rows = store.language_breakdown().map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(language, count)| LanguageStatDto { language, count })
        .collect())
}

#[tauri::command]
fn get_history_page(
    page: u32,
    page_size: u32,
    state: tauri::State<Option<Arc<history::HistoryStore>>>,
) -> Result<Vec<HistoryEntryDto>, String> {
    let store = state.inner().as_ref().ok_or("history store unavailable")?;
    let rows = store.list_page(page, page_size).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|e| HistoryEntryDto {
            timestamp: e.timestamp,
            duration_secs: e.duration_secs,
            text: e.text,
            language: e.language,
            engine: e.engine,
        })
        .collect())
}

/// Backs the history list's "copy" button — Section 8: "doubles as the
/// copy-and-paste-at-any-time recovery mechanism".
#[tauri::command]
fn copy_history_text(text: String) -> Result<(), String> {
    injection::copy_to_clipboard(&text).map_err(|e| e.to_string())
}

/// Backs the dashboard sidebar's quit button. The tray menu's predefined
/// "Quit" item (see `run()`) is the primary quit path; this is the same
/// action reachable from the dashboard itself, per the reference mockup's
/// sidebar layout — not worth adding the separate `tauri-plugin-process`
/// dependency for one button when `AppHandle::exit` already does this.
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

pub fn run() {
    logging::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            cancel_recording,
            get_metrics,
            get_language_breakdown,
            get_history_page,
            copy_history_text,
            quit_app
        ])
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
            let (session_handle, history_for_dashboard): (
                Option<session::SessionHandle>,
                Option<Arc<history::HistoryStore>>,
            ) = match history::HistoryStore::open() {
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
                        history.clone(),
                        DEFAULT_ENGINE_NAME,
                    );

                    let hotkey_handle = handle.clone();
                    hotkey::register_hotkeys(&app_handle, move |mode| {
                        hotkey_handle.hotkey_pressed(mode);
                    })?;

                    (Some(handle), Some(history))
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "history store failed to open — dictation disabled this session"
                    );
                    (None, None)
                }
            };
            app.manage(session_handle);
            app.manage(history_for_dashboard);

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
