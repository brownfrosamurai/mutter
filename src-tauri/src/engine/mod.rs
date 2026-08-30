//! Two separate traits, not one: `TranscriptionEngine` (audio -> text) and
//! `TextProcessor` (text -> text). A single shared trait was the eng review's
//! first architecture finding — audio-to-text and text-to-text engines don't
//! have the same input/output shape, and forcing one interface to serve both
//! would mean callers pattern-matching on a variant. See
//! docs/mutter-project-plan.md Section 10.

pub mod apple_speech;
pub mod whisper;

/// Errors an engine or processor can return. Typed on purpose — an untyped
/// boxed error would mean the pill/error log could only ever say "something
/// went wrong" instead of something specific and recoverable. See
/// docs/mutter-project-plan.md Section 10 (Code Quality Issue 5).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("model not loaded: {0}")]
    ModelNotLoaded(String),

    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("inference failed: {0}")]
    InferenceFailed(String),

    #[error("operation timed out after {0:?}")]
    Timeout(std::time::Duration),
}

/// Audio in, text out. Implemented by `WhisperEngine` and (if the Phase 0
/// benchmark favors it) `AppleSpeechEngine`. Language is auto-detected from
/// the audio, not manually selected — see docs/mutter-project-plan.md
/// Section 10.
#[async_trait::async_trait]
pub trait TranscriptionEngine: Send + Sync {
    async fn transcribe(&self, audio: &[f32]) -> Result<String, EngineError>;
}

/// Text in, text out. Used by the grammar-cleanup step (docs/mutter-project-plan.md
/// Section 5, Option B) — a per-transcript, user-triggered action, never
/// always-on middleware in the default pipeline.
#[async_trait::async_trait]
pub trait TextProcessor: Send + Sync {
    async fn process(&self, text: &str, language: &str) -> Result<String, EngineError>;
}
