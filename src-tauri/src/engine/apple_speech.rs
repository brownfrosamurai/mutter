//! `SFSpeechRecognizer`-backed `TranscriptionEngine`.
//!
//! STUB — only build this out if the Phase 0 benchmark (docs/mutter-project-plan.md
//! Section 6) favors Apple's on-device Speech framework for any of the six
//! target languages. Free, no model download, but verify on-device-only
//! behavior per language before relying on it — Apple's on-device support is
//! locale-dependent and has historically fallen back to server-based
//! recognition for less common languages. A silent network fallback would
//! violate "zero network calls after model download" as an absolute.
//!
//! `SFSpeechRecognizer` is an Objective-C/Swift-native API — this needs the
//! same kind of binding decision as the ScreenCaptureKit bridge
//! (see capture/system_audio.rs), not hand-written Swift source.

use super::{EngineError, TranscriptionEngine};

pub struct AppleSpeechEngine {
    // Bridge handle goes here once the binding approach is decided.
}

impl AppleSpeechEngine {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AppleSpeechEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TranscriptionEngine for AppleSpeechEngine {
    async fn transcribe(&self, _audio: &[f32]) -> Result<String, EngineError> {
        Err(EngineError::ModelNotLoaded(
            "AppleSpeechEngine is a scaffold stub — pending Phase 0 benchmark result".into(),
        ))
    }
}
