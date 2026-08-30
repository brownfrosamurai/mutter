//! whisper.cpp-backed `TranscriptionEngine`, via `whisper-rs`.
//!
//! STUB — Phase 0 spike required first:
//! - whisper-rs bindings vs. CLI shell-out vs. raw FFI (docs/mutter-project-plan.md
//!   Section 14, Open Questions)
//! - Model size/quantization per language (Section 6 — tiered routing:
//!   `small` default, `medium` for languages that need the accuracy)
//! - Lazy-load on first use, then kept resident (Section 6, Performance
//!   Issue 8) — do NOT reload the model per transcription
//!
//! Language auto-detection happens here (Section 10) — the caller never
//! passes a language.

use super::{EngineError, TranscriptionEngine};

pub struct WhisperEngine {
    // Model handle goes here once loaded. Lazy-loaded on first
    // `transcribe()` call, not at construction — see module docs above.
}

impl WhisperEngine {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for WhisperEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TranscriptionEngine for WhisperEngine {
    async fn transcribe(&self, _audio: &[f32]) -> Result<String, EngineError> {
        Err(EngineError::ModelNotLoaded(
            "WhisperEngine is a scaffold stub — Phase 0 spike not yet done".into(),
        ))
    }
}
