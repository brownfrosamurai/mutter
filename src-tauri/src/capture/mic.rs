//! Mic capture via `cpal`. STUB — Phase 1 (core loop) work.
//!
//! Capture must run on a dedicated background thread, never the UI/hotkey
//! handler thread (docs/mutter-project-plan.md Section 3). Buffer is capped
//! at 120s; hitting the cap auto-transcribes-and-continues rather than
//! truncating (Section 3 — this was a real gap caught by the outside-voice
//! review: the primary use case, dictating specs to an AI agent, routinely
//! runs past 2 minutes).

pub const MAX_DURATION_SECS: u64 = 120;

pub struct MicCapture {
    // cpal stream handle goes here.
}

impl MicCapture {
    pub fn new() -> Self {
        Self {}
    }

    /// Start capturing. Stub — no-op until Phase 1.
    pub fn start(&mut self) {
        unimplemented!("MicCapture::start — Phase 1 core loop work")
    }

    /// Stop capturing and return the buffered PCM samples.
    pub fn stop(&mut self) -> Vec<f32> {
        unimplemented!("MicCapture::stop — Phase 1 core loop work")
    }
}

impl Default for MicCapture {
    fn default() -> Self {
        Self::new()
    }
}
