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
}
