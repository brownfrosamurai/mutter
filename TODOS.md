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

## Grammar cleanup Option B (local-LLM cleanup) — deferred, not parked

Still pending real signal from Phase 8 daily-use dogfooding on whether
rule-based cleanup (Option A, shipped) is actually insufficient. Unaffected
by the Yoruba decision above — revisit once there's dogfooding evidence
either way.
