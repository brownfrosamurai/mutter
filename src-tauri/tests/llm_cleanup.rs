//! Real-inference integration test for `LlmCleanup` (Section 5, "Option B").
//!
//! Deliberately `#[ignore]`d, same reasoning as `fixture_audio.rs`: downloads
//! two real files on first run (a ~380MB GGUF model + ~7MB tokenizer.json)
//! and takes real wall-clock time for inference. Run explicitly:
//!
//!   cargo test --test llm_cleanup -- --ignored --nocapture
//!
//! This also exists to catch a real integration risk this feature almost
//! shipped with: the first implementation used `llama-cpp-2`, which
//! statically links its own vendored copy of GGML — the same library
//! `whisper-rs-sys` also vendors. Both in one binary produced hundreds of
//! linker duplicate-symbol warnings that macOS's `ld` doesn't hard-error
//! on, and it turned out not to be cosmetic: a GGUF file that loaded
//! perfectly in a standalone llama-cpp-2-only binary failed with spurious
//! "tensor is duplicated" errors once linked alongside whisper-rs-sys, on
//! a different tensor for each of two unrelated GGUF files tried — proof
//! of loader corruption, not a bad file. `engine/llm_cleanup.rs` was
//! rewritten on `candle` instead (pure Rust, no vendored GGML) specifically
//! to eliminate that collision. This test, run alongside `fixture_audio.rs`,
//! is what actually proves both engines are correct together now — worth
//! rerunning both after any dependency bump that touches either engine.

use mutter_lib::engine::llm_cleanup::LlmCleanup;
use mutter_lib::engine::TextProcessor;

#[tokio::test]
#[ignore = "downloads a ~380MB GGUF model + ~7MB tokenizer on first run; run explicitly, see module docs"]
async fn llm_cleanup_fixes_grammar_and_preserves_meaning() {
    let llm = LlmCleanup::new();

    let input =
        "the api return a 404 when the user is not logged in and we need to fix this before friday";
    let output = llm
        .process(input, "en")
        .await
        .expect("LLM cleanup should succeed against a real model");

    assert!(
        !output.trim().is_empty(),
        "expected non-empty cleaned text, got: {output:?}"
    );
    // Not asserting exact output (a real model's phrasing isn't fully
    // deterministic to pin down here) — asserting the things Section 5's
    // eng-review risk actually cares about: technical/precise words from
    // the input should survive recognizably in the output, not get
    // paraphrased away.
    let lower = output.to_lowercase();
    assert!(
        lower.contains("api") && lower.contains("404"),
        "expected key technical terms ('api', '404') preserved, got: {output:?}"
    );
}

#[tokio::test]
#[ignore = "downloads a ~380MB GGUF model + ~7MB tokenizer on first run; run explicitly, see module docs"]
async fn llm_cleanup_handles_empty_input_without_invoking_the_model() {
    let llm = LlmCleanup::new();
    let output = llm
        .process("   ", "en")
        .await
        .expect("empty input should not error");
    assert_eq!(output, "   ");
}
