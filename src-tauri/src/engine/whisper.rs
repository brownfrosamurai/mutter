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
//! by default. `Medium` is wired and functional but nothing selects it —
//! `tests/language_benchmark.rs` measured Small at 100% accuracy on English
//! (same as Medium, 3x faster), so there's no case for routing to it while
//! v1 stays scoped to English. Yoruba and the other four originally-named
//! languages are parked by the user's own direction (2026-08-30, see
//! CLAUDE.md and `docs/mutter-project-plan.md` Section 6's status note) —
//! not removed, since auto-detection still transcribes them if dictated,
//! just not benchmarked or specifically tiered right now.

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
    crate::paths::app_support_subdir("models")
        .map_err(|e| EngineError::ModelNotLoaded(format!("could not create models dir: {e}")))
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

    std::fs::rename(&tmp_path, &path)
        .map_err(|e| EngineError::ModelNotLoaded(format!("could not finalize model file: {e}")))?;
    Ok(path)
}

/// Windowed-RMS silence threshold used by `trim_silence` below. Chosen as a
/// conservative floor for typical quiet-room mic noise (measured silence
/// tends to sit under ~0.003 RMS on 16-bit-equivalent float32 samples,
/// ordinary speech well above 0.02) — not derived from a real-world
/// recording corpus (this environment has no mic), so treat as a tunable
/// heuristic if real dogfooding ever shows it clipping quiet speech onsets.
const SILENCE_RMS_THRESHOLD: f32 = 0.006;
/// 10ms at the 16kHz rate `mic.rs` always resamples to before calling this
/// engine (`capture::mic::TARGET_SAMPLE_RATE`).
const SILENCE_WINDOW_SAMPLES: usize = 160;
/// 100ms of padding kept on each side of detected speech so a trim can never
/// clip a real word's onset/offset.
const SILENCE_PADDING_SAMPLES: usize = 1600;

/// Trims contiguous near-silence from both ends of `audio` before it's ever
/// handed to whisper.cpp. Two independent wins from one cheap O(n) pass over
/// data already in memory: (1) speed — the encoder's cost scales with input
/// length, and a toggle-hotkey dictation flow (press, pause, speak, pause,
/// press again) routinely has real dead air at both ends; (2) correctness —
/// leading/trailing silence is exactly the input shape that provokes
/// Whisper's own documented "[BLANK_AUDIO]"/"[MUSIC]"-style non-speech
/// hallucinations (see `strip_non_speech_annotations` below), so trimming it
/// away removes many of them before inference ever runs, not just after.
///
/// Returns `None` if the whole clip stays under threshold — nothing to
/// transcribe at all, letting the caller skip inference entirely.
fn trim_silence(audio: &[f32]) -> Option<&[f32]> {
    if audio.is_empty() {
        return None;
    }

    let loud_window = |w: &[f32]| -> bool {
        let rms = (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt();
        rms >= SILENCE_RMS_THRESHOLD
    };

    let windows: Vec<bool> = audio
        .chunks(SILENCE_WINDOW_SAMPLES)
        .map(loud_window)
        .collect();
    let first_loud = windows.iter().position(|&loud| loud)?;
    let last_loud = windows.iter().rposition(|&loud| loud)?;

    let start = (first_loud * SILENCE_WINDOW_SAMPLES).saturating_sub(SILENCE_PADDING_SAMPLES);
    let end = ((last_loud + 1) * SILENCE_WINDOW_SAMPLES + SILENCE_PADDING_SAMPLES).min(audio.len());
    Some(&audio[start..end])
}

/// Defensive backstop for the same non-speech-tag hallucination class
/// `trim_silence` and `set_suppress_non_speech_tokens` already fight
/// upstream: strips any residual `[...]` span (real dictated speech never
/// legitimately produces literal brackets — there's no spoken-formatting
/// phrase for them, unlike "comma"/"period") and cleans up the punctuation
/// artifact a removed tag can leave behind, e.g. "Go ahead. [MUSIC]."
/// (whisper's own real output, from history) -> "Go ahead.", not
/// "Go ahead. .".
fn strip_non_speech_annotations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            for c2 in chars.by_ref() {
                if c2 == ']' {
                    break;
                }
            }
            // Collapse to exactly one removed separator, never both: a
            // tag flanked by spaces on both sides ("word [TAG] word")
            // must leave a single space behind, not zero.
            let consumed_following_space = chars.peek() == Some(&' ');
            if consumed_following_space {
                chars.next();
            }
            if !consumed_following_space && out.ends_with(' ') {
                out.pop();
            }
        } else {
            out.push(c);
        }
    }

    let mut deduped = String::with_capacity(out.len());
    for c in out.chars() {
        if matches!(c, '.' | '!' | '?' | ',') && deduped.ends_with(c) {
            continue;
        }
        deduped.push(c);
    }
    deduped
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
    async fn ensure_ready(&self) -> Result<(), EngineError> {
        self.context().await?;
        Ok(())
    }

    async fn transcribe(&self, audio: &[f32]) -> Result<Transcript, EngineError> {
        let context = self.context().await?;
        let audio = audio.to_vec();

        tokio::task::spawn_blocking(move || -> Result<Transcript, EngineError> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let Some(trimmed) = trim_silence(&audio) else {
                    // Whole segment is below the silence threshold — skip
                    // inference entirely rather than pay a full encoder pass
                    // for known-silent audio (see trim_silence's doc
                    // comment; session.rs already treats an empty
                    // Transcript::text as a no-op, so this is a pure
                    // speed/correctness win with no new downstream case).
                    return Ok(Transcript {
                        text: String::new(),
                        language: "unknown".to_string(),
                    });
                };

                let mut state = context
                    .create_state()
                    .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;

                let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                // Auto-detect, never manual — docs/mutter-project-plan.md Section 10.
                // Leaving `language` unset (None) already makes whisper.cpp
                // auto-detect as part of the normal transcription pass.
                // Do NOT also call `set_detect_language(true)`: per
                // whisper.cpp's own source (whisper_full, the
                // `if (params.detect_language) { return 0; }` branch right
                // after language ID), that flag means "detect the language
                // and stop" — it skips transcription entirely. Setting it
                // silently produced an empty transcript on every real
                // recording; caught by the fixture-audio integration test
                // (tests/fixture_audio.rs), which got detected language
                // "en" back but an empty `text` until this was removed.
                params.set_language(None);
                params.set_translate(false);
                // whisper.cpp defaults n_threads to 4 regardless of the
                // machine — leaving real cores idle on anything with more
                // performance cores than that (every current Apple Silicon
                // Mac). Metal (see Cargo.toml) offloads the matmuls, but
                // mel-spectrogram extraction and CPU-side ops still scale
                // with this.
                let n_threads = std::thread::available_parallelism()
                    .map(|n| n.get() as i32)
                    .unwrap_or(4);
                params.set_n_threads(n_threads);
                params.set_print_special(false);
                params.set_print_progress(false);
                params.set_print_realtime(false);
                params.set_print_timestamps(false);
                // The real fix for whisper's "[BLANK_AUDIO]"/"[MUSIC]"-style
                // non-speech hallucinations: whisper.cpp maintains a curated
                // list of non-speech symbol tokens (incl. "[", "]", "(",
                // ")", music notes) and, when this is on, suppresses their
                // logits at decode time so the model can never generate them
                // — not a post-hoc string filter, the model structurally
                // can't open a bracket. Off by default in both whisper.cpp
                // and whisper-rs; never previously set here.
                params.set_suppress_non_speech_tokens(true);

                state
                    .full(params, trimmed)
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
                    text: strip_non_speech_annotations(text.trim()),
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

    // --- strip_non_speech_annotations ---

    #[test]
    fn strips_real_hallucinated_tags_from_history() {
        // Verbatim examples pulled from real Mutter transcripts.
        assert_eq!(
            strip_non_speech_annotations("Let us develop topic 5. [BLANK_AUDIO]."),
            "Let us develop topic 5."
        );
        assert_eq!(
            strip_non_speech_annotations("Go ahead. [MUSIC]."),
            "Go ahead."
        );
        assert_eq!(
            strip_non_speech_annotations(
                "Ensure the default height of the dashboard covers the entire matrix panel. [MUSIC]."
            ),
            "Ensure the default height of the dashboard covers the entire matrix panel."
        );
    }

    #[test]
    fn strips_a_leading_tag_without_leaving_a_leading_space() {
        assert_eq!(
            strip_non_speech_annotations("[MUSIC] hello world"),
            "hello world"
        );
    }

    #[test]
    fn strips_a_mid_sentence_tag_without_a_double_space() {
        assert_eq!(
            strip_non_speech_annotations("hello [NOISE] world"),
            "hello world"
        );
    }

    #[test]
    fn a_pure_tag_transcript_becomes_empty() {
        assert_eq!(strip_non_speech_annotations("[BLANK_AUDIO]"), "");
    }

    #[test]
    fn text_with_no_tags_is_unchanged() {
        assert_eq!(
            strip_non_speech_annotations("just a normal sentence."),
            "just a normal sentence."
        );
    }

    // --- trim_silence ---

    #[test]
    fn a_fully_silent_clip_trims_to_none() {
        let audio = vec![0.0_f32; SILENCE_WINDOW_SAMPLES * 10];
        assert!(trim_silence(&audio).is_none());
    }

    #[test]
    fn an_empty_clip_trims_to_none() {
        assert!(trim_silence(&[]).is_none());
    }

    #[test]
    fn loud_audio_padded_by_silence_trims_the_silence_but_keeps_padding() {
        let silence = vec![0.0_f32; SILENCE_WINDOW_SAMPLES * 20];
        let loud = vec![0.5_f32; SILENCE_WINDOW_SAMPLES * 5];
        let mut audio = silence.clone();
        audio.extend_from_slice(&loud);
        audio.extend_from_slice(&silence);

        let trimmed = trim_silence(&audio).expect("clip has loud audio");
        // Shorter than the original (real trimming happened)...
        assert!(trimmed.len() < audio.len());
        // ...but still covers every loud sample plus some padding margin.
        assert!(trimmed.len() >= loud.len());
    }

    #[test]
    fn all_loud_audio_is_returned_unchanged() {
        let audio = vec![0.5_f32; SILENCE_WINDOW_SAMPLES * 5];
        let trimmed = trim_silence(&audio).expect("clip is loud");
        assert_eq!(trimmed, &audio[..]);
    }

    #[test]
    fn download_url_points_at_the_right_filename() {
        assert!(ModelTier::Small.download_url().ends_with("ggml-small.bin"));
        assert!(ModelTier::Medium
            .download_url()
            .ends_with("ggml-medium.bin"));
    }

    // Real model loading + inference needs a ~500MB download and a real
    // audio sample — exercised by the ignored fixture-audio integration
    // test (tests/fixture_audio.rs), not a fast unit test.
}
