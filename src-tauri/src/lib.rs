pub mod cancel;
pub mod capture;
pub mod engine;
pub mod history;
pub mod hotkey;
pub mod injection;
pub mod logging;
pub mod paths;
pub mod permissions;

pub fn run() {
    logging::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Menu-bar-only app, no dock icon — set at runtime rather than
            // via Info.plist, per docs/mutter-project-plan.md Section 4
            // ("Menu-bar icon only, no dock presence").
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // TODO(Phase 1): register the tray icon + menu, wire
            // hotkey::register_hotkeys(), and keep both windows hidden
            // until summoned (pill: during a recording cycle; dashboard:
            // when opened from the tray menu).
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running mutter");
}
