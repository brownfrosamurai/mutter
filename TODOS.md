# Mutter — open TODOs

Human-gated items an agent cannot close alone. See `CLAUDE.md`'s "Current
state" section and `docs/mutter-project-plan.md` for full context on each.

## PARKED — Yoruba accuracy benchmark (Section 6 / Section 17)

**Parked by the user, 2026-08-30 — not a blocker, not currently being
pursued.** v1 focus is English only for now. Whisper's multilingual model
still auto-detects and transcribes Yoruba (and the other four descoped
languages) exactly as before — nothing was removed — this is just dedicated
benchmarking/accuracy-hardening work on hold, not a functional regression.

If this scope comes back: Whisper Small vs. Medium accuracy on Yoruba is
still unmeasured. English is done (`tests/language_benchmark.rs`, run
2026-08-30: Small 100.0% / Medium 100.0%, Small ≈3x faster — Small is the
confirmed English default).

Blocked on: `say -v '?'` lists no Yoruba voice on this machine, so there is
no automatable proxy the way there was for English. Needs a human to supply:

- `src-tauri/tests/fixtures/lang-benchmark/yo.wav` — a real Yoruba recording
- `src-tauri/tests/fixtures/lang-benchmark/yo.txt` — its exact ground-truth transcript

The moment both exist, `cargo test --test language_benchmark -- --ignored --nocapture`
picks them up automatically — no code changes needed.

This also feeds the Phase 0 engine-choice fork (Whisper vs. `AppleSpeechEngine`,
Section 6) — Yoruba was the language most likely to change that call. With
it parked, the fork is resolved for now: Whisper already won for
English (100% accuracy), so `AppleSpeechEngine` is not being built.

## Backend full-text search for History

**Deferred 2026-08-31, confirmed during the frontend-rewrite `/plan-eng-review`.**
The rebuilt History panel's search box only filters the already-loaded
page-0 results client-side (matching today's page-0-only loading) — once
someone has more history than fits one page, search silently misses older
entries. The UI is honest about this (placeholder text reads "Search recent
transcripts…", not a bare "Search transcripts…").

Real fix: a search command + likely an FTS5 virtual table or `LIKE`-based
query in `history/mod.rs`, plus actual pagination UI in the History panel
(the backend's `get_history_page` has always accepted a `page` param — the
frontend has just never exposed one). Not built now because it's a real,
separate feature (schema/index work), not something the current rewrite's
scope called for.

## LLM-based (Option B style) spoken-corrections/spoken-formatting

**Deferred 2026-08-31, confirmed during the frontend-rewrite `/plan-eng-review`.**
`engine/grammar.rs`'s new `apply_spoken_corrections`/`apply_spoken_formatting`
steps are rule-based pattern matching (see that file's module docs for the
documented ceiling: "period" as ordinary content vs. punctuation, "I meant"
as ordinary content vs. a real self-correction — neither is disambiguable
by pattern matching alone).

If this ceiling turns out to actually matter in practice: the existing
local-LLM pipeline (`engine/llm_cleanup.rs`, already built, toggleable,
off-by-default) could be extended to handle these two cases with real
language understanding instead. Same "wait for real dogfooding signal
before building" discipline this project already applied before building
Option B itself in the first place — not built speculatively now.

## DONE — Grammar cleanup Option B (local-LLM cleanup)

Built 2026-08-30 (`engine/llm_cleanup.rs`, `engine/pipeline.rs`), ahead of
the Phase 8 dogfooding signal the 2026-08-29 decision was waiting for — at
the user's explicit request, not because that signal arrived. See
`CLAUDE.md`'s Grammar cleanup entry for the full story (including a real
GGML linker collision this feature had to work around by switching from
`llama-cpp-2` to `candle`). Toggle lives in the dashboard Settings panel,
off by default.
