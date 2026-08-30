//! Fixture-audio integration test — Section 11's CI item ("an integration
//! test running fixture audio through the full pipeline per registered
//! engine") and Section 21's Test Issue: a real audio sample exercised
//! through the actual `WhisperEngine`, not a mock.
//!
//! Deliberately `#[ignore]`d rather than run by default or wired into CI:
//! it downloads a ~500MB whisper model on first run (a real network call —
//! the one and only one this app ever makes, per CLAUDE.md's "zero network
//! calls after model download" constraint, which doesn't cover the download
//! itself) and takes real wall-clock time for inference. Run explicitly:
//!
//!   cargo test --test fixture_audio -- --ignored --nocapture
//!
//! `tests/fixtures/sample-en.wav` is a short synthesized-speech clip
//! (macOS `say` -> `afconvert` to 16kHz mono PCM) checked into the repo —
//! not a recording of a real person, and small enough (~170KB) to live in
//! git. Good enough to prove the pipeline wires together correctly; it is
//! NOT the Section 17 accuracy benchmark (that needs real human speech
//! across all six languages and a human judging correctness — this test
//! only asserts the engine returns *some* plausible English text).

use hound::WavReader;
use mutter_lib::engine::whisper::WhisperEngine;
use mutter_lib::engine::TranscriptionEngine;

fn load_fixture_as_f32(path: &str) -> Vec<f32> {
    let mut reader = WavReader::open(path).expect("fixture WAV should open");
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000, "whisper.cpp expects 16kHz audio");
    assert_eq!(spec.channels, 1, "fixture should already be mono");

    reader
        .samples::<i16>()
        .map(|s| s.expect("fixture WAV should decode cleanly") as f32 / i16::MAX as f32)
        .collect()
}

#[tokio::test]
#[ignore = "downloads a ~500MB whisper model on first run; run explicitly, see module docs"]
async fn whisper_engine_transcribes_fixture_audio() {
    let audio = load_fixture_as_f32("tests/fixtures/sample-en.wav");
    assert!(!audio.is_empty(), "fixture audio should not be empty");

    let engine = WhisperEngine::new();
    // session.rs calls ensure_ready() before the first transcribe() to show
    // a distinct pill "loading" state (Section 6, Performance Issue 8) —
    // exercise that same sequence here, not just a bare transcribe() call,
    // so a regression in that path (e.g. ensure_ready() not actually
    // populating the same lazily-loaded context transcribe() reuses) would
    // show up as this test hanging or re-downloading rather than passing
    // silently.
    engine
        .ensure_ready()
        .await
        .expect("ensure_ready should load the model successfully");
    let transcript = engine
        .transcribe(&audio)
        .await
        .expect("transcription should succeed against real fixture audio");

    assert!(
        !transcript.text.trim().is_empty(),
        "expected non-empty transcribed text, got: {transcript:?}"
    );
    assert_eq!(
        transcript.language, "en",
        "fixture is English speech, expected auto-detected language 'en', got: {transcript:?}"
    );
}
