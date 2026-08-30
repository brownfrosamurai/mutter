//! Escape/cancel state machine: `Recording -> CancelPending -> {Discarded | Resumed}`.
//! From docs/mutter-project-plan.md Section 7, directly from the user's own
//! spec: 1st Escape starts a visible countdown; the countdown expiring
//! discards the in-progress transcription; a 2nd Escape before it expires
//! aborts the cancellation and resumes exactly where it left off.
//!
//! The Escape key hook itself is installed only for the duration of an
//! active recording/cancel-pending session and torn down immediately after
//! (docs/mutter-project-plan.md Section 3) — it must never swallow Escape
//! system-wide while idle. That hook lifecycle lives in hotkey.rs; this
//! module is just the state machine it drives.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelState {
    /// Not in a cancel flow — normal recording or transcribing.
    Recording,
    /// First Escape pressed; countdown running.
    CancelPending,
    /// Countdown expired — the in-progress buffer is discarded.
    Discarded,
    /// Second Escape pressed before the countdown expired — recording resumes.
    Resumed,
}

pub struct CancelStateMachine {
    state: CancelState,
}

impl CancelStateMachine {
    pub fn new() -> Self {
        Self {
            state: CancelState::Recording,
        }
    }

    pub fn state(&self) -> CancelState {
        self.state
    }

    /// Call when Escape is pressed. Returns the new state.
    pub fn on_escape(&mut self) -> CancelState {
        self.state = match self.state {
            CancelState::Recording => CancelState::CancelPending,
            CancelState::CancelPending => CancelState::Resumed,
            // Escape while already terminal is a no-op — nothing to cancel
            // or resume.
            terminal @ (CancelState::Discarded | CancelState::Resumed) => terminal,
        };
        self.state
    }

    /// Call when the cancel countdown expires without a second Escape.
    /// No-op unless currently `CancelPending`.
    pub fn on_countdown_expired(&mut self) -> CancelState {
        if self.state == CancelState::CancelPending {
            self.state = CancelState::Discarded;
        }
        self.state
    }
}

impl Default for CancelStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_recording() {
        let m = CancelStateMachine::new();
        assert_eq!(m.state(), CancelState::Recording);
    }

    #[test]
    fn first_escape_enters_cancel_pending() {
        let mut m = CancelStateMachine::new();
        assert_eq!(m.on_escape(), CancelState::CancelPending);
    }

    #[test]
    fn countdown_expiry_discards() {
        let mut m = CancelStateMachine::new();
        m.on_escape();
        assert_eq!(m.on_countdown_expired(), CancelState::Discarded);
    }

    #[test]
    fn second_escape_before_expiry_resumes() {
        let mut m = CancelStateMachine::new();
        m.on_escape(); // -> CancelPending
        assert_eq!(m.on_escape(), CancelState::Resumed); // -> Resumed
    }

    #[test]
    fn countdown_expiry_is_noop_outside_cancel_pending() {
        let mut m = CancelStateMachine::new();
        // Never entered CancelPending — expiry should not discard.
        assert_eq!(m.on_countdown_expired(), CancelState::Recording);
    }

    #[test]
    fn escape_after_resumed_is_noop() {
        let mut m = CancelStateMachine::new();
        m.on_escape(); // CancelPending
        m.on_escape(); // Resumed
        assert_eq!(m.on_escape(), CancelState::Resumed);
    }

    #[test]
    fn escape_after_discarded_is_noop() {
        let mut m = CancelStateMachine::new();
        m.on_escape(); // CancelPending
        m.on_countdown_expired(); // Discarded
        assert_eq!(m.on_escape(), CancelState::Discarded);
    }
}
