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
//! User-configurable since Section 15's dashboard settings phase — see
//! ../settings.rs for persistence and `update_hotkey` below for changing a
//! binding at runtime.
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
    InvalidShortcut(String, String),
    #[error("failed to register shortcut: {0}")]
    RegisterFailed(String),
}

pub const MIC_DICTATION_SHORTCUT_DEFAULT: &str = "CmdOrCtrl+Shift+Space";
pub const SYSTEM_AUDIO_SHORTCUT_DEFAULT: &str = "CmdOrCtrl+Shift+M";

/// Register both toggle hotkeys against their currently-configured strings
/// (see `settings::AppSettings`). `on_press` is invoked once per key-down
/// (never key-up) with which mode fired. The caller is responsible for
/// hopping to wherever session state actually lives — this runs on whatever
/// thread the OS delivers the hotkey event on.
pub fn register_hotkeys<R, F>(
    app: &AppHandle<R>,
    mic_shortcut: &str,
    system_audio_shortcut: &str,
    on_press: F,
) -> Result<(), HotkeyError>
where
    R: Runtime,
    F: Fn(HotkeyMode) + Send + Sync + 'static,
{
    let mic: Shortcut = parse_shortcut(mic_shortcut)?;
    let system_audio: Shortcut = parse_shortcut(system_audio_shortcut)?;

    let on_press = Arc::new(on_press);
    register_one(app, mic, HotkeyMode::MicDictation, on_press.clone())?;
    register_one(app, system_audio, HotkeyMode::SystemAudio, on_press)?;

    tracing::info!(
        mic = mic_shortcut,
        system_audio = system_audio_shortcut,
        "global hotkeys registered"
    );
    Ok(())
}

/// Change one mode's binding at runtime. Registers the new shortcut
/// *before* unregistering the old one — if the new spec is invalid or
/// already claimed by another app, this fails with the old binding still
/// intact rather than leaving that mode with no hotkey at all.
pub fn update_hotkey<R, F>(
    app: &AppHandle<R>,
    mode: HotkeyMode,
    old_shortcut: &str,
    new_shortcut: &str,
    on_press: F,
) -> Result<(), HotkeyError>
where
    R: Runtime,
    F: Fn(HotkeyMode) + Send + Sync + 'static,
{
    let parsed_new = parse_shortcut(new_shortcut)?;
    register_one(app, parsed_new, mode, Arc::new(on_press))?;

    if let Err(e) = unregister(app, old_shortcut) {
        tracing::warn!(
            error = %e,
            old = old_shortcut,
            "failed to unregister old hotkey after registering its replacement"
        );
    }

    tracing::info!(mode = ?mode, old = old_shortcut, new = new_shortcut, "hotkey updated");
    Ok(())
}

pub fn unregister<R: Runtime>(app: &AppHandle<R>, shortcut: &str) -> Result<(), HotkeyError> {
    let shortcut = parse_shortcut(shortcut)?;
    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|e| HotkeyError::RegisterFailed(e.to_string()))
}

fn register_one<R, F>(
    app: &AppHandle<R>,
    shortcut: Shortcut,
    mode: HotkeyMode,
    on_press: Arc<F>,
) -> Result<(), HotkeyError>
where
    R: Runtime,
    F: Fn(HotkeyMode) + Send + Sync + 'static,
{
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                on_press(mode);
            }
        })
        .map_err(|e| HotkeyError::RegisterFailed(e.to_string()))
}

/// A real incident during dev (not hypothetical): a fumbled edit in the
/// dashboard's hotkey-config UI briefly persisted the mic-dictation hotkey
/// as a bare, unmodified `"Space"` — which `tauri-plugin-global-shortcut`
/// happily registered, meaning every spacebar press system-wide toggled mic
/// dictation until the app was killed. A global hotkey with zero modifier
/// keys is never a reasonable binding (it hijacks that key everywhere, in
/// every app), so this rejects one at the single choke point every caller
/// already goes through — the hardcoded defaults above are unaffected since
/// they both carry modifiers.
fn parse_shortcut(spec: &str) -> Result<Shortcut, HotkeyError> {
    let shortcut: Shortcut = spec
        .parse()
        .map_err(|e: <Shortcut as std::str::FromStr>::Err| {
            HotkeyError::InvalidShortcut(spec.to_string(), e.to_string())
        })?;
    if shortcut.mods.is_empty() {
        return Err(HotkeyError::InvalidShortcut(
            spec.to_string(),
            "a global hotkey needs at least one modifier key (Cmd/Ctrl/Shift/Alt) — a bare key would hijack it system-wide".to_string(),
        ));
    }
    Ok(shortcut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_bare_unmodified_key() {
        let err = parse_shortcut("Space").unwrap_err();
        assert!(matches!(err, HotkeyError::InvalidShortcut(_, _)));
    }

    #[test]
    fn accepts_a_modified_shortcut() {
        assert!(parse_shortcut("CmdOrCtrl+Shift+Space").is_ok());
    }

    #[test]
    fn default_shortcuts_are_valid() {
        assert!(parse_shortcut(MIC_DICTATION_SHORTCUT_DEFAULT).is_ok());
        assert!(parse_shortcut(SYSTEM_AUDIO_SHORTCUT_DEFAULT).is_ok());
    }
}
