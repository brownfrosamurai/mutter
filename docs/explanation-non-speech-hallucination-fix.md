# Why transcripts don't say "[BLANK_AUDIO]" anymore

`engine/whisper.rs` fixes a real, user-visible bug: Whisper occasionally inserted literal bracketed tags like `[BLANK_AUDIO]` or `[MUSIC]` into transcripts, and those tags got typed straight into whatever the user was dictating into. This document explains why that happened and the three-layer fix, since the mechanism isn't obvious from the code alone.

## The problem

OpenAI's Whisper models (and by extension whisper.cpp, which `WhisperEngine` wraps via `whisper-rs`) were trained on data that includes closed-caption-style annotations for non-speech audio — silence, background music, applause, and so on. When the model is fed a segment with little or no actual speech, it can "hallucinate" one of these training-data annotations as if it were transcribed text, rather than correctly outputting nothing. Real examples pulled from this app's own history before the fix:

```
"Let us develop topic 5. [BLANK_AUDIO]."
"Go ahead. [MUSIC]."
```

Since `session.rs` injects whatever text the engine returns directly into the focused app, these tags landed in real dictated documents and messages — not a cosmetic issue.

## The fix: three independent layers

### 1. Tell whisper.cpp to never generate these tokens (the real fix)

whisper.cpp maintains a curated list of "non-speech" symbol tokens — `[`, `]`, `(`, `)`, music notes, and similar — and has a built-in mechanism (`suppress_non_speech_tokens`) to suppress their logits at decode time, so the model is structurally incapable of generating them. This parameter defaults to `false` in both whisper.cpp and `whisper-rs`, and `WhisperEngine::transcribe` simply never set it:

```rust
params.set_suppress_non_speech_tokens(true);
```

Because `[` and `]` are in that suppression list, the model can no longer open a bracket at all — it can't produce `[BLANK_AUDIO]` or any similar tag, since that requires generating the bracket characters first. This is the upstream, sanctioned fix for exactly this class of hallucination (see whisper.cpp's own source comment pointing at `openai/whisper`'s `tokenizer.py`, which introduced this suppression list for the same reason).

### 2. Trim silence before inference even runs (`trim_silence`)

A toggle-hotkey dictation flow (press, pause, speak, pause, press again) routinely produces real dead air at both ends of a recording. `trim_silence` does a cheap windowed-RMS pass over the audio (10ms windows, a conservative threshold, 100ms of padding kept on each side so a real word's onset/offset never gets clipped) and trims contiguous near-silence from the start and end before the audio is ever handed to whisper.cpp.

This does two things at once:
- **Speed** — the encoder's cost scales with input length, so less audio in means faster inference.
- **Correctness** — leading/trailing silence is exactly the input shape most likely to provoke a hallucination in the first place, so removing it removes many potential hallucinations before inference ever runs, independent of layer 1.

If the *entire* clip is below the silence threshold, `trim_silence` returns `None` and `transcribe()` skips calling whisper.cpp entirely, returning an empty `Transcript` immediately — a segment that's pure silence costs nothing.

### 3. A defensive string-level backstop (`strip_non_speech_annotations`)

Even with layers 1 and 2, this is a defensive backstop: it strips any residual `[...]` span from the final transcript text. Real dictated speech never legitimately produces literal square brackets — there's no spoken-formatting phrase for them (unlike "comma" → `,` or "period" → `.`, see [`explanation-grammar-pipeline.md`](explanation-grammar-pipeline.md)) — so this is a safe, low-risk strip.

The implementation is slightly more involved than a naive "delete text between brackets" because of what it leaves behind. Given `"Go ahead. [MUSIC]."` (a real captured example — the tag sits directly between two sentence-ending periods), naively deleting just `[MUSIC]` leaves `"Go ahead. ."` — an orphaned space and duplicate period. The function instead:

1. Collapses to exactly one removed separator around the tag (not zero, not two) — a tag flanked by spaces on both sides (`"word [TAG] word"`) must leave a single space behind, not a doubled or missing one.
2. Runs a final pass that collapses immediately-repeated terminal punctuation (`..` → `.`) left behind when a tag sat directly between two sentence-enders.

Result: `"Go ahead. [MUSIC]."` → `"Go ahead."` — not `"Go ahead. ."`.

## Trade-offs

- **`SILENCE_RMS_THRESHOLD` is a tuned heuristic, not measured against a real recording corpus** (this development environment has no microphone) — chosen conservatively based on typical quiet-room noise floors vs. speech levels. If real dogfooding ever shows it clipping quiet speech onsets, it's the first thing to revisit.
- **Three layers is deliberate redundancy, not indecision.** Layer 1 is the structural fix and should catch nearly everything; layer 2 helps speed independently of the hallucination problem; layer 3 is cheap insurance against any residual case (a future model swap, an edge case in tokenization) that layers 1-2 don't catch. Given the failure mode is literal garbage text typed into a user's real document, the extra insurance was judged worth the small amount of code.

## Related

- [`reference-architecture.md`](reference-architecture.md) — where `WhisperEngine` sits in the pipeline
- [`explanation-grammar-pipeline.md`](explanation-grammar-pipeline.md) — the separate `TextProcessor` stage this transcript then passes through
- `src-tauri/src/engine/whisper.rs` — `trim_silence`, `strip_non_speech_annotations`, and their unit tests
