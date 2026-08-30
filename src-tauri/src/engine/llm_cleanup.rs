//! Local-LLM grammar/word-choice cleanup — Section 5's "Option B": a
//! quantized instruction model, invoked only on the transcript text, never
//! the audio.
//!
//! **Decision (2026-08-30, user-confirmed):** built and wired as an
//! **always-on** pipeline stage, toggled in Settings (off by default) —
//! not the plan's original recommendation of a per-transcript,
//! user-triggered action. The eng review's own risk callout still applies
//! and is worth restating here, not just in the plan doc: the primary
//! named use case (dictating to AI coding agents) needs precise technical
//! vocabulary preserved, not paraphrased, and an always-on pass risks
//! corrupting exactly that. The user chose always-on anyway, fully aware of
//! that risk, over the safer per-transcript design — see
//! `docs/mutter-project-plan.md` Section 5. `GrammarPipeline` (pipeline.rs)
//! is what actually gates this behind the toggle and falls back to Option
//! A's output if this stage errors, so a broken/unavailable model can never
//! block a transcript from being inserted.
//!
//! Model: Qwen2.5-0.5B-Instruct, GGUF Q4_K_M (bartowski's re-quantization,
//! ~380MB) + its base repo's `tokenizer.json` (~7MB) — see
//! `ensure_model_downloaded`'s doc comment for exactly why two files and
//! why that specific GGUF source. Chosen smaller than the plan's upper
//! "1-3B" suggestion — this stage now runs on every dictation, including
//! short ones, so it directly competes with "blazing fast"; a smaller
//! model is the more conservative choice until real latency numbers exist
//! (mirroring how Small vs. Medium was decided for Whisper, not guessed).
//! Downloaded to the same `~/Library/Application Support/Mutter/models/`
//! directory whisper.rs uses, on first use only.
//!
//! Uses `candle` (Hugging Face's pure-Rust ML framework), not
//! `llama-cpp-2` — verified live 2026-08-30 that `llama-cpp-2` cannot
//! safely coexist with whisper-rs in this binary: both statically link
//! their own independently-vendored copy of GGML, and the linker's silent
//! duplicate-symbol resolution measurably corrupts GGUF loading (see
//! Cargo.toml's comment on the `candle` dependency for the full story).
//! `candle` has no vendored C/GGML dependency, so it can't collide with
//! whisper-rs-sys the same way — confirmed by loading the exact same GGUF
//! file that failed under llama-cpp-2 successfully here.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use candle::quantized::gguf_file;
use candle::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights as Qwen2;
use tokenizers::Tokenizer;
use tokio::sync::OnceCell;

use super::{EngineError, TextProcessor};

const MODEL_FILENAME: &str = "qwen2.5-0.5b-instruct-q4_k_m.gguf";
const TOKENIZER_FILENAME: &str = "qwen2.5-0.5b-instruct-tokenizer.json";

/// Deliberately NOT Qwen's own official GGUF repo
/// (`Qwen/Qwen2.5-0.5B-Instruct-GGUF`) — verified live 2026-08-30 that it
/// fails to load with "invalid model: tensor 'token_embd.weight' is
/// duplicated" under this project's llama.cpp toolchain (a symptom that
/// turned out to be the GGML linker collision described in the module
/// docs above, not a bug in that specific file — but bartowski's
/// re-quantization loads cleanly and is a widely-used, actively
/// llama.cpp-version-tracking GGUF source, so it's what's used here).
const MODEL_DOWNLOAD_URL: &str = "https://huggingface.co/bartowski/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf";

/// `candle`'s GGUF loader doesn't reconstruct a `tokenizers::Tokenizer`
/// from the GGUF's own embedded vocab metadata — it needs a real
/// `tokenizer.json`, so one gets downloaded separately from the model's
/// original (non-GGUF) repo, same as the official candle
/// `quantized-qwen2-instruct` example does.
const TOKENIZER_DOWNLOAD_URL: &str =
    "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/main/tokenizer.json";

/// Deliberately blunt about preserving exact wording — this is the prompt-
/// level mitigation for the vocabulary-corruption risk named in the module
/// docs above. Doesn't eliminate the risk (that's the tradeoff the user
/// accepted by choosing always-on), but a model this small follows a sharp,
/// narrow instruction far more reliably than a vague one.
const SYSTEM_PROMPT: &str = "You are a transcription cleanup assistant. Fix grammar, \
punctuation, and word choice in the user's dictated text. Preserve technical terms, \
code, proper nouns, numbers, and the speaker's exact meaning — never paraphrase, \
summarize, add information, or remove information. Reply with ONLY the corrected \
text: no preamble, no quotation marks, no explanation.";

const EOS_TOKEN: &str = "<|im_end|>";
const MAX_NEW_TOKENS: usize = 512;

/// `~/Library/Application Support/Mutter/models/` — same directory
/// whisper.rs's `models_dir()` uses, just different files in it.
fn models_dir() -> Result<PathBuf, EngineError> {
    crate::paths::app_support_subdir("models")
        .map_err(|e| EngineError::ModelNotLoaded(format!("could not create models dir: {e}")))
}

/// Mirrors whisper.rs's `ensure_model_downloaded` (shell out to `curl`,
/// already on macOS, no new HTTP-client dependency — same rationale as
/// that function's doc comment) but for one named file instead of a
/// `ModelTier` match, since this needs two independent files (GGUF weights
/// + tokenizer.json) rather than one.
fn ensure_file_downloaded(filename: &str, url: &str) -> Result<PathBuf, EngineError> {
    let path = models_dir()?.join(filename);
    if path.exists() {
        return Ok(path);
    }

    tracing::info!(file = filename, "downloading grammar-cleanup LLM file");
    let tmp_path = path.with_extension("part");
    let status = Command::new("curl")
        .args(["-fSL", "--retry", "3", "-o"])
        .arg(&tmp_path)
        .arg(url)
        .status()
        .map_err(|e| EngineError::ModelNotLoaded(format!("curl failed to start: {e}")))?;

    if !status.success() {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(EngineError::ModelNotLoaded(format!(
            "download of {filename} failed (curl exit status {status})"
        )));
    }

    std::fs::rename(&tmp_path, &path)
        .map_err(|e| EngineError::ModelNotLoaded(format!("could not finalize {filename}: {e}")))?;
    Ok(path)
}

fn device() -> Device {
    match Device::new_metal(0) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Metal unavailable for grammar-cleanup LLM, falling back to CPU"
            );
            Device::Cpu
        }
    }
}

struct LoadedModel {
    model: std::sync::Mutex<Qwen2>,
    tokenizer: Tokenizer,
    eos_token: u32,
    device: Device,
}

pub struct LlmCleanup {
    loaded: OnceCell<Arc<LoadedModel>>,
}

impl LlmCleanup {
    pub fn new() -> Self {
        Self {
            loaded: OnceCell::new(),
        }
    }

    async fn loaded(&self) -> Result<Arc<LoadedModel>, EngineError> {
        let loaded = self
            .loaded
            .get_or_try_init(|| async {
                tokio::task::spawn_blocking(|| -> Result<Arc<LoadedModel>, EngineError> {
                    let gguf_path = ensure_file_downloaded(MODEL_FILENAME, MODEL_DOWNLOAD_URL)?;
                    let tokenizer_path =
                        ensure_file_downloaded(TOKENIZER_FILENAME, TOKENIZER_DOWNLOAD_URL)?;

                    // candle's inference path (Metal/gemm) is native code
                    // reached through unsafe FFI at the edges — CLAUDE.md's
                    // catch_unwind requirement for native/bridge calls
                    // applies here the same as it does to whisper-rs.
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let device = device();
                        let mut file = std::fs::File::open(&gguf_path).map_err(|e| {
                            EngineError::ModelNotLoaded(format!(
                                "could not open {}: {e}",
                                gguf_path.display()
                            ))
                        })?;
                        let content = gguf_file::Content::read(&mut file)
                            .map_err(|e| EngineError::ModelNotLoaded(e.to_string()))?;
                        let model = Qwen2::from_gguf(content, &mut file, &device)
                            .map_err(|e| EngineError::ModelNotLoaded(e.to_string()))?;

                        let tokenizer = Tokenizer::from_file(&tokenizer_path)
                            .map_err(|e| EngineError::ModelNotLoaded(e.to_string()))?;
                        let eos_token =
                            *tokenizer.get_vocab(true).get(EOS_TOKEN).ok_or_else(|| {
                                EngineError::ModelNotLoaded(format!(
                                    "tokenizer vocab missing expected eos token {EOS_TOKEN}"
                                ))
                            })?;

                        Ok(Arc::new(LoadedModel {
                            model: std::sync::Mutex::new(model),
                            tokenizer,
                            eos_token,
                            device,
                        }))
                    }))
                    .map_err(|_| {
                        EngineError::InferenceFailed("candle model/tokenizer load panicked".into())
                    })?
                })
                .await
                .map_err(|e| {
                    EngineError::InferenceFailed(format!("model load task panicked: {e}"))
                })?
            })
            .await?;
        Ok(loaded.clone())
    }
}

impl Default for LlmCleanup {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TextProcessor for LlmCleanup {
    async fn process(&self, text: &str, _language: &str) -> Result<String, EngineError> {
        if text.trim().is_empty() {
            return Ok(text.to_string());
        }

        let loaded = self.loaded().await?;
        let text = text.to_string();

        tokio::task::spawn_blocking(move || -> Result<String, EngineError> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| generate(&loaded, &text)))
                .map_err(|_| EngineError::InferenceFailed("candle inference panicked".into()))?
        })
        .await
        .map_err(|e| EngineError::InferenceFailed(format!("inference task panicked: {e}")))?
    }
}

/// Runs entirely synchronously on a blocking thread — see `process()`'s
/// `spawn_blocking` wrapper. `Qwen2`'s `forward()` takes `&mut self` (it
/// owns the KV cache internally), so `LoadedModel.model` is a plain
/// `std::sync::Mutex` rather than something lock-free — `TextProcessor`
/// calls are already serialized through `segment_worker`'s single FIFO
/// worker (session.rs), so this is never actually contended, just needed
/// to satisfy `Send + Sync` for the resident `Arc<LoadedModel>`.
fn generate(loaded: &LoadedModel, text: &str) -> Result<String, EngineError> {
    let prompt = format!(
        "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n<|im_start|>user\n{text}<|im_end|>\n<|im_start|>assistant\n"
    );

    let encoding = loaded
        .tokenizer
        .encode(prompt, true)
        .map_err(|e| EngineError::InferenceFailed(format!("failed to tokenize prompt: {e}")))?;
    let prompt_tokens = encoding.get_ids();

    let mut model = loaded
        .model
        .lock()
        .map_err(|_| EngineError::InferenceFailed("grammar-cleanup model lock poisoned".into()))?;

    let mut logits_processor = LogitsProcessor::from_sampling(0, Sampling::ArgMax);

    let input = Tensor::new(prompt_tokens, &loaded.device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;
    let logits = model
        .forward(&input, 0)
        .and_then(|t| t.squeeze(0))
        .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;
    let mut next_token = logits_processor
        .sample(&logits)
        .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;

    let mut generated = Vec::with_capacity(MAX_NEW_TOKENS);
    for index in 0..MAX_NEW_TOKENS {
        if next_token == loaded.eos_token {
            break;
        }
        generated.push(next_token);

        let input = Tensor::new(&[next_token], &loaded.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;
        let logits = model
            .forward(&input, prompt_tokens.len() + index)
            .and_then(|t| t.squeeze(0))
            .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;
        next_token = logits_processor
            .sample(&logits)
            .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;
    }

    let output = loaded
        .tokenizer
        .decode(&generated, true)
        .map_err(|e| EngineError::InferenceFailed(format!("failed to decode output: {e}")))?;
    let cleaned = output.trim().trim_matches('"').trim();

    if cleaned.is_empty() {
        Ok(text.to_string())
    } else {
        Ok(cleaned.to_string())
    }
}
