//! Two separate traits, not one: `TranscriptionEngine` (audio -> text) and
//! `TextProcessor` (text -> text). A single shared trait was the eng review's
//! first architecture finding — audio-to-text and text-to-text engines don't
//! have the same input/output shape, and forcing one interface to serve both
//! would mean callers pattern-matching on a variant. See
//! docs/mutter-project-plan.md Section 10.

pub mod apple_speech;
pub mod grammar;
pub mod llm_cleanup;
pub mod pipeline;
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

/// Result of one transcription: the recognized text, plus the language
/// whisper.cpp auto-detected (e.g. `"en"`, `"yo"`).
///
/// Added during implementation: Section 10 describes `TextProcessor::process`
/// as taking a `language` argument, and Section 8's dashboard needs a
/// per-language breakdown, but the trait as originally specified returned a
/// bare `String` with no way to carry the detected language out of
/// `transcribe()`. Audio-in stays language-free — Section 10's
/// auto-detect-only decision is unchanged, this only fixes the *output*
/// shape so downstream consumers that need the language don't have to
/// re-derive it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    pub language: String,
}

/// Audio in, text out. Implemented by `WhisperEngine` and (if the Phase 0
/// benchmark favors it) `AppleSpeechEngine`. Language is auto-detected from
/// the audio, not manually selected — see docs/mutter-project-plan.md
/// Section 10.
#[async_trait::async_trait]
pub trait TranscriptionEngine: Send + Sync {
    async fn transcribe(&self, audio: &[f32]) -> Result<Transcript, EngineError>;

    /// Pay any one-time model-load cost up front, separately from
    /// `transcribe()`. Lets the session orchestrator show a distinct
    /// "loading" pill state for the (potentially very slow — a ~500MB
    /// download on first run ever) first call, rather than folding that
    /// latency silently into the first "transcribing" state where it would
    /// look indistinguishable from a hang (Section 6, Performance Issue 8:
    /// "first transcription pays load latency, shown via the pill's loading
    /// state, subsequent ones don't"). Default no-op — only engines with a
    /// real lazy-load cost (currently just `WhisperEngine`) need to
    /// override this.
    async fn ensure_ready(&self) -> Result<(), EngineError> {
        Ok(())
    }
}

/// Text in, text out. Used by the grammar-cleanup step
/// (docs/mutter-project-plan.md Section 5). The pipeline actually wired in
/// `session.rs` is `engine::pipeline::GrammarPipeline` — Option A
/// (`grammar::RuleBasedCleanup`) always runs, and Option B
/// (`llm_cleanup::LlmCleanup`) additionally runs when the user has it
/// toggled on in Settings (default off) — see `pipeline.rs`'s doc comment
/// for why this ended up always-on rather than the plan's original
/// per-transcript-triggered design.
#[async_trait::async_trait]
pub trait TextProcessor: Send + Sync {
    async fn process(&self, text: &str, language: &str) -> Result<String, EngineError>;
}
