//! whisper.cpp-backed `TranscriptionEngine`, via `whisper-rs`.
//!
//! Phase 0 spike result: whisper-rs's native build (whisper.cpp + ggml)
//! compiles cleanly on this toolchain (verified 2026-08-29, cmake/clang
//! already present) — no need for a CLI shell-out or raw FFI fallback
//! (docs/mutter-project-plan.md Section 14, Open Questions).
//!
//! Model is lazy-loaded on first `transcribe()` call, then kept resident for
//! the process lifetime (Section 6, Performance Issue 8) — never reloaded
//! per transcription. Language auto-detection happens inside whisper.cpp
//! itself (Section 10) — the caller never passes a language in, and gets the
//! detected language back out via `Transcript::language`.
//!
//! Model tiering (Section 6): `Small` is the only tier currently routed to
//! by default. `Medium` is wired and functional but nothing selects it yet
//! — the per-language routing decision requires a real accuracy benchmark
//! across all six languages (Yoruba specifically called out) that this
//! environment cannot run: it needs real multi-language audio samples and a
//! human listening for correctness, neither of which exist here. That
//! benchmark is explicitly the user's own task per Section 17 ("run the
//! Phase 0 engine benchmark yourself, informally").

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use tokio::sync::OnceCell;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::{EngineError, Transcript, TranscriptionEngine};

/// Which GGML model size to load. See module docs — only `Small` is
/// currently the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTier {
    Small,
    Medium,
}

impl ModelTier {
    fn filename(self) -> &'static str {
        match self {
            ModelTier::Small => "ggml-small.bin",
            ModelTier::Medium => "ggml-medium.bin",
        }
    }

    fn download_url(self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.filename()
        )
    }
}

/// `~/Library/Application Support/Mutter/models/`. Created on first use.
fn models_dir() -> Result<PathBuf, EngineError> {
    let home =
        std::env::var("HOME").map_err(|_| EngineError::ModelNotLoaded("$HOME not set".into()))?;
    let dir = PathBuf::from(home).join("Library/Application Support/Mutter/models");
    std::fs::create_dir_all(&dir)
        .map_err(|e| EngineError::ModelNotLoaded(format!("could not create models dir: {e}")))?;
    Ok(dir)
}

/// Download `tier`'s model file via `curl` if it isn't already on disk.
/// Shells out to `curl` rather than adding an HTTP client dependency (e.g.
/// reqwest) — this is a one-time, macOS-only operation and `curl` ships with
/// the OS, consistent with the project's minimal-dependency stance. This is
/// also the one place in the whole app that makes a network call, and only
/// runs when the model file is missing — see CLAUDE.md's "zero network
/// calls after model download" constraint.
///
/// NOTE: sha256 pinning (docs/mutter-project-plan.md Section 15's "GGML
/// model-routing/sha256" note) is intentionally not wired here — hardcoding
/// a guessed hash would be worse than no check at all. Before this ships
/// past Phase 0, pin the real digest from whisper.cpp's published checksums
/// (ggerganov/whisper.cpp `models/README.md`).
fn ensure_model_downloaded(tier: ModelTier) -> Result<PathBuf, EngineError> {
    let path = models_dir()?.join(tier.filename());
    if path.exists() {
        return Ok(path);
    }

    tracing::info!(model = tier.filename(), "downloading whisper model");
    let tmp_path = path.with_extension("part");
    let status = Command::new("curl")
        .args(["-fSL", "--retry", "3", "-o"])
        .arg(&tmp_path)
        .arg(tier.download_url())
        .status()
        .map_err(|e| EngineError::ModelNotLoaded(format!("curl failed to start: {e}")))?;

    if !status.success() {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(EngineError::ModelNotLoaded(format!(
            "model download failed (curl exit status {status})"
        )));
    }

    std::fs::rename(&tmp_path, &path).map_err(|e| {
        EngineError::ModelNotLoaded(format!("could not finalize model file: {e}"))
    })?;
    Ok(path)
}

pub struct WhisperEngine {
    tier: ModelTier,
    context: OnceCell<Arc<WhisperContext>>,
}

impl WhisperEngine {
    pub fn new() -> Self {
        Self::with_tier(ModelTier::Small)
    }

    pub fn with_tier(tier: ModelTier) -> Self {
        Self {
            tier,
            context: OnceCell::new(),
        }
    }

    async fn context(&self) -> Result<Arc<WhisperContext>, EngineError> {
        let ctx = self
            .context
            .get_or_try_init(|| async {
                let tier = self.tier;
                tokio::task::spawn_blocking(move || -> Result<Arc<WhisperContext>, EngineError> {
                    let path = ensure_model_downloaded(tier)?;
                    let path_str = path.to_string_lossy().into_owned();
                    // Native FFI call — CLAUDE.md requires whisper-rs/bridge
                    // calls wrapped in catch_unwind so a panic in the C++
                    // library can never take down the whole app.
                    let ctx = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        WhisperContext::new_with_params(
                            &path_str,
                            WhisperContextParameters::default(),
                        )
                    }))
                    .map_err(|_| {
                        EngineError::InferenceFailed(
                            "whisper-rs panicked while loading the model".into(),
                        )
                    })?
                    .map_err(|e| EngineError::ModelNotLoaded(e.to_string()))?;
                    Ok(Arc::new(ctx))
                })
                .await
                .map_err(|e| {
                    EngineError::InferenceFailed(format!("model load task panicked: {e}"))
                })?
            })
            .await?;
        Ok(ctx.clone())
    }
}

impl Default for WhisperEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TranscriptionEngine for WhisperEngine {
    async fn transcribe(&self, audio: &[f32]) -> Result<Transcript, EngineError> {
        let context = self.context().await?;
        let audio = audio.to_vec();

        tokio::task::spawn_blocking(move || -> Result<Transcript, EngineError> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut state = context
                    .create_state()
                    .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;

                let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                // Auto-detect, never manual — docs/mutter-project-plan.md Section 10.
                params.set_language(None);
                params.set_detect_language(true);
                params.set_translate(false);
                params.set_print_special(false);
                params.set_print_progress(false);
                params.set_print_realtime(false);
                params.set_print_timestamps(false);

                state
                    .full(params, &audio)
                    .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;

                let n_segments = state
                    .full_n_segments()
                    .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;
                let mut text = String::new();
                for i in 0..n_segments {
                    let segment = state
                        .full_get_segment_text(i)
                        .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;
                    text.push_str(&segment);
                }

                let lang_id = state
                    .full_lang_id_from_state()
                    .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;
                let language = whisper_rs::get_lang_str(lang_id)
                    .unwrap_or("unknown")
                    .to_string();

                Ok(Transcript {
                    text: text.trim().to_string(),
                    language,
                })
            }))
            .map_err(|_| {
                EngineError::InferenceFailed("whisper-rs panicked during inference".into())
            })?
        })
        .await
        .map_err(|e| EngineError::InferenceFailed(format!("inference task panicked: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_tiers_have_distinct_filenames() {
        assert_ne!(ModelTier::Small.filename(), ModelTier::Medium.filename());
    }

    #[test]
    fn download_url_points_at_the_right_filename() {
        assert!(ModelTier::Small.download_url().ends_with("ggml-small.bin"));
        assert!(ModelTier::Medium.download_url().ends_with("ggml-medium.bin"));
    }

    // Real model loading + inference needs a ~500MB download and a real
    // audio sample — exercised by the Phase 0 benchmark harness
    // (examples/whisper_benchmark.rs), not an automated unit test.
}
