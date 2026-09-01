# How to run the ignored integration tests

`cargo test` (no flags) runs 75 fast unit tests and skips four real-inference integration tests that need model downloads and genuine wall-clock inference time. This guide covers running each of those explicitly, and what they actually prove.

## Prerequisites

- A working `cargo build` in `src-tauri/`
- A real network connection (each test downloads real model weights on first run — the only network calls this app ever makes outside of update checks)
- Real disk space: up to ~2.4GB across all three test files' models combined, cached at `~/Library/Application Support/Mutter/models/` and reused on subsequent runs

## The four tests

| Test file | What it proves | First-run download |
|---|---|---|
| `tests/fixture_audio.rs` | `WhisperEngine` actually transcribes real audio end-to-end (not mocked) | ~500MB (Whisper Small) |
| `tests/language_benchmark.rs` | Whisper Small vs. Medium accuracy/latency, per language, against known ground truth | ~500MB (Small) + ~1.5GB (Medium) |
| `tests/llm_cleanup.rs` | `LlmCleanup` (Option B) runs real inference and improves grammar without corrupting meaning | ~380MB GGUF + ~7MB tokenizer |
| `bindings_export::export_bindings` (in `lib.rs`, not `tests/`) | Not an integration test — codegen. Covered in [How to add or change a Tauri command](howto-add-a-tauri-command.md), not here | none |

## Steps

1. **Fixture-audio** (the fastest of the three — one short pre-recorded clip, no benchmark loop):

   ```bash
   cd src-tauri
   cargo test --test fixture_audio -- --ignored --nocapture
   ```

   Confirms the real, checked-in `tests/fixtures/sample-en.wav` (a short synthesized clip, ~170KB) transcribes to real English text with the correct detected language — a genuine pipeline smoke test, not the accuracy benchmark below.

2. **Language benchmark** (slower — runs Small and Medium, per available language):

   ```bash
   cargo test --test language_benchmark -- --ignored --nocapture
   ```

   English is fully automated (synthesized via macOS `say` from known ground-truth text, objectively scored). **Yoruba has no automated path on a machine without a Yoruba `say` voice** — the test looks for `tests/fixtures/lang-benchmark/yo.wav` + `yo.txt` and reports "skipped" until a human supplies both; it does not fail. See `TODOS.md`'s "PARKED — Yoruba accuracy benchmark" entry before spending time chasing this.

3. **LLM cleanup** (Option B, real inference):

   ```bash
   cargo test --test llm_cleanup -- --ignored --nocapture
   ```

   Also the regression test for a real, previously-shipped bug: an earlier `llama-cpp-2`-based implementation silently corrupted GGUF loading when linked alongside `whisper-rs-sys` (both vendor their own GGML, and the linker's silent duplicate-symbol resolution picked the wrong one). Worth rerunning **alongside** `fixture_audio.rs` after bumping any dependency that touches either engine — that's what actually proves both engines are still correct together in the same binary, not just individually.

## Verification

Each test prints its own real numbers via `--nocapture` — read the output, don't just check the exit code. `fixture_audio.rs` prints the actual transcribed text and detected language; `language_benchmark.rs` prints per-language accuracy percentages and latency; `llm_cleanup.rs` prints the before/after text so you can eyeball whether meaning was preserved.

## Troubleshooting

- **Download fails or hangs** — these shell out to `curl -fSL --retry 3`; check your network directly, and check `~/Library/Application Support/Mutter/models/` for a stray `.part` file from an interrupted download (safe to delete, it'll re-download).
- **`language_benchmark.rs` reports Yoruba as skipped** — expected on a machine with no Yoruba `say` voice (check with `say -v '?'`). Not a bug; see step 2 above.
- **Tests are slow** — that's the point; they're excluded from the default `cargo test` run and from CI specifically because real inference takes real time. Don't try to make these fast.

## Related

- [How to add or change a Tauri command](howto-add-a-tauri-command.md) — the `export_bindings` codegen step, a different kind of `#[ignore]`d test
- [`reference-architecture.md`](reference-architecture.md) — `engine/whisper.rs` and `engine/llm_cleanup.rs`
