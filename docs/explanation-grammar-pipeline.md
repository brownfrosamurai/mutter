# Why grammar cleanup is Option A + optional Option B, and why B is always-on

Every transcript passes through `engine::pipeline::GrammarPipeline` before injection. This document explains the two-option design, and the specific, deliberate risk the project accepted when Option B's design changed from the original plan.

## Option A: rule-based cleanup, always on

`engine/grammar.rs`'s `RuleBasedCleanup` runs unconditionally on every transcript — five small, independently-toggleable, pure-function steps (corrections → formatting → filler-removal → capitalise → tidy-punctuation; see [`reference-settings.md`](reference-settings.md) for exactly why that order). It's fast, fully offline, and adds no model weight. Two of the five steps (capitalise, tidy-punctuation) are the app's *original* pre-refactor behavior, generalized into toggles without changing their default-on behavior — a mandatory regression test (`full_pipeline_at_defaults_matches_pre_rewrite_output_on_plain_text`) exists specifically to prove the refactor didn't silently change output for existing users.

Rule-based matching has a documented ceiling, not a bug to chase to 100%: "period" used mid-sentence as ordinary content ("a period of time") can't be told apart from "period" meaning punctuation by pattern matching alone, and the same ambiguity exists for "I meant" as a self-correction trigger vs. ordinary reported speech. `engine/grammar.rs`'s tests encode both as `documented_heuristic_limit` cases — expected, understood behavior, not something a future PR should try to "fix" without an actual NLU approach.

## Option B: local-LLM cleanup, toggleable, off by default

`engine/llm_cleanup.rs`'s `LlmCleanup` runs a small local model (Qwen2.5-0.5B-Instruct, GGUF Q4_K_M, ~380MB) over the Option A output when `AppSettings.grammar_llm_cleanup_enabled` is on. It exists to catch exactly what rule-based matching structurally can't — real language understanding instead of pattern matching.

### The design changed from the original plan, on purpose

`docs/mutter-project-plan.md` Section 5 originally specified Option B as a **per-transcript, user-triggered** action — something the user explicitly asks for on a given transcript, not something that runs on everything automatically. What's actually built is different: an **always-on pipeline stage**, gated by one Settings toggle that applies to every future transcript, not a per-transcript button.

This was a deliberate, user-confirmed reversal (2026-08-30), not scope creep an agent introduced on its own. The risk of that reversal was named explicitly *before* building it, not discovered after: Mutter's primary use case is dictating specs and bug reports to AI coding agents, which need precise technical vocabulary preserved exactly — variable names, exact phrasing, code fragments spoken aloud. An always-on LLM pass risks paraphrasing exactly that vocabulary, which a per-transcript opt-in would have let the user avoid on the dictations where it mattered most. The user chose always-on anyway, fully aware of the trade, over the safer per-transcript design.

The one concrete mitigation this risk got, at the prompt level: `SYSTEM_PROMPT` is deliberately blunt about preserving exact wording —

> "Preserve technical terms, code, proper nouns, numbers, and the speaker's exact meaning — never paraphrase, summarize, add information, or remove information."

This doesn't eliminate the risk (that's the trade-off the user accepted), but a model this small follows a sharp, narrow instruction more reliably than a vague one.

## Why `GrammarPipeline` always falls back to Option A's output on any Option B failure

```rust
match self.llm.process(&cleaned, language).await {
    Ok(polished) => Ok(polished),
    Err(e) => {
        tracing::error!(error = %e, "LLM grammar cleanup failed, falling back to rule-based output");
        Ok(cleaned)
    }
}
```

A broken or unavailable optional enhancement must never block a transcript from being inserted at all. If the model failed to load, the download failed, or inference panicked, the user still gets Option A's already-computed output — worse than a perfect Option B result, but never nothing.

## Why `candle`, not `llama-cpp-2`

The first implementation used `llama-cpp-2`, and it was live-verified broken: `llama-cpp-sys-2` and `whisper-rs-sys` (already in this binary for the transcription engine) each statically link their own independently-vendored copy of GGML. The linker reports hundreds of duplicate-symbol warnings on macOS, which doesn't hard-error — it silently picks one definition per symbol, and this measurably corrupted GGUF loading: the exact same GGUF file that loaded perfectly in a standalone `llama-cpp-2`-only binary failed with spurious "tensor is duplicated" errors once linked alongside `whisper-rs-sys`, and the specific tensor reported changed between two unrelated GGUF files — a pattern that only makes sense as loader corruption, not two coincidentally-bad files.

`candle` (Hugging Face's pure-Rust ML framework) has no vendored C/GGML dependency at all — its GPU backends are pure Rust plus Metal/CUDA bindings — so it can't collide with `whisper-rs-sys` the same way. Confirmed by loading the exact GGUF file that failed under `llama-cpp-2` successfully under `candle`, in the same binary as `whisper-rs`, with a clean relink showing zero duplicate-symbol warnings.

## Related

- [`reference-settings.md`](reference-settings.md) — the toggle fields and defaults
- [`reference-architecture.md`](reference-architecture.md)
- `TODOS.md`'s "LLM-based (Option B style) spoken-corrections/spoken-formatting" entry — the next place this same "wait for real dogfooding signal" discipline would apply
