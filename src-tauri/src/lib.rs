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
pub mod settings;
pub mod vibrancy;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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
#[specta::specta]
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

#[derive(serde::Serialize, specta::Type)]
struct MetricsDto {
    sessions: i64,
    words: i64,
    time_saved_minutes: f64,
    average_wpm: f64,
    total_dictation_minutes: f64,
}

#[derive(serde::Serialize, specta::Type)]
struct LanguageStatDto {
    language: String,
    count: i64,
    average_wpm: f64,
}

#[derive(serde::Serialize, specta::Type)]
struct DailyActivityDto {
    date: String,
    count: i64,
}

#[derive(serde::Serialize, specta::Type)]
struct HistoryEntryDto {
    timestamp: i64,
    duration_secs: f64,
    text: String,
    language: String,
    engine: String,
}

/// Wire-format twin of `history::LatencyPercentiles`.
#[derive(serde::Serialize, specta::Type)]
struct LatencyPercentilesDto {
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    samples: i64,
}

/// Wire-format twin of `history::LatencyStats` — backs the Stats page's
/// Latency table (frontend-rewrite plan, 2026-08-31).
#[derive(serde::Serialize, specta::Type)]
struct LatencyStatsDto {
    recording: LatencyPercentilesDto,
    inference: LatencyPercentilesDto,
}

impl From<history::LatencyPercentiles> for LatencyPercentilesDto {
    fn from(p: history::LatencyPercentiles) -> Self {
        Self {
            p50_ms: p.p50_ms,
            p95_ms: p.p95_ms,
            samples: p.samples,
        }
    }
}

/// Same trailing-window length as the Activity chart (`get_daily_activity`'s
/// existing `days` param, hardcoded to 14 on the frontend) — the Latency
/// table and Activity chart are the same "last 14 days" window on the same
/// Stats page, so there's no reason for them to silently disagree.
const LATENCY_WINDOW_DAYS: u32 = 14;

#[tauri::command]
#[specta::specta]
fn get_latency_stats(
    state: tauri::State<Option<Arc<history::HistoryStore>>>,
) -> Result<LatencyStatsDto, String> {
    let store = state.inner().as_ref().ok_or("history store unavailable")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let stats = store
        .latency_stats(LATENCY_WINDOW_DAYS, now)
        .map_err(|e| e.to_string())?;
    Ok(LatencyStatsDto {
        recording: stats.recording.into(),
        inference: stats.inference.into(),
    })
}

#[tauri::command]
#[specta::specta]
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
        total_dictation_minutes: m.total_dictation_minutes,
    })
}

#[tauri::command]
#[specta::specta]
fn get_language_breakdown(
    state: tauri::State<Option<Arc<history::HistoryStore>>>,
) -> Result<Vec<LanguageStatDto>, String> {
    let store = state.inner().as_ref().ok_or("history store unavailable")?;
    let rows = store.language_breakdown().map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|s| LanguageStatDto {
            language: s.language,
            count: s.count,
            average_wpm: s.average_wpm,
        })
        .collect())
}

/// Backs the Stats page's activity chart (2026-08-30 redesign) — see
/// `HistoryStore::daily_activity`'s doc comment for why this is a real
/// backend aggregate rather than the frontend bucketing raw history pages.
#[tauri::command]
#[specta::specta]
fn get_daily_activity(
    days: u32,
    state: tauri::State<Option<Arc<history::HistoryStore>>>,
) -> Result<Vec<DailyActivityDto>, String> {
    let store = state.inner().as_ref().ok_or("history store unavailable")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let rows = store.daily_activity(days, now).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|d| DailyActivityDto {
            date: d.date,
            count: d.count,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
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
#[specta::specta]
fn copy_history_text(text: String) -> Result<(), String> {
    injection::copy_to_clipboard(&text).map_err(|e| e.to_string())
}

#[derive(serde::Serialize, specta::Type)]
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
#[specta::specta]
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

/// Which permission a Grant button targets — one enum instead of a
/// stringly-typed `kind: String` (matches `SettingField`'s own established
/// pattern, D3 from the frontend-rewrite plan). Shared by the onboarding
/// window's Permissions step and the dashboard's Settings panel.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    Microphone,
    Accessibility,
    ScreenRecording,
}

/// Deep-links to the matching System Settings pane for Accessibility and
/// Screen Recording (macOS has no active-request API for either — unlike
/// mic, see `request_mic_access` below). Also the fallback path for mic
/// once its one-shot native prompt (`request_mic_access`) has already been
/// answered, since that prompt won't show again.
///
/// `async` + `spawn_blocking`, matching `request_mic_access` right below —
/// a plain sync `#[tauri::command]` runs on the same thread that dispatches
/// the IPC message (the main/UI thread for the default wry/WKWebView
/// backend), so `Command::status()`'s blocking wait for `open` to launch
/// would otherwise stall the UI for however long that takes (review finding,
/// caught by inconsistency with the sibling command below it).
#[tauri::command]
#[specta::specta]
async fn open_permission_settings(kind: PermissionKind) -> Result<(), String> {
    let url = match kind {
        PermissionKind::Microphone => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        PermissionKind::Accessibility => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        PermissionKind::ScreenRecording => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|e| format!("failed to open System Settings: {e}"))
    })
    .await
    .map_err(|e| format!("open-settings task panicked: {e}"))??;
    Ok(())
}

/// Shows the real native macOS mic-permission prompt (Outside Voice finding
/// #2 from the onboarding-flow plan review — mic gets an active request,
/// unlike Accessibility/Screen Recording, which stay on
/// `open_permission_settings` above). Blocking, so it's dispatched onto
/// Tauri's blocking-task pool rather than the async command's own task —
/// never call the underlying native function from the main thread.
#[tauri::command]
#[specta::specta]
async fn request_mic_access() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut gate = permissions::PermissionGate::<permissions::Mic>::new();
        gate.request()
    })
    .await
    .map_err(|e| format!("mic permission request task panicked: {e}"))
}

/// Marks onboarding as finished — persisted so it never shows again — then
/// closes the onboarding window and shows the dashboard. Awaited by the
/// frontend with its "Open Dashboard" button disabled while pending, so a
/// `settings.json` save failure (disk full, permissions) surfaces as a
/// real error instead of silently leaving `onboarding_completed=false`
/// (which would otherwise only be discoverable as "onboarding weirdly
/// reappears next launch").
#[tauri::command]
#[specta::specta]
fn complete_onboarding(
    app: tauri::AppHandle,
    settings_state: tauri::State<Mutex<settings::AppSettings>>,
) -> Result<(), String> {
    {
        let mut settings = settings_state
            .inner()
            .lock()
            .expect("settings lock poisoned");
        settings.onboarding_completed = true;
        settings
            .save()
            .map_err(|e| format!("onboarding finished but failed to persist to disk: {e}"))?;
    }
    if let Some(win) = app.get_webview_window("onboarding") {
        let _ = win.close();
    }
    reveal_dashboard(&app);
    if let Some(win) = app.get_webview_window("dashboard") {
        let _ = win.set_focus();
    }
    Ok(())
}

/// The onboarding-vs-recovery priority decision from `setup()`, pulled out
/// as a pure function so it's unit-testable without standing up a real
/// `tauri::App` — recovery mode must always win, regardless of
/// `onboarding_completed`, so a fresh install whose history DB is somehow
/// already corrupt shows recovery, never onboarding (review finding: this
/// contract was documented in comments and in the onboarding-flow plan's
/// Implementation Tasks checklist, but never actually had a test).
fn should_show_onboarding(in_recovery: bool, onboarding_completed: bool) -> bool {
    !in_recovery && !onboarding_completed
}

#[cfg(test)]
mod onboarding_tests {
    use super::should_show_onboarding;

    #[test]
    fn shows_onboarding_only_when_not_in_recovery_and_not_completed() {
        assert!(should_show_onboarding(false, false));
    }

    #[test]
    fn recovery_mode_wins_even_if_onboarding_was_never_completed() {
        assert!(!should_show_onboarding(true, false));
    }

    #[test]
    fn recovery_mode_wins_even_if_onboarding_was_already_completed() {
        assert!(!should_show_onboarding(true, true));
    }

    #[test]
    fn already_completed_onboarding_never_shows_again() {
        assert!(!should_show_onboarding(false, true));
    }
}

/// Backs the dashboard sidebar's quit button. The tray menu's predefined
/// "Quit" item (see `run()`) is the primary quit path; this is the same
/// action reachable from the dashboard itself, per the reference mockup's
/// sidebar layout — not worth adding the separate `tauri-plugin-process`
/// dependency for one button when `AppHandle::exit` already does this.
#[tauri::command]
#[specta::specta]
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
#[specta::specta]
fn get_recovery_info(state: tauri::State<RecoveryInfo>) -> Option<String> {
    state.inner().0.clone()
}

/// The live, read-every-call flag `engine::pipeline::GrammarPipeline`
/// checks — a separate managed type from `Mutex<settings::AppSettings>`
/// (below) even though `AppSettings` also carries this same value, because
/// the pipeline needs a cheap atomic read on the hot transcription path,
/// not a mutex lock shared with hotkey-configuration commands.
/// `set_grammar_llm_cleanup_enabled` below keeps both in sync.
struct GrammarLlmCleanupFlag(Arc<AtomicBool>);

/// The tray's "Start Listening"/"Stop Listening" item, retained as managed
/// state so `session::update_tray_listening_indicator` can mutate its label
/// and icon from wherever the session actor emits a pill-state change —
/// see that function's own docs.
pub(crate) struct ListeningMenuItem<R: tauri::Runtime>(pub tauri::menu::IconMenuItem<R>);

/// Small filled danger-red dot — `IconMenuItem`'s icon while a recording is
/// actually in progress, matching `--danger` (`#ff453a`) used everywhere
/// else in the app for "this is the destructive/state-changing one" (the
/// sidebar's Quit button, Recovery's Quit button). No icon at all
/// (`set_icon(None)`) in every other state — presence of the icon, not just
/// the text, is the signal.
///
/// Built as a raw RGBA buffer rather than a bundled PNG asset — `Image::
/// from_bytes` needs tauri's `image-png` feature (a real `image`-crate PNG
/// decoder dependency) for what's otherwise just a solid-color circle;
/// computing the 16x16 buffer directly avoids that dependency entirely.
pub(crate) fn tray_listening_danger_icon() -> tauri::image::Image<'static> {
    const SIZE: u32 = 16;
    const CENTER: f32 = (SIZE as f32 - 1.0) / 2.0;
    const RADIUS: f32 = SIZE as f32 / 2.0 - 1.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - CENTER;
            let dy = y as f32 - CENTER;
            if (dx * dx + dy * dy).sqrt() <= RADIUS {
                rgba.extend_from_slice(&[0xff, 0x45, 0x3a, 0xff]); // --danger, opaque
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]); // transparent
            }
        }
    }
    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

#[tauri::command]
#[specta::specta]
fn get_settings(state: tauri::State<Mutex<settings::AppSettings>>) -> settings::AppSettings {
    state
        .inner()
        .lock()
        .expect("settings lock poisoned")
        .clone()
}

/// Backs the dashboard's hotkey-configuration rows (Section 4: "hotkey
/// configuration (separate bindings for mic-dictation mode and system-audio
/// mode)"). `mode` is `"mic"` or `"system_audio"` — a plain string over the
/// wire rather than reusing `hotkey::HotkeyMode` directly, since that enum
/// isn't (and doesn't need to be) `Deserialize`.
#[tauri::command]
#[specta::specta]
fn set_hotkey(
    mode: String,
    shortcut: String,
    app: tauri::AppHandle,
    settings_state: tauri::State<Mutex<settings::AppSettings>>,
    session: tauri::State<Option<session::SessionHandle>>,
) -> Result<(), String> {
    let Some(session_handle) = session.inner().clone() else {
        return Err(
            "hotkeys aren't active this session (history store failed to open at startup)".into(),
        );
    };

    let hotkey_mode = match mode.as_str() {
        "mic" => hotkey::HotkeyMode::MicDictation,
        "system_audio" => hotkey::HotkeyMode::SystemAudio,
        other => return Err(format!("unknown hotkey mode: {other}")),
    };

    let mut settings = settings_state
        .inner()
        .lock()
        .expect("settings lock poisoned");
    let (old_shortcut, other_mode_current) = match hotkey_mode {
        hotkey::HotkeyMode::MicDictation => (
            settings.mic_hotkey.clone(),
            settings.system_audio_hotkey.clone(),
        ),
        hotkey::HotkeyMode::SystemAudio => (
            settings.system_audio_hotkey.clone(),
            settings.mic_hotkey.clone(),
        ),
    };

    if shortcut == other_mode_current {
        return Err("that shortcut is already assigned to the other hotkey".into());
    }

    hotkey::update_hotkey(&app, hotkey_mode, &old_shortcut, &shortcut, move |m| {
        session_handle.hotkey_pressed(m);
    })
    .map_err(|e| e.to_string())?;

    match hotkey_mode {
        hotkey::HotkeyMode::MicDictation => settings.mic_hotkey = shortcut,
        hotkey::HotkeyMode::SystemAudio => settings.system_audio_hotkey = shortcut,
    }
    settings
        .save()
        .map_err(|e| format!("hotkey updated but failed to persist to disk: {e}"))?;

    Ok(())
}

/// Backs the dashboard's grammar-cleanup toggle (Section 5, "Option B").
/// Updates the live flag `GrammarPipeline` reads immediately (no restart
/// needed — the very next transcript picks it up) and persists to
/// `settings.json` via the same `Mutex<AppSettings>` `set_hotkey` uses.
#[tauri::command]
#[specta::specta]
fn set_grammar_llm_cleanup_enabled(
    enabled: bool,
    settings_state: tauri::State<Mutex<settings::AppSettings>>,
    llm_enabled: tauri::State<GrammarLlmCleanupFlag>,
) -> Result<(), String> {
    llm_enabled.inner().0.store(enabled, Ordering::Relaxed);

    let mut settings = settings_state
        .inner()
        .lock()
        .expect("settings lock poisoned");
    settings.grammar_llm_cleanup_enabled = enabled;
    settings
        .save()
        .map_err(|e| format!("setting updated but failed to persist to disk: {e}"))
}

/// Live, hot-path-readable twin of the seven new `AppSettings` boolean
/// fields (frontend-rewrite plan, D3) — same reasoning as
/// `GrammarLlmCleanupFlag`: `RuleBasedCleanup` and `segment_worker` read
/// these on every transcript without locking the same `Mutex<AppSettings>`
/// the Settings-panel commands use. `set_bool_setting` below is the single
/// choke point that keeps both this and the persisted settings file in
/// sync — one command instead of seven near-identical ones (D3), with
/// tauri-specta still generating a fully-typed TS union for `field` so the
/// frontend can never pass a typo'd/stringly-typed field name.
struct LiveToggleFlags {
    paste_automatically: Arc<AtomicBool>,
    restore_clipboard: Arc<AtomicBool>,
    rule_based: engine::grammar::RuleBasedCleanupFlags,
}

impl LiveToggleFlags {
    fn from_settings(settings: &settings::AppSettings) -> Self {
        Self {
            paste_automatically: Arc::new(AtomicBool::new(settings.paste_automatically)),
            restore_clipboard: Arc::new(AtomicBool::new(settings.restore_clipboard)),
            rule_based: engine::grammar::RuleBasedCleanupFlags {
                capitalise_sentences: Arc::new(AtomicBool::new(settings.capitalise_sentences)),
                tidy_punctuation: Arc::new(AtomicBool::new(settings.tidy_punctuation)),
                remove_filler_words: Arc::new(AtomicBool::new(settings.remove_filler_words)),
                spoken_formatting: Arc::new(AtomicBool::new(settings.spoken_formatting)),
                apply_spoken_corrections: Arc::new(AtomicBool::new(
                    settings.apply_spoken_corrections,
                )),
            },
        }
    }

    /// The one atomic `field` selects — mirrors `AppSettings::field_mut`'s
    /// match arms exactly (same enum, same seven variants).
    fn atomic_for(&self, field: settings::SettingField) -> &Arc<AtomicBool> {
        use settings::SettingField::*;
        match field {
            PasteAutomatically => &self.paste_automatically,
            RestoreClipboard => &self.restore_clipboard,
            CapitaliseSentences => &self.rule_based.capitalise_sentences,
            TidyPunctuation => &self.rule_based.tidy_punctuation,
            RemoveFillerWords => &self.rule_based.remove_filler_words,
            SpokenFormatting => &self.rule_based.spoken_formatting,
            ApplySpokenCorrections => &self.rule_based.apply_spoken_corrections,
        }
    }
}

/// Backs every new Settings toggle except grammar-LLM-cleanup (which keeps
/// its own dedicated `set_grammar_llm_cleanup_enabled` command — pre-existing,
/// left untouched). Same live-flag-plus-persisted-settings dual-update shape
/// as that command, just generalized over `SettingField` (D3) instead of
/// hardcoded to one field.
#[tauri::command]
#[specta::specta]
fn set_bool_setting(
    field: settings::SettingField,
    enabled: bool,
    settings_state: tauri::State<Mutex<settings::AppSettings>>,
    live_flags: tauri::State<LiveToggleFlags>,
) -> Result<(), String> {
    live_flags
        .inner()
        .atomic_for(field)
        .store(enabled, Ordering::Relaxed);

    let mut settings = settings_state
        .inner()
        .lock()
        .expect("settings lock poisoned");
    *settings.field_mut(field) = enabled;
    settings
        .save()
        .map_err(|e| format!("setting updated but failed to persist to disk: {e}"))
}

/// Wire-format twin of `vibrancy::Rect` — `serde::Deserialize` lives here
/// rather than on the shared type since it's purely an IPC concern.
#[derive(serde::Deserialize, specta::Type)]
struct VibrancyRectDto {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<VibrancyRectDto> for vibrancy::Rect {
    fn from(r: VibrancyRectDto) -> Self {
        vibrancy::Rect {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// `pill.js` calls this on load and whenever `#pill`'s own rendered size
/// changes (its width varies per state — listening shows a waveform/timer/
/// controls, done/canceling show just an icon+status — even though the
/// window itself is fixed-size and non-resizable, so a `ResizeObserver` on
/// `#pill` itself is what drives this, not a window resize event).
///
/// Delegates to `session::apply_pill_layout`, which only touches the real
/// window while it's visible or about to become visible — see that
/// function's docs for the real bug this guards against (resizing/
/// remasking a *hidden* vibrant window left a persistent WindowServer
/// compositing ghost, and this command fires on every page load
/// regardless of window visibility, so every launch used to trigger it).
/// Only `pill.width` is used; `pill.x`/`pill.y` are not — see
/// `apply_pill_layout`'s own docs for why.
#[tauri::command]
#[specta::specta]
fn set_pill_vibrancy_layout(window: tauri::WebviewWindow, pill: VibrancyRectDto) {
    session::apply_pill_layout(&window, pill.width);
}

/// Shows the dashboard window — both real call sites (`complete_onboarding`,
/// the tray's "Open Dashboard") go through this rather than an inline
/// `.show()`.
///
/// **2026-09-01: a native single-shape vibrancy mask (matching the pill's
/// already-proven `mask_to_shape`) was attempted here, to round the
/// dashboard window's outer "shell" corners — user-directed, "the shell
/// should have rounded corners, same as the dashboard." Reverted, not
/// shipped: live-verified via screenshots and native-side NSLog diagnostics
/// that the reported rect and the vibrancy view's own bounds matched
/// exactly (both confirmed 520x400, mask request (0,0,520,400) r=16), yet
/// the rendered result stayed square at the top and only rounded at the
/// bottom — and a deliberately wrong, drastically inset test rect (60pt
/// margin on every side) produced no visible change at all, meaning
/// `effectView.maskImage` was not visibly affecting the window's actual
/// rendered vibrancy for this window. Root cause not found (Xcode's view
/// debugger would be the next real step, not available here). This is the
/// same fragility class `vibrancy.rs`'s own module docs already warn about
/// for the dashboard specifically — the two-shape version hit an equally
/// unresolved wall in 2026-08-31. `windowEffects.radius` remains completely
/// unusable (blank-window bug, see DESIGN.md). Left as a real open TODO,
/// not silently dropped.
fn reveal_dashboard(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("dashboard") {
        let _ = win.show();
    }
}

/// Builds the tauri-specta command registry once, shared by both the real
/// `invoke_handler` and (debug builds only) the TypeScript export below —
/// this is the single source of truth for "what commands exist and what do
/// their types look like" that `frontend/src/lib/bindings.ts` is generated
/// from, so the frontend can never drift from what the backend actually
/// accepts (the exact class of bug the frontend-rewrite plan's tauri-specta
/// choice exists to eliminate — see this project's own history with the
/// `Transcript` struct deviation).
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        cancel_recording,
        get_metrics,
        get_language_breakdown,
        get_daily_activity,
        get_latency_stats,
        get_history_page,
        copy_history_text,
        get_permission_status,
        open_permission_settings,
        request_mic_access,
        complete_onboarding,
        quit_app,
        get_recovery_info,
        get_settings,
        set_hotkey,
        set_grammar_llm_cleanup_enabled,
        set_bool_setting,
        set_pill_vibrancy_layout,
    ])
}

pub fn run() {
    logging::init();

    let specta_builder = specta_builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app| {
            // Menu-bar-only app, no dock icon — set at runtime rather than
            // via Info.plist, per docs/mutter-project-plan.md Section 4
            // ("Menu-bar icon only, no dock presence").
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // The dashboard's vibrancy history, 2026-08-31, same day, three
            // stops: static background image -> masked two-shape native
            // vibrancy (`#app` card + `#sidebar` pill) -> no background at
            // all -> here, uniform whole-window native vibrancy, same
            // mechanism as pill/recovery, no masking. The masked attempt
            // confirmed the native call itself worked (`applied=true`,
            // correct geometry via a live diagnostic) but never actually
            // constrained vibrancy to just the two reported shapes — the
            // exact "rectangular edge artifacts" fragility vibrancy.rs's
            // module docs already warned about, reproduced not resolved.
            // Rather than keep fighting that, the dashboard now applies
            // vibrancy to the whole window (`windowEffects` in
            // tauri.conf.json, no masking command at all) and lets
            // `#sidebar`/`#app` each carry their own `.glass-panel` tint on
            // top — the gap around the floating sidebar is vibrant too, not
            // real unblurred desktop, a deliberate reliability trade.

            let app_handle = app.handle().clone();
            let app_settings = settings::AppSettings::load();
            let onboarding_completed = app_settings.onboarding_completed;
            let grammar_llm_cleanup_enabled = Arc::new(AtomicBool::new(
                app_settings.grammar_llm_cleanup_enabled,
            ));
            let live_toggle_flags = LiveToggleFlags::from_settings(&app_settings);

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
                        Arc::new(engine::pipeline::GrammarPipeline::new(
                            grammar_llm_cleanup_enabled.clone(),
                            live_toggle_flags.rule_based.clone(),
                        ));

                    let handle = session::spawn(
                        app_handle.clone(),
                        engine,
                        grammar,
                        history.clone(),
                        DEFAULT_ENGINE_NAME,
                        live_toggle_flags.paste_automatically.clone(),
                        live_toggle_flags.restore_clipboard.clone(),
                    );

                    let hotkey_handle = handle.clone();
                    hotkey::register_hotkeys(
                        &app_handle,
                        &app_settings.mic_hotkey,
                        &app_settings.system_audio_hotkey,
                        move |mode| {
                            hotkey_handle.hotkey_pressed(mode);
                        },
                    )?;

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

            // First-run onboarding (docs/designs/onboarding-flow-plan.md).
            // Recovery mode takes priority by construction — checked first,
            // above — so a fresh install whose history DB is somehow
            // already corrupt shows recovery, never onboarding. Otherwise,
            // an unset `onboarding_completed` (both a genuinely fresh
            // install and an existing user's `settings.json` from before
            // this field existed — the same `#[serde(default)]` false)
            // shows the onboarding window instead of leaving pill/dashboard
            // in their normal hidden-until-summoned state. `set_focus()`
            // mirrors `recovery`'s own exact pattern above — the app runs
            // `ActivationPolicy::Accessory` (no Dock icon), so a newly-shown
            // window does not auto-foreground on its own.
            //
            // The priority decision itself (`should_show_onboarding`) is a
            // pure function specifically so this branch ordering — the one
            // real behavioral contract onboarding adds to `setup()` — is
            // unit-testable without standing up a real `tauri::App`.
            if should_show_onboarding(in_recovery, onboarding_completed) {
                if let Some(win) = app.get_webview_window("onboarding") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }

            app.manage(session_handle);
            app.manage(history_for_dashboard);
            app.manage(recovery_info);
            app.manage(GrammarLlmCleanupFlag(grammar_llm_cleanup_enabled));
            app.manage(live_toggle_flags);
            app.manage(Mutex::new(app_settings));

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

            // The pill is draggable (`data-tauri-drag-region` on `#pill`,
            // 2026-08-30) — this is the only way to observe that a drag
            // actually happened, since a native drag moves the window
            // directly at the WindowServer level with no JS drag-start/
            // drag-end event of our own. See session.rs's
            // `handle_pill_moved` / `PILL_USER_POSITIONED` docs for why a
            // real user drag needs to be told apart from our own
            // programmatic repositioning here, not just observed directly.
            if let Some(pill) = app.get_webview_window("pill") {
                pill.on_window_event(move |event| {
                    if let tauri::WindowEvent::Moved(_) = event {
                        session::handle_pill_moved();
                    }
                });
            }

            // tauri.conf.json's `app.trayIcon` already creates the tray
            // icon itself at startup (id "main") — this attaches the menu
            // and click handling to it. `toggle_listening` is built as an
            // `IconMenuItem` (not the `.text()` builder shorthand the other
            // two items use) specifically so its handle can be retained and
            // mutated later — see `ListeningMenuItem` and
            // `session::update_tray_listening_indicator`, which flips its
            // label between "Start Listening"/"Stop Listening" and shows a
            // danger-red dot icon only while a recording is actually in
            // progress (`state == "listening"`), the same signal the
            // sidebar's own danger-colored Quit button uses for "this is
            // the one destructive/state-changing action here".
            let toggle_listening_item =
                tauri::menu::IconMenuItemBuilder::with_id("toggle_listening", "Start Listening")
                    .build(app)?;
            app.manage(ListeningMenuItem(toggle_listening_item.clone()));
            let menu = MenuBuilder::new(app)
                .item(&toggle_listening_item)
                .separator()
                .text("open_dashboard", "Open Dashboard")
                .separator()
                .quit()
                .build()?;
            if let Some(tray) = app.tray_by_id("main") {
                tray.set_menu(Some(menu))?;
                tray.on_menu_event(move |app, event| {
                    if event.id() == "toggle_listening" {
                        // Same toggle the mic hotkey itself sends — this
                        // app's session model is toggle-based throughout
                        // (press once to start, again to stop), not
                        // push-to-talk, so there's no separate "start only"
                        // primitive to call. Recovery mode has no session
                        // handle at all (None), so this is a harmless no-op
                        // there.
                        if let Some(handle) = app.state::<Option<session::SessionHandle>>().inner()
                        {
                            handle.hotkey_pressed(hotkey::HotkeyMode::MicDictation);
                        }
                    } else if event.id() == "open_dashboard" {
                        // In recovery mode the dashboard window was closed
                        // above and has no working history store behind
                        // it anyway — route back to the recovery screen
                        // instead of trying (and failing) to open it.
                        if in_recovery {
                            if let Some(win) = app.get_webview_window("recovery") {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        } else {
                            reveal_dashboard(app);
                            if let Some(win) = app.get_webview_window("dashboard") {
                                let _ = win.set_focus();
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running mutter");
}

#[cfg(test)]
mod bindings_export {
    /// Regenerates `frontend/src/lib/bindings.ts` from the current
    /// `#[tauri::command]` definitions — the standard tauri-specta
    /// pattern (a `#[test]`, not a runtime call in `run()`). Two real
    /// reasons this lives here instead of app startup:
    ///
    /// 1. **A real bug, found live**: exporting from inside the actual
    ///    launched `.app` bundle (even debug-only) makes macOS treat the
    ///    write as "a GUI app touching your Documents folder" and throw up
    ///    a TCC consent dialog — reproduced by launching the bundled app
    ///    and watching a real "Mutter would like to access files in your
    ///    Documents folder" prompt appear, blocking the window from
    ///    rendering until dismissed. `cargo test` runs as a plain CLI
    ///    process (inheriting the terminal's own TCC grants), which
    ///    doesn't trigger this at all.
    /// 2. Codegen-as-a-side-effect-of-starting-the-app is also just an
    ///    unusual pattern regardless of the TCC issue — an explicit,
    ///    deliberately-run step is more conventional.
    ///
    /// Run explicitly: `cargo test --lib export_bindings -- --ignored`.
    /// `#[ignore]`d so it's not part of the default `cargo test` run (it
    /// writes to the repo rather than asserting anything) — same
    /// convention this crate already uses for the slow model-download
    /// integration tests.
    #[test]
    #[ignore = "codegen, not an assertion — writes frontend/src/lib/bindings.ts; run explicitly after changing any #[tauri::command]"]
    fn export_bindings() {
        super::specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../frontend/src/lib/bindings.ts"
                ),
            )
            .expect("failed to export TypeScript bindings");
    }
}
