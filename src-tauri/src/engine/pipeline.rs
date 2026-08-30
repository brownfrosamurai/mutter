//! Composes Option A (always-on rules) with Option B (toggleable local-LLM
//! cleanup) into the single `TextProcessor` `session.rs` actually calls.
//!
//! Option A always runs first — it's fast, deterministic, and the floor
//! this app already ships. Option B only runs if the live settings toggle
//! (`grammar_llm_cleanup_enabled`, checked on every call, not just at
//! startup — flipping it in Settings takes effect on the very next
//! transcript) is on, and its output *replaces* Option A's for that
//! transcript. If Option B errors — model failed to load, download failed,
//! inference panicked — this falls back to Option A's already-computed
//! output rather than losing the transcript entirely; a broken optional
//! enhancement must never block insertion of what Whisper + rules already
//! produced correctly.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::grammar::RuleBasedCleanup;
use super::llm_cleanup::LlmCleanup;
use super::{EngineError, TextProcessor};

pub struct GrammarPipeline {
    rules: RuleBasedCleanup,
    llm: LlmCleanup,
    llm_enabled: Arc<AtomicBool>,
}

impl GrammarPipeline {
    pub fn new(llm_enabled: Arc<AtomicBool>) -> Self {
        Self {
            rules: RuleBasedCleanup,
            llm: LlmCleanup::new(),
            llm_enabled,
        }
    }
}

#[async_trait::async_trait]
impl TextProcessor for GrammarPipeline {
    async fn process(&self, text: &str, language: &str) -> Result<String, EngineError> {
        let cleaned = self.rules.process(text, language).await?;

        if !self.llm_enabled.load(Ordering::Relaxed) {
            return Ok(cleaned);
        }

        match self.llm.process(&cleaned, language).await {
            Ok(polished) => Ok(polished),
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "LLM grammar cleanup failed, falling back to rule-based output"
                );
                Ok(cleaned)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    struct FailingProcessor;
    #[async_trait::async_trait]
    impl TextProcessor for FailingProcessor {
        async fn process(&self, _text: &str, _language: &str) -> Result<String, EngineError> {
            Err(EngineError::InferenceFailed("boom".into()))
        }
    }

    struct UppercaseProcessor;
    #[async_trait::async_trait]
    impl TextProcessor for UppercaseProcessor {
        async fn process(&self, text: &str, _language: &str) -> Result<String, EngineError> {
            Ok(text.to_uppercase())
        }
    }

    /// A stand-in `GrammarPipeline` whose Option B stage is swappable, so
    /// these tests exercise the real gating/fallback logic without needing
    /// the real ~470MB model download `LlmCleanup` requires.
    struct TestPipeline<P: TextProcessor> {
        rules: RuleBasedCleanup,
        llm: P,
        llm_enabled: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl<P: TextProcessor + Send + Sync> TextProcessor for TestPipeline<P> {
        async fn process(&self, text: &str, language: &str) -> Result<String, EngineError> {
            let cleaned = self.rules.process(text, language).await?;
            if !self.llm_enabled.load(Ordering::Relaxed) {
                return Ok(cleaned);
            }
            match self.llm.process(&cleaned, language).await {
                Ok(polished) => Ok(polished),
                Err(_) => Ok(cleaned),
            }
        }
    }

    #[tokio::test]
    async fn skips_llm_stage_when_disabled() {
        let pipeline = TestPipeline {
            rules: RuleBasedCleanup,
            llm: UppercaseProcessor,
            llm_enabled: Arc::new(AtomicBool::new(false)),
        };
        let out = pipeline.process("hello world", "en").await.unwrap();
        assert_eq!(out, "Hello world.");
    }

    #[tokio::test]
    async fn runs_llm_stage_on_rules_output_when_enabled() {
        let pipeline = TestPipeline {
            rules: RuleBasedCleanup,
            llm: UppercaseProcessor,
            llm_enabled: Arc::new(AtomicBool::new(true)),
        };
        let out = pipeline.process("hello world", "en").await.unwrap();
        assert_eq!(out, "HELLO WORLD.");
    }

    #[tokio::test]
    async fn falls_back_to_rules_output_when_llm_stage_errors() {
        let pipeline = TestPipeline {
            rules: RuleBasedCleanup,
            llm: FailingProcessor,
            llm_enabled: Arc::new(AtomicBool::new(true)),
        };
        let out = pipeline.process("hello world", "en").await.unwrap();
        assert_eq!(out, "Hello world.");
    }

    #[tokio::test]
    async fn toggle_takes_effect_live_without_reconstructing_pipeline() {
        let llm_enabled = Arc::new(AtomicBool::new(false));
        let pipeline = TestPipeline {
            rules: RuleBasedCleanup,
            llm: UppercaseProcessor,
            llm_enabled: llm_enabled.clone(),
        };

        assert_eq!(
            pipeline.process("hello world", "en").await.unwrap(),
            "Hello world."
        );

        llm_enabled.store(true, Ordering::Relaxed);

        assert_eq!(
            pipeline.process("hello world", "en").await.unwrap(),
            "HELLO WORLD."
        );
    }
}
