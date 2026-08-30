//! Global hotkey registration via `tauri-plugin-global-shortcut`.
//!
//! Toggle, not push-to-talk: press once to start capture, press again to
//! stop and trigger transcription (docs/mutter-project-plan.md Section 3).
//! This module only reports *that a hotkey was pressed* — it fires the
//! caller's callback on every `Pressed` event (ignoring `Released`) and
//! deliberately does not itself track "are we currently recording" or
//! re-entrancy ("ignore a press while a prior recording is still
//! transcribing", Section 3). That's session state, not hotkey state — it
//! belongs in the session orchestrator (see ../session.rs), which is the
//! single place that decides what a press means right now.
//!
//! Default key combinations below are placeholders — Section 15's dashboard
//! settings phase is where these become user-configurable; nothing in the
//! plan pins a specific combo.
//!
//! The Escape-key hook for the cancel state machine (see ../cancel.rs) is a
//! separate concern with a separate lifecycle (installed only for the
//! duration of an active recording/cancel-pending session, torn down
//! immediately after — never registered system-wide while idle) and is not
//! handled by this module.

use std::sync::Arc;

use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyMode {
    MicDictation,
    SystemAudio,
}

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("invalid shortcut string {0:?}: {1}")]
    InvalidShortcut(&'static str, String),
    #[error("failed to register shortcut: {0}")]
    RegisterFailed(String),
}

const MIC_DICTATION_SHORTCUT: &str = "CmdOrCtrl+Shift+Space";
const SYSTEM_AUDIO_SHORTCUT: &str = "CmdOrCtrl+Shift+M";

/// Register both toggle hotkeys. `on_press` is invoked once per key-down
/// (never key-up) with which mode fired. The caller is responsible for
/// hopping to wherever session state actually lives — this runs on whatever
/// thread the OS delivers the hotkey event on.
pub fn register_hotkeys<R, F>(app: &AppHandle<R>, on_press: F) -> Result<(), HotkeyError>
where
    R: Runtime,
    F: Fn(HotkeyMode) + Send + Sync + 'static,
{
    let mic_shortcut: Shortcut = parse_shortcut(MIC_DICTATION_SHORTCUT)?;
    let system_audio_shortcut: Shortcut = parse_shortcut(SYSTEM_AUDIO_SHORTCUT)?;

    let on_press = Arc::new(on_press);
    let global_shortcut = app.global_shortcut();

    let mic_callback = on_press.clone();
    global_shortcut
        .on_shortcut(mic_shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                mic_callback(HotkeyMode::MicDictation);
            }
        })
        .map_err(|e| HotkeyError::RegisterFailed(e.to_string()))?;

    let system_audio_callback = on_press;
    global_shortcut
        .on_shortcut(system_audio_shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                system_audio_callback(HotkeyMode::SystemAudio);
            }
        })
        .map_err(|e| HotkeyError::RegisterFailed(e.to_string()))?;

    Ok(())
}

fn parse_shortcut(spec: &'static str) -> Result<Shortcut, HotkeyError> {
    spec.parse::<Shortcut>()
        .map_err(|e| HotkeyError::InvalidShortcut(spec, e.to_string()))
}
