//! One generic permission state machine, shared by mic, Accessibility, and
//! system-audio permissions — not three hand-rolled implementations. A DRY
//! finding from eng review (docs/mutter-project-plan.md Section 11, Code
//! Quality Issue 4): three near-identical state machines is a violation
//! waiting to happen, and a future fourth permission family should cost one
//! instantiation, not a new implementation.

use std::marker::PhantomData;

/// State of a single permission. `Unavailable` covers device-level problems
/// distinct from a user denial: mic not present, disconnected mid-capture,
/// held exclusively by another app (mic); or the equivalent for other
/// permission kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    NotRequested,
    Denied,
    Granted,
    Unavailable,
}

/// Marker types for each permission family this app cares about. Adding a
/// fourth family is one new zero-sized marker type plus one
/// `PermissionGate<NewKind>` instantiation — not a new state machine.
pub struct Mic;
pub struct Accessibility;
pub struct SystemAudio;

/// A permission gate for one permission kind `T`. `T` carries no data — it
/// exists only to make `PermissionGate<Mic>` and `PermissionGate<Accessibility>`
/// distinct types at compile time, so they can't be mixed up by accident.
pub struct PermissionGate<T> {
    state: PermissionState,
    _kind: PhantomData<T>,
}

impl<T> PermissionGate<T> {
    pub fn new() -> Self {
        Self {
            state: PermissionState::NotRequested,
            _kind: PhantomData,
        }
    }

    pub fn state(&self) -> PermissionState {
        self.state
    }

    pub fn set_state(&mut self, state: PermissionState) {
        self.state = state;
    }

    pub fn is_granted(&self) -> bool {
        matches!(self.state, PermissionState::Granted)
    }

    /// True for any state that should surface a recoverable UI path (deep
    /// link to System Settings, or a clear "unavailable" message) rather
    /// than a silent failure.
    pub fn needs_recovery_ui(&self) -> bool {
        matches!(
            self.state,
            PermissionState::Denied | PermissionState::Unavailable
        )
    }
}

impl<T> Default for PermissionGate<T> {
    fn default() -> Self {
        Self::new()
    }
}

// --- Real OS-backed status checks ---
//
// One `refresh()` per kind rather than a generic dispatch — `T` carries no
// data to dispatch on (it's a zero-sized marker), and each permission
// family's underlying OS API is genuinely different (AVFoundation,
// ApplicationServices' AX API, CoreGraphics' screen-capture preflight).
// Adding a fourth family per the module doc's own framing is still "one new
// marker type plus one `PermissionGate<NewKind>` instantiation" — this is
// just where that instantiation's OS-specific edge lives.

extern "C" {
    /// Defined in native/permissions_shim.m. Mirrors `AVAuthorizationStatus`
    /// exactly: 0 = NotDetermined, 1 = Restricted, 2 = Denied, 3 = Authorized.
    fn mutter_mic_auth_status() -> i32;

    /// Defined in native/permissions_shim.m. Shows the real native mic
    /// permission prompt and blocks until answered (or returns immediately
    /// if already answered). Returns 1 if granted, 0 otherwise. Callers
    /// MUST NOT invoke this on the main thread — see the shim header.
    fn mutter_request_mic_access() -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    /// Plain C entry point — no Objective-C shim needed for this one.
    fn CGPreflightScreenCaptureAccess() -> bool;
}

impl PermissionGate<Mic> {
    /// Query the real microphone authorization status from AVFoundation and
    /// update `self` to match. `Restricted` (parental controls/MDM, not a
    /// user choice) maps to `Unavailable` per this module's own
    /// distinction between "device-level problem" and "user denial".
    pub fn refresh(&mut self) {
        let status = unsafe { mutter_mic_auth_status() };
        self.state = match status {
            0 => PermissionState::NotRequested,
            1 => PermissionState::Unavailable,
            2 => PermissionState::Denied,
            3 => PermissionState::Granted,
            other => {
                tracing::warn!(status = other, "unexpected AVAuthorizationStatus value");
                PermissionState::Unavailable
            }
        };
    }

    /// Shows the real native mic permission prompt (a one-time system
    /// dialog — a no-op returning the existing status if already answered)
    /// and updates `self` to match the real post-prompt status. Blocking —
    /// callers MUST run this off the main/UI thread (see
    /// `lib.rs`'s `request_mic_access` command, which wraps it in
    /// `tauri::async_runtime::spawn_blocking`).
    ///
    /// Deliberately untested, unlike its siblings' `_does_not_panic` smoke
    /// tests below (review finding): a real test would trigger the actual
    /// system dialog and hang CI waiting for a human to answer it. See
    /// `TODOS.md`'s "Onboarding's mic native-prompt path" entry — this
    /// exact path is already flagged as needing live human verification,
    /// which is a stronger check than a CI smoke test could give anyway.
    pub fn request(&mut self) -> bool {
        let granted = unsafe { mutter_request_mic_access() } != 0;
        self.refresh();
        granted
    }
}

impl PermissionGate<Accessibility> {
    /// `AXIsProcessTrusted` has no third "not yet asked" state the way mic
    /// permission does — untrusted is untrusted regardless of whether the
    /// user has ever seen the prompt (the first `AXUIElement` call made
    /// while untrusted, in injection.rs, is itself what triggers it).
    pub fn refresh(&mut self) {
        let trusted = unsafe { accessibility_sys::AXIsProcessTrusted() };
        self.state = if trusted {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        };
    }
}

impl PermissionGate<SystemAudio> {
    pub fn refresh(&mut self) {
        let granted = unsafe { CGPreflightScreenCaptureAccess() };
        self.state = if granted {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_not_requested() {
        let gate: PermissionGate<Mic> = PermissionGate::new();
        assert_eq!(gate.state(), PermissionState::NotRequested);
        assert!(!gate.is_granted());
        assert!(!gate.needs_recovery_ui());
    }

    #[test]
    fn granted_is_granted_and_not_recoverable() {
        let mut gate: PermissionGate<Accessibility> = PermissionGate::new();
        gate.set_state(PermissionState::Granted);
        assert!(gate.is_granted());
        assert!(!gate.needs_recovery_ui());
    }

    #[test]
    fn denied_and_unavailable_both_need_recovery_ui() {
        let mut gate: PermissionGate<SystemAudio> = PermissionGate::new();
        gate.set_state(PermissionState::Denied);
        assert!(gate.needs_recovery_ui());

        gate.set_state(PermissionState::Unavailable);
        assert!(gate.needs_recovery_ui());
    }

    // Smoke tests only — the actual `PermissionState` these produce depends
    // on the CI/dev machine's real TCC state, which these tests
    // deliberately don't assert a specific value for. What they do verify:
    // the FFI calls into the native shim/frameworks don't crash and always
    // leave the gate in a valid (non-panicking-to-construct) state.
    #[test]
    fn mic_refresh_does_not_panic() {
        let mut gate: PermissionGate<Mic> = PermissionGate::new();
        gate.refresh();
        let _ = gate.state();
    }

    #[test]
    fn accessibility_refresh_does_not_panic() {
        let mut gate: PermissionGate<Accessibility> = PermissionGate::new();
        gate.refresh();
        let _ = gate.state();
    }

    #[test]
    fn system_audio_refresh_does_not_panic() {
        let mut gate: PermissionGate<SystemAudio> = PermissionGate::new();
        gate.refresh();
        let _ = gate.state();
    }
}
