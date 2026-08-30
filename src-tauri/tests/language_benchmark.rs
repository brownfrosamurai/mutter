//! Phase 0 language/model benchmark (docs/mutter-project-plan.md Section 6,
//! Section 15's Phase 0 deliverable, Section 17's assignment) — Whisper
//! `Small` vs `Medium`, per language, with real latency and an objective
//! accuracy number.
//!
//! Scoped to English and Yoruba for now (English + Yoruba are the two
//! poles of Section 6's risk: English is Whisper's best-resourced language,
//! Yoruba its named worst) rather than all six — narrow first, widen later
//! if useful.
//!
//! **What this can and can't replace.** Section 17 literally asks the user
//! to try Apple's built-in dictation by ear. An agent has no microphone and
//! no ears, so that can't be done here. What *can* be done without a
//! human: synthesize speech via a macOS `say` voice from **known
//! ground-truth text**, run it through `WhisperEngine`, and objectively
//! score the transcript against that known text — real audio, real
//! inference, a real number, no ear required.
//!
//! **Yoruba specifically has no automated path here.** `say -v '?'` on this
//! machine lists no Yoruba voice at all — there is no way to synthesize
//! the one language the plan names as the highest accuracy risk. This test
//! looks for `tests/fixtures/lang-benchmark/yo.wav` +
//! `tests/fixtures/lang-benchmark/yo.txt` (the exact words spoken, one
//! line) at runtime; if a human drops in a real recording and its
//! transcript, it's picked up automatically with zero code changes. Until
//! then it's reported as skipped, every run, not silently omitted.
//!
//! Also out of scope here: `AppleSpeechEngine` (still a stub — nothing to
//! benchmark against yet) and real accented/noisy human speech (TTS audio
//! is about as easy as input audio gets, so these numbers are a ceiling on
//! real-world accuracy, not a floor).
//!
//! Deliberately `#[ignore]`d and not a pass/fail gate on accuracy — this is
//! a diagnostic to inform the Section 6 model-routing decision, not a CI
//! regression test. Run with:
//!   cargo test --test language_benchmark -- --ignored --nocapture

use mutter_lib::engine::whisper::{ModelTier, WhisperEngine};
use mutter_lib::engine::TranscriptionEngine;

struct Case {
    lang: &'static str,
    fixture: String,
    expected_text: String,
}

const YORUBA_FIXTURE: &str = "tests/fixtures/lang-benchmark/yo.wav";
const YORUBA_TRANSCRIPT: &str = "tests/fixtures/lang-benchmark/yo.txt";

fn active_cases() -> Vec<Case> {
    let mut cases = vec![Case {
        lang: "en",
        fixture: "tests/fixtures/lang-benchmark/en.wav".to_string(),
        expected_text:
            "The quick brown fox jumps over the lazy dog while the developer writes code."
                .to_string(),
    }];

    if let Ok(text) = std::fs::read_to_string(YORUBA_TRANSCRIPT) {
        if std::path::Path::new(YORUBA_FIXTURE).exists() {
            cases.push(Case {
                lang: "yo",
                fixture: YORUBA_FIXTURE.to_string(),
                expected_text: text.trim().to_string(),
            });
        }
    }

    cases
}

fn load_wav_as_f32(path: &str) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path)
        .unwrap_or_else(|e| panic!("failed to open fixture {path}: {e}"));
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000, "fixture must be 16kHz: {path}");
    assert_eq!(spec.channels, 1, "fixture must be mono: {path}");
    reader
        .samples::<i16>()
        .map(|s| s.expect("fixture WAV should decode cleanly") as f32 / i16::MAX as f32)
        .collect()
}

/// Word-level accuracy: `1 - (edit_distance / max(ref_len, hyp_len))`,
/// case-folded and punctuation-stripped on both sides. Not a rigorous WER
/// implementation (real WER has separate insertion/deletion/substitution
/// accounting) — good enough for "is this in the right ballpark, and does
/// Medium meaningfully beat Small" without pulling in a WER crate for one
/// diagnostic test.
fn word_accuracy(expected: &str, actual: &str) -> f64 {
    let normalize = |s: &str| -> Vec<String> {
        s.chars()
            .filter(|c| !c.is_ascii_punctuation())
            .collect::<String>()
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect()
    };
    let a = normalize(expected);
    let b = normalize(actual);
    if a.is_empty() {
        return if b.is_empty() { 1.0 } else { 0.0 };
    }

    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    let edit_distance = dp[n][m] as f64;
    1.0 - (edit_distance / n.max(m) as f64)
}

#[tokio::test]
#[ignore = "downloads both whisper Small (~500MB) and Medium (~1.5GB) models; run explicitly, see module docs"]
async fn whisper_small_vs_medium_per_language() {
    let small = WhisperEngine::with_tier(ModelTier::Small);
    let medium = WhisperEngine::with_tier(ModelTier::Medium);
    let cases = active_cases();

    let yoruba_ran = cases.iter().any(|c| c.lang == "yo");
    if !yoruba_ran {
        println!(
            "\nYoruba: SKIPPED — no {YORUBA_FIXTURE} / {YORUBA_TRANSCRIPT} found, and no macOS \
             `say` voice exists to synthesize one. Drop in a real recording + its exact spoken \
             text (one line, no fixture format needed beyond that) and this benchmark picks it \
             up automatically next run."
        );
    }

    // Warm up both engines before timing anything — `transcribe()` lazily
    // loads the model on its first call (same as the real app), and that
    // one-time load cost (~1.5GB for Medium) would otherwise swamp the
    // first case's "latency" column and make it look like Medium takes
    // 19 minutes per utterance, which is nonsense. This mirrors what T8
    // already guarantees in the real app: load once, resident after.
    if let Some(first) = cases.first() {
        let warmup_audio = load_wav_as_f32(&first.fixture);
        let _ = small.transcribe(&warmup_audio).await;
        let _ = medium.transcribe(&warmup_audio).await;
    }

    println!(
        "\n{:<4} {:<8} {:>10} {:>9} {:>10} {:>9}",
        "lang", "tier", "detected", "accuracy", "latency", "match?"
    );
    println!("{}", "-".repeat(60));

    for case in &cases {
        let audio = load_wav_as_f32(&case.fixture);

        for (tier_name, engine) in [("small", &small), ("medium", &medium)] {
            let start = std::time::Instant::now();
            let result = engine.transcribe(&audio).await;
            let latency = start.elapsed();

            match result {
                Ok(transcript) => {
                    let accuracy = word_accuracy(&case.expected_text, &transcript.text);
                    let lang_match = transcript.language == case.lang;
                    println!(
                        "{:<4} {:<8} {:>10} {:>8.1}% {:>9.2?} {:>9}",
                        case.lang,
                        tier_name,
                        transcript.language,
                        accuracy * 100.0,
                        latency,
                        lang_match,
                    );
                    println!("     expected: {}", case.expected_text);
                    println!("     got:      {}", transcript.text);
                }
                Err(e) => {
                    println!(
                        "{:<4} {:<8} {:>10} {:>9} {:>10} {:>9}",
                        case.lang, tier_name, "ERROR", "-", "-", "-"
                    );
                    println!("     error: {e}");
                }
            }
        }
    }
}
