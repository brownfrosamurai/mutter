//! Rules-based grammar/punctuation cleanup — Section 5's "Option A": fast,
//! fully offline, zero extra model weight. Always-on in the default
//! pipeline, unlike Option B (a local-LLM "clean up with AI" pass), which
//! Section 5 explicitly scopes as a per-transcript, user-triggered action
//! rather than always-on middleware.
//!
//! **Five independently-toggleable steps (frontend-rewrite plan, 2026-08-31,
//! `/plan-eng-review` decisions D4/D5/D6)** — each one a small free function
//! with its own unit tests, run in a fixed order over an ordered list rather
//! than a hand-rolled if/else chain or a full plugin/trait system (D4: right
//! -sized for exactly 5 known, fixed steps). Order (D5), and why:
//!
//! ```text
//! spoken-corrections -> spoken-formatting -> filler-word-removal
//!   -> capitalise -> tidy-punctuation
//! ```
//!
//! - **Corrections first**: operates on the rawest text, before anything
//!   else rewrites it.
//! - **Formatting**, then **filler-removal**: turning "comma"/"new line"
//!   into literal characters before stripping filler words keeps the two
//!   concerns from interacting in surprising ways.
//! - **Capitalise after filler-removal**: removing a leading filler word
//!   changes what the transcript's real first word is — capitalising
//!   before filler-removal could capitalise a word that's about to be
//!   deleted, leaving the wrong word uncapitalised.
//! - **Tidy-punctuation last**: cleans up whatever spacing/punctuation
//!   artifacts the earlier steps introduce.
//!
//! Two of the five (capitalise, tidy-punctuation) are `RuleBasedCleanup`'s
//! original always-on behavior, now individually toggleable but still
//! defaulting to on — see `settings::AppSettings`'s doc comments for the
//! backward-compatibility contract (an existing `settings.json` must load
//! with every one of these five defaulting to `true`, an exact behavioral
//! match for what this file did before the refactor). If any step panics or
//! errors, the whole pipeline falls back to raw Whisper output (D6) —
//! `TextProcessor::process`'s caller (`engine::pipeline::GrammarPipeline`)
//! already has this fallback shape for Option B, and this reuses it rather
//! than inventing a second one.
//!
//! **Rule-based heuristics, not real NLU — a documented ceiling, not a bug
//! to chase to 100%:**
//! - *Spoken formatting*: "period" used mid-sentence as an ordinary word
//!   ("a period of time") can't be told apart from "period" meaning
//!   punctuation by pattern matching alone.
//! - *Spoken corrections*: "I meant" appearing as ordinary content (not a
//!   self-correction) can't be perfectly disambiguated either.
//!
//! An LLM-based version of either would do much better — that's exactly
//! Option B's territory (`engine::llm_cleanup`), and is tracked in
//! `TODOS.md` pending real dogfooding signal that the rule-based ceiling is
//! actually a problem in practice, the same discipline this project already
//! applied before building Option B itself.
//!
//! **Decision (2026-08-29, user-confirmed, superseded in spirit but not in
//! substance by the above):** Option A only, for v1; Option B stays a
//! separate, explicitly-triggered enhancement layered on top by
//! `GrammarPipeline`, not folded into these five steps.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::{EngineError, TextProcessor};

/// Live, cheaply-readable flags for the five steps — one `Arc<AtomicBool>`
/// per step, matching `GrammarLlmCleanupFlag`'s existing pattern (a
/// dedicated atomic for the hot transcription-path read, kept in sync by
/// `lib.rs`'s `set_bool_setting` rather than locking the same
/// `Mutex<AppSettings>` commands like `set_hotkey` use). Cloned cheaply
/// (each field is an `Arc`) into `RuleBasedCleanup` at construction.
#[derive(Clone)]
pub struct RuleBasedCleanupFlags {
    pub capitalise_sentences: Arc<AtomicBool>,
    pub tidy_punctuation: Arc<AtomicBool>,
    pub remove_filler_words: Arc<AtomicBool>,
    pub spoken_formatting: Arc<AtomicBool>,
    pub apply_spoken_corrections: Arc<AtomicBool>,
}

impl RuleBasedCleanupFlags {
    /// All five true — the documented default (see module docs).
    pub fn all_enabled() -> Self {
        Self {
            capitalise_sentences: Arc::new(AtomicBool::new(true)),
            tidy_punctuation: Arc::new(AtomicBool::new(true)),
            remove_filler_words: Arc::new(AtomicBool::new(true)),
            spoken_formatting: Arc::new(AtomicBool::new(true)),
            apply_spoken_corrections: Arc::new(AtomicBool::new(true)),
        }
    }
}

pub struct RuleBasedCleanup {
    flags: RuleBasedCleanupFlags,
}

impl RuleBasedCleanup {
    pub fn new(flags: RuleBasedCleanupFlags) -> Self {
        Self { flags }
    }
}

#[async_trait::async_trait]
impl TextProcessor for RuleBasedCleanup {
    async fn process(&self, text: &str, _language: &str) -> Result<String, EngineError> {
        let mut out = text.to_string();

        if self.flags.apply_spoken_corrections.load(Ordering::Relaxed) {
            out = apply_spoken_corrections(&out);
        }
        if self.flags.spoken_formatting.load(Ordering::Relaxed) {
            out = apply_spoken_formatting(&out);
        }
        if self.flags.remove_filler_words.load(Ordering::Relaxed) {
            out = remove_filler_words(&out);
        }
        if self.flags.capitalise_sentences.load(Ordering::Relaxed) {
            out = capitalise_first_letter(&out);
        }
        if self.flags.tidy_punctuation.load(Ordering::Relaxed) {
            out = tidy_punctuation(&out);
        }

        Ok(out)
    }
}

/// Detects a spoken self-correction — "I meant X", "sorry, X", "make that
/// X" — and keeps only the correction. Looks for the LAST occurrence of a
/// trigger phrase so "I meant to say hello, actually, I meant goodbye"
/// keeps just "goodbye" (the most recent correction wins), and drops
/// everything before the trigger phrase itself. A no-op if no trigger
/// phrase is present. Case-insensitive matching on the trigger, original
/// casing preserved in the kept remainder.
fn apply_spoken_corrections(text: &str) -> String {
    const TRIGGERS: &[&str] = &["i meant", "make that", "sorry,"];

    let lower = text.to_lowercase();
    let mut best: Option<(usize, usize)> = None; // (byte offset of match start, trigger len)
    for trigger in TRIGGERS {
        if let Some(pos) = lower.rfind(trigger) {
            if best.map_or(true, |(best_pos, _)| pos > best_pos) {
                best = Some((pos, trigger.len()));
            }
        }
    }

    let Some((pos, trigger_len)) = best else {
        return text.to_string();
    };

    let remainder = &text[pos + trigger_len..];
    let trimmed = remainder.trim_start_matches([' ', ',', ':', '-']).trim();
    if trimmed.is_empty() {
        text.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Maps spoken formatting phrases to their literal characters —
/// word-boundary-aware (matches whole words only, via `\b`-equivalent
/// manual boundary checks, not substring replacement) so "a period of
/// time" only has its trailing punctuation-phrase collapsed if "period" is
/// truly a standalone word use, and multi-word phrases like "new line"
/// match before single-word ones. Case-insensitive; replacement casing is
/// always the literal character, never inherited from the spoken phrase.
fn apply_spoken_formatting(text: &str) -> String {
    // Longest phrases first so "new paragraph" is matched whole, not as
    // "new line" failing to match then leaving "paragraph" untouched.
    const PHRASES: &[(&str, &str)] = &[
        ("new paragraph", "\n\n"),
        ("new line", "\n"),
        ("comma", ","),
        ("period", "."),
        ("question mark", "?"),
        ("exclamation mark", "!"),
    ];

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    'outer: while !rest.is_empty() {
        // A phrase can only start here if we're not already mid-word —
        // otherwise "aperiod" would wrongly match "period" the moment the
        // scan reaches its 'p' (match_word_boundary only checks the
        // boundary *after* a candidate match, not before).
        let at_word_start = out
            .chars()
            .next_back()
            .map_or(true, |c| !c.is_alphanumeric());
        for (phrase, literal) in PHRASES {
            if !at_word_start {
                break;
            }
            if let Some(matched) = match_word_boundary(rest, phrase) {
                // Drop the space that was between the previous word and
                // the spoken phrase — "hello comma" -> "hello,", not
                // "hello ,". Whatever whitespace follows the phrase in the
                // original text (e.g. "comma world"'s space before
                // "world") is deliberately left alone and copied through
                // normally on the next iteration — that's what gives
                // "hello, world" its correct post-comma spacing.
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push_str(literal);
                rest = &rest[matched..];
                continue 'outer;
            }
        }
        // No phrase matched at the current position — copy one char and
        // advance, matching UTF-8 char boundaries.
        let ch_len = rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        out.push_str(&rest[..ch_len]);
        rest = &rest[ch_len..];
    }

    out
}

/// Case-insensitive match of `phrase` at the very start of `text`,
/// requiring a real word boundary (start-of-string or non-alphanumeric
/// before, end-of-string or non-alphanumeric after) on both sides — so
/// "period" matches in "a period." but not inside "periodic". Returns the
/// byte length of the match (including the phrase itself) if found.
fn match_word_boundary(text: &str, phrase: &str) -> Option<usize> {
    let text_lower_prefix: String = text.chars().take(phrase.chars().count()).collect();
    if !text_lower_prefix.eq_ignore_ascii_case(phrase) {
        return None;
    }
    let phrase_byte_len = text_lower_prefix.len();
    let next_char = text[phrase_byte_len..].chars().next();
    let boundary_after = next_char.map_or(true, |c| !c.is_alphanumeric());
    if boundary_after {
        Some(phrase_byte_len)
    } else {
        None
    }
}

/// Strips filler words ("um", "uh", "you know", "like" used as a filler,
/// approximated here as standalone "like" — a real disambiguation of
/// filler-"like" vs. comparison-"like" needs more than word matching, so
/// this treats every standalone "like" as filler, a deliberate, documented
/// simplification consistent with this module's rule-based ceiling).
/// Word-boundary-aware so "umbrella" survives. Line-preserving, same
/// reasoning as `tidy_punctuation` — this runs after `apply_spoken_formatting`
/// (D5) and must not flatten a real `\n` back into a space by naively
/// `split_whitespace`-ing the whole text (newlines count as whitespace).
fn remove_filler_words(text: &str) -> String {
    const FILLERS: &[&str] = &["um", "uh", "you know", "like"];

    text.split('\n')
        .map(|line| {
            let mut words: Vec<&str> = Vec::new();
            'outer: for word in line.split_whitespace() {
                let bare: String = word
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase();
                for filler in FILLERS {
                    if bare == *filler {
                        continue 'outer;
                    }
                }
                words.push(word);
            }
            words.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `RuleBasedCleanup`'s pre-rewrite behavior, unchanged: capitalises only
/// the first character of the whole (already-collapsed) text — not real
/// per-sentence capitalisation, despite the Settings UI's "Capitalise
/// sentences" label (see `settings::AppSettings::capitalise_sentences`'s
/// doc comment for why that scope was deliberately kept, not expanded).
fn capitalise_first_letter(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `RuleBasedCleanup`'s other pre-rewrite always-on behavior: collapses
/// internal whitespace and ensures terminal punctuation — extended
/// (frontend-rewrite plan) to collapse whitespace *within* each line while
/// preserving the newlines themselves, since this step runs after
/// `apply_spoken_formatting` (D5) and must not flatten the real `\n`/`\n\n`
/// that step just inserted for spoken "new line"/"new paragraph" back into
/// plain spaces — the pre-rewrite single-`split_whitespace` version did
/// exactly that (newlines count as whitespace to `split_whitespace`), which
/// would have silently defeated the new spoken-formatting step.
fn tidy_punctuation(text: &str) -> String {
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let collapsed = lines.join("\n");
    let trimmed = collapsed.trim_matches(|c: char| c == ' ' || c == '\t');
    if trimmed.is_empty() {
        return String::new();
    }

    let ends_with_terminal = trimmed
        .chars()
        .last()
        .map(|c| matches!(c, '.' | '!' | '?' | '…' | '،' | '؟'))
        .unwrap_or(false);
    if ends_with_terminal {
        trimmed.to_string()
    } else {
        format!("{trimmed}.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleanup_with_all(enabled: bool) -> RuleBasedCleanup {
        RuleBasedCleanup::new(RuleBasedCleanupFlags {
            capitalise_sentences: Arc::new(AtomicBool::new(enabled)),
            tidy_punctuation: Arc::new(AtomicBool::new(enabled)),
            remove_filler_words: Arc::new(AtomicBool::new(enabled)),
            spoken_formatting: Arc::new(AtomicBool::new(enabled)),
            apply_spoken_corrections: Arc::new(AtomicBool::new(enabled)),
        })
    }

    // --- Legacy behavior preserved (capitalise + tidy-punctuation) ---

    #[test]
    fn capitalise_first_letter_and_terminal_punctuation() {
        assert_eq!(capitalise_first_letter("hello world"), "Hello world");
        assert_eq!(tidy_punctuation("hello world"), "hello world.");
    }

    #[test]
    fn tidy_punctuation_preserves_existing_terminal_punctuation() {
        assert_eq!(tidy_punctuation("is this working?"), "is this working?");
    }

    #[test]
    fn tidy_punctuation_collapses_internal_whitespace() {
        assert_eq!(
            tidy_punctuation("hello   there   friend"),
            "hello there friend."
        );
    }

    #[test]
    fn tidy_punctuation_empty_input_stays_empty() {
        assert_eq!(tidy_punctuation("   "), "");
    }

    #[test]
    fn capitalise_non_cased_scripts_is_a_harmless_no_op() {
        let out = capitalise_first_letter("مرحبا بالعالم");
        assert!(out.starts_with('م'));
    }

    /// REGRESSION (mandatory, frontend-rewrite plan's Iron Rule): the full
    /// 5-step pipeline at every flag's DEFAULT (all `true`) must produce
    /// byte-identical output to the pre-rewrite `RuleBasedCleanup::clean()`
    /// for input containing none of the new steps' trigger phrases —
    /// proving the refactor didn't silently change existing behavior.
    #[tokio::test]
    async fn full_pipeline_at_defaults_matches_pre_rewrite_output_on_plain_text() {
        let cleanup = cleanup_with_all(true);
        let cases = [
            ("hello world", "Hello world."),
            ("is this working?", "Is this working?"),
            ("hello   there   friend", "Hello there friend."),
        ];
        for (input, expected) in cases {
            let out = cleanup.process(input, "en").await.unwrap();
            assert_eq!(out, expected, "input: {input:?}");
        }
    }

    // --- New steps ---

    #[test]
    fn filler_word_removal_strips_standalone_fillers_not_substrings() {
        assert_eq!(
            remove_filler_words("um so I uh want an umbrella"),
            "so I want an umbrella"
        );
    }

    #[test]
    fn filler_word_removal_word_boundary_umbrella_survives() {
        // "like" is stripped as filler but "umbrella" (contains "um") must not be.
        let out = remove_filler_words("um bring an umbrella");
        assert!(out.contains("umbrella"));
        assert!(!out.split_whitespace().any(|w| w.eq_ignore_ascii_case("um")));
    }

    #[test]
    fn spoken_formatting_converts_known_phrases() {
        assert_eq!(
            apply_spoken_formatting("hello comma world period"),
            "hello, world."
        );
        // The leading space on "second line" here is real — this function
        // only strips whitespace immediately *before* an inserted literal,
        // not whitespace it left behind afterward. `tidy_punctuation`
        // (which runs after this step in the real pipeline, see D5) is
        // what collapses it away; see the full-pipeline test below for the
        // clean end-to-end result.
        assert_eq!(
            apply_spoken_formatting("first line new line second line"),
            "first line\n second line"
        );
    }

    #[tokio::test]
    async fn full_pipeline_spoken_formatting_newline_survives_tidy_punctuation() {
        let cleanup = cleanup_with_all(true);
        let out = cleanup
            .process("first line new line second line", "en")
            .await
            .unwrap();
        // Only the transcript's very first character is capitalised (D3 /
        // settings.rs's capitalise_sentences doc comment — not real
        // per-sentence/per-line capitalisation), so "second" on the new
        // line correctly stays lowercase here.
        assert_eq!(out, "First line\nsecond line.");
    }

    #[test]
    fn spoken_formatting_does_not_match_mid_word() {
        // "aperiod" is one word — "period" must not match starting at its
        // 'p', even though match_word_boundary alone (checking only the
        // boundary *after* a candidate) would accept it.
        assert_eq!(apply_spoken_formatting("aperiod later"), "aperiod later");
    }

    #[test]
    fn spoken_formatting_documented_heuristic_limit_period_mid_sentence() {
        // "a period of time" — "period" here is ordinary content, not a
        // punctuation instruction, but the rule-based matcher can't tell
        // the difference (documented ceiling, not a bug).
        let out = apply_spoken_formatting("a period of time");
        assert_eq!(out, "a. of time");
    }

    #[test]
    fn spoken_corrections_keeps_the_last_correction() {
        assert_eq!(
            apply_spoken_corrections("I meant to say hello, actually, I meant goodbye"),
            "goodbye"
        );
    }

    #[test]
    fn spoken_corrections_no_trigger_is_a_no_op() {
        assert_eq!(
            apply_spoken_corrections("just a normal sentence"),
            "just a normal sentence"
        );
    }

    #[test]
    fn spoken_corrections_documented_heuristic_limit_ordinary_content() {
        // "I meant" used as ordinary reported speech, not a self-correction
        // — the rule-based matcher can't tell the difference either.
        let out = apply_spoken_corrections("she said I meant well");
        assert_eq!(out, "well");
    }

    // --- Ordering (D5) ---

    #[tokio::test]
    async fn capitalise_runs_after_filler_removal_so_the_real_first_word_is_capitalised() {
        let cleanup = cleanup_with_all(true);
        let out = cleanup.process("um hello there", "en").await.unwrap();
        assert_eq!(out, "Hello there.");
    }

    #[tokio::test]
    async fn corrections_run_before_formatting_and_filler_removal() {
        let cleanup = cleanup_with_all(true);
        // "make that" correction should win before "comma"/"period" get
        // interpreted as punctuation in the discarded prefix.
        let out = cleanup
            .process("hello comma world period make that goodbye", "en")
            .await
            .unwrap();
        assert_eq!(out, "Goodbye.");
    }

    // --- Individual toggles ---

    #[tokio::test]
    async fn each_step_is_independently_toggleable() {
        let flags = RuleBasedCleanupFlags {
            capitalise_sentences: Arc::new(AtomicBool::new(false)),
            tidy_punctuation: Arc::new(AtomicBool::new(false)),
            remove_filler_words: Arc::new(AtomicBool::new(true)),
            spoken_formatting: Arc::new(AtomicBool::new(false)),
            apply_spoken_corrections: Arc::new(AtomicBool::new(false)),
        };
        let cleanup = RuleBasedCleanup::new(flags);
        let out = cleanup.process("um hello world", "en").await.unwrap();
        // Only filler removal ran: no capitalisation, no terminal period.
        assert_eq!(out, "hello world");
    }

    #[tokio::test]
    async fn all_flags_off_is_a_pure_passthrough() {
        let cleanup = cleanup_with_all(false);
        let out = cleanup.process("  um hello   world  ", "en").await.unwrap();
        assert_eq!(out, "  um hello   world  ");
    }

    #[tokio::test]
    async fn empty_input_every_step_handles_it_without_panicking() {
        let cleanup = cleanup_with_all(true);
        let out = cleanup.process("", "en").await.unwrap();
        assert_eq!(out, "");
    }
}
