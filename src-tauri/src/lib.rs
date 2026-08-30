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
    let rows = store
        .list_page(page, page_size)
        .map_err(|e| e.to_string())?;
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

#[derive(serde::Serialize)]
struct PermissionStatusDto {
    mic: &'static str,
    accessibility: &'static str,
    system_audio: &'static str,
}

fn permission_state_label(state: permissions::PermissionState) -> &'static str {
    match state {
        permissions::PermissionState::NotRequested => "not_requested",
        permissions::PermissionState::Denied => "denied",
        permissions::PermissionState::Granted => "granted",
        permissions::PermissionState::Unavailable => "unavailable",
    }
}

/// Backs the dashboard Settings panel's Permissions row — real OS-queried
/// status (permissions.rs), not the "not yet wired" placeholder it started
/// as.
#[tauri::command]
fn get_permission_status() -> PermissionStatusDto {
    let mut mic = permissions::PermissionGate::<permissions::Mic>::new();
    mic.refresh();
    let mut accessibility = permissions::PermissionGate::<permissions::Accessibility>::new();
    accessibility.refresh();
    let mut system_audio = permissions::PermissionGate::<permissions::SystemAudio>::new();
    system_audio.refresh();

    PermissionStatusDto {
        mic: permission_state_label(mic.state()),
        accessibility: permission_state_label(accessibility.state()),
        system_audio: permission_state_label(system_audio.state()),
    }
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

/// Set only when `HistoryStore::open()` returned `MigrationFailed` — holds
/// the backup path the recovery window (Section 11) names. `None` in the
/// normal case, and also in the "some other database error" case (e.g. the
/// app-support directory couldn't be created), which the plan doesn't
/// single out for the recovery screen — that's handled by the existing
/// graceful-degradation path (log + disable session, app still opens).
struct RecoveryInfo(Option<String>);

#[tauri::command]
fn get_recovery_info(state: tauri::State<RecoveryInfo>) -> Option<String> {
    state.inner().0.clone()
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
            get_permission_status,
            quit_app,
            get_recovery_info
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
            // the backup path" — handled below by showing the `recovery`
            // window instead of ever registering hotkeys or showing the
            // pill/dashboard. A non-migration open failure (e.g. the
            // app-support directory itself couldn't be created) isn't
            // covered by that specific contract, so it keeps the milder
            // fallback: log loudly, disable dictation for the session, but
            // let the tray + dashboard still come up.
            let (session_handle, history_for_dashboard, recovery_info): (
                Option<session::SessionHandle>,
                Option<Arc<history::HistoryStore>>,
                RecoveryInfo,
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

                    (Some(handle), Some(history), RecoveryInfo(None))
                }
                Err(history::HistoryError::MigrationFailed(backup_path)) => {
                    tracing::error!(
                        backup_path = %backup_path,
                        "history db migration failed — refusing to launch normally, showing recovery screen"
                    );
                    if let Some(win) = app.get_webview_window("pill") {
                        let _ = win.close();
                    }
                    if let Some(win) = app.get_webview_window("dashboard") {
                        let _ = win.close();
                    }
                    if let Some(win) = app.get_webview_window("recovery") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                    (None, None, RecoveryInfo(Some(backup_path)))
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "history store failed to open — dictation disabled this session"
                    );
                    (None, None, RecoveryInfo(None))
                }
            };
            let in_recovery = recovery_info.0.is_some();
            app.manage(session_handle);
            app.manage(history_for_dashboard);
            app.manage(recovery_info);

            // The dashboard window is meant to persist for the app's whole
            // lifetime and be shown/hidden (via the tray's "Open Dashboard"
            // and this window's own custom close button — see dashboard.js),
            // never actually destroyed — there's no code path that recreates
            // it once gone. Without this, the default CloseRequested
            // behavior would destroy it on the first Cmd+W (or, before the
            // custom titlebar existed, the native red traffic light), and
            // "Open Dashboard" would silently find nothing to show for the
            // rest of the session. No-ops harmlessly if the window was
            // already closed above (recovery mode).
            if let Some(dashboard) = app.get_webview_window("dashboard") {
                let dashboard_for_hide = dashboard.clone();
                dashboard.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = dashboard_for_hide.hide();
                    }
                });
            }

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
                tray.on_menu_event(move |app, event| {
                    if event.id() == "open_dashboard" {
                        // In recovery mode the dashboard window was closed
                        // above and has no working history store behind
                        // it anyway — route back to the recovery screen
                        // instead of trying (and failing) to open it.
                        let label = if in_recovery { "recovery" } else { "dashboard" };
                        if let Some(win) = app.get_webview_window(label) {
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
