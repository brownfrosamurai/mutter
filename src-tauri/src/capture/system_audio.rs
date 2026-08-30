//! System-audio (speaker) capture via `ScreenCaptureKit`, "like Granola" per
//! the original spec (docs/mutter-idea-dump.md). STUB — Phase 0 spike
//! required first, then Phase 4 implementation.
//!
//! Open questions this stub does NOT resolve (docs/mutter-project-plan.md
//! Section 9, Section 15 Phase 0):
//! - Rust-binding approach: `objc2`-based crate vs. a minimal build-time
//!   Objective-C shim (NOT hand-written Swift source — that stays out per
//!   the project's hard constraint, see ../../../CLAUDE.md).
//! - Audio-only capability shape: confirm no video-frame capture overhead,
//!   and the exact entitlement/permission scoping for audio-only vs.
//!   full-screen capture — assumed clean, not yet verified.
//! - Permission model is heavier than a mic prompt: `ScreenCaptureKit`
//!   requires Screen Recording consent even for audio-only capture, with a
//!   persistent system recording indicator. This needs its own onboarding
//!   copy explaining why an audio-only feature triggers a screen-recording
//!   permission.
//!
//! Buffer cap: 300s (longer than mic's 120s, given meeting-length use).

pub const MAX_DURATION_SECS: u64 = 300;

pub struct SystemAudioCapture {
    // ScreenCaptureKit bridge handle goes here once the binding approach is
    // decided.
}

impl SystemAudioCapture {
    pub fn new() -> Self {
        Self {}
    }

    pub fn start(&mut self) {
        unimplemented!("SystemAudioCapture::start — pending Phase 0 spike")
    }

    pub fn stop(&mut self) -> Vec<f32> {
        unimplemented!("SystemAudioCapture::stop — pending Phase 0 spike")
    }
}

impl Default for SystemAudioCapture {
    fn default() -> Self {
        Self::new()
    }
}
