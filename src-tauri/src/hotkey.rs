//! Global hotkey registration via `tauri-plugin-global-shortcut`. STUB —
//! Phase 1 (core loop) work.
//!
//! Toggle, not push-to-talk: press once to start capture, press again to
//! stop and trigger transcription (docs/mutter-project-plan.md Section 3 —
//! this corrected an assumption in the first draft of the plan, which
//! wrongly assumed push-to-talk/hold).
//!
//! Separate hotkey bindings exist for mic-dictation mode and system-audio
//! mode (Section 4). Re-entrancy: if the hotkey is pressed again while a
//! prior recording is still transcribing, the new press is ignored until
//! the in-flight one completes (Section 3).
//!
//! The Escape-key hook for the cancel state machine (see ../cancel.rs) is
//! installed only for the duration of an active recording/cancel-pending
//! session and torn down immediately after — never registered system-wide
//! while idle.

pub enum HotkeyMode {
    MicDictation,
    SystemAudio,
}

pub fn register_hotkeys() {
    unimplemented!("register_hotkeys — Phase 1 core loop work")
}
