//! Rules-based grammar/punctuation cleanup — Section 5's "Option A": fast,
//! fully offline, zero extra model weight. Always-on in the default
//! pipeline, unlike Option B (a local-LLM "clean up with AI" pass), which
//! Section 5 explicitly scopes as a per-transcript, user-triggered action
//! rather than always-on middleware. Option B is NOT implemented here —
//! wiring a local LLM is a real scope/cost decision the plan says to
//! confirm with the user before Phase 3, not to commit silently by building
//! it anyway.
//!
//! Deliberately conservative: normalizes whitespace, capitalizes the first
//! letter, and ensures terminal punctuation. It does not attempt real
//! grammar or word-choice correction — that's exactly the ceiling Option A
//! accepts in exchange for speed/simplicity — and it never touches
//! Whisper's actual word choices, so precise technical vocabulary dictated
//! to an AI coding agent survives un-paraphrased (Section 5's eng-review
//! note on this exact risk).

use super::{EngineError, TextProcessor};

pub struct RuleBasedCleanup;

#[async_trait::async_trait]
impl TextProcessor for RuleBasedCleanup {
    async fn process(&self, text: &str, _language: &str) -> Result<String, EngineError> {
        Ok(clean(text))
    }
}

fn clean(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return collapsed;
    }

    let mut chars = collapsed.chars();
    let first = chars.next().expect("collapsed is non-empty");
    let mut result: String = first.to_uppercase().collect();
    result.push_str(chars.as_str());

    let ends_with_terminal = result
        .chars()
        .last()
        .map(|c| matches!(c, '.' | '!' | '?' | '…' | '،' | '؟'))
        .unwrap_or(false);
    if !ends_with_terminal {
        result.push('.');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalizes_first_letter_and_adds_terminal_punctuation() {
        assert_eq!(clean("hello world"), "Hello world.");
    }

    #[test]
    fn preserves_existing_terminal_punctuation() {
        assert_eq!(clean("is this working?"), "Is this working?");
    }

    #[test]
    fn collapses_internal_whitespace() {
        assert_eq!(clean("hello   there   friend"), "Hello there friend.");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(clean("   "), "");
    }

    #[test]
    fn non_cased_scripts_still_get_terminal_punctuation() {
        // Arabic has no letter case, so uppercasing the first "letter" is a
        // harmless no-op — terminal punctuation is still added.
        let out = clean("مرحبا بالعالم");
        assert!(out.ends_with('.'));
        assert!(out.starts_with('م'));
    }
}
