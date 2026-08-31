//! User-configurable settings, persisted as JSON at
//! `~/Library/Application Support/Mutter/settings.json`.
//!
//! Deliberately not SQLite (unlike `history/`) — this is a handful of
//! user preferences, not structured relational data with aggregates and
//! pagination needs. A plain JSON file is the right-sized tool here.
//!
//! Missing or corrupt settings fall back to defaults rather than refusing
//! to launch — this is user preference, not the kind of data-loss-risk
//! integrity contract `history.rs`'s backup-then-migrate exists for.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::hotkey;

fn true_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AppSettings {
    pub mic_hotkey: String,
    pub system_audio_hotkey: String,
    /// Section 5, Option B — off by default: it's an opt-in ~390MB download
    /// plus real per-transcript latency, not something to silently turn on
    /// for existing or new users. See `engine::pipeline::GrammarPipeline`
    /// for how this is actually consumed (checked live, not just at
    /// startup) and `engine::llm_cleanup` for the always-on-vs-per-
    /// transcript tradeoff this toggle represents.
    #[serde(default)]
    pub grammar_llm_cleanup_enabled: bool,

    // --- Output settings (frontend-rewrite plan, 2026-08-31) ---
    // All seven default to `true`: the four that already had real, always-on
    // behavior (paste_automatically, restore_clipboard, capitalise_sentences,
    // tidy_punctuation) default on to match *today's* actual behavior
    // exactly — an existing user's `settings.json` predates these fields
    // and must see zero change on upgrade. The three genuinely new toggles
    // (remove_filler_words, spoken_formatting, apply_spoken_corrections)
    // also default on, matching every toggle shown "on" in the reference
    // screenshots.
    /// Off = copy the transcript to the clipboard only, never call
    /// `injection::insert_at_cursor`. See `injection.rs`'s module docs for
    /// why "restore the clipboard" and "auto-paste" are related but
    /// separately toggleable concerns.
    #[serde(default = "true_default")]
    pub paste_automatically: bool,
    /// Only meaningfully affects the clipboard-*fallback* injection path
    /// (terminals, apps without a settable AX text value) — the primary
    /// Accessibility-API path never touches the clipboard at all, so this
    /// toggle has no effect there. See `injection.rs`.
    #[serde(default = "true_default")]
    pub restore_clipboard: bool,
    /// `engine::grammar`'s `capitalise_first_letter` step — despite the
    /// UI label ("Capitalise sentences"), this capitalises only the first
    /// character of the whole transcript, matching `RuleBasedCleanup`'s
    /// pre-rewrite behavior exactly (real per-sentence capitalisation was
    /// considered and explicitly scoped out — see the frontend-rewrite
    /// plan's `/plan-eng-review` notes).
    #[serde(default = "true_default")]
    pub capitalise_sentences: bool,
    /// `engine::grammar`'s `tidy_punctuation` step (whitespace collapse +
    /// terminal punctuation) — `RuleBasedCleanup`'s other pre-rewrite
    /// always-on behavior, now toggleable.
    #[serde(default = "true_default")]
    pub tidy_punctuation: bool,
    /// `engine::grammar`'s `remove_filler_words` step — strips "um", "uh",
    /// "you know" and similar. New in the frontend-rewrite plan.
    #[serde(default = "true_default")]
    pub remove_filler_words: bool,
    /// `engine::grammar`'s `apply_spoken_formatting` step — turns spoken
    /// phrases like "new line"/"comma"/"period" into the literal characters
    /// they name. New in the frontend-rewrite plan.
    #[serde(default = "true_default")]
    pub spoken_formatting: bool,
    /// `engine::grammar`'s `apply_spoken_corrections` step — detects
    /// spoken self-corrections ("I meant X", "make that X") and keeps only
    /// the correction. New in the frontend-rewrite plan; rule-based
    /// heuristic, documented ceiling in `engine/grammar.rs`.
    #[serde(default = "true_default")]
    pub apply_spoken_corrections: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            mic_hotkey: hotkey::MIC_DICTATION_SHORTCUT_DEFAULT.to_string(),
            system_audio_hotkey: hotkey::SYSTEM_AUDIO_SHORTCUT_DEFAULT.to_string(),
            grammar_llm_cleanup_enabled: false,
            paste_automatically: true,
            restore_clipboard: true,
            capitalise_sentences: true,
            tidy_punctuation: true,
            remove_filler_words: true,
            spoken_formatting: true,
            apply_spoken_corrections: true,
        }
    }
}

/// Selects which boolean field `set_bool_setting` (`lib.rs`) writes — one
/// small enum instead of seven near-identical dedicated commands (DRY),
/// while tauri-specta still generates a fully-typed TS union for it, so the
/// frontend can never pass a typo'd/stringly-typed field name (the reason a
/// fully generic `set_setting(key: String, ...)` was rejected during
/// `/plan-eng-review` — see the frontend-rewrite plan's D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SettingField {
    PasteAutomatically,
    RestoreClipboard,
    CapitaliseSentences,
    TidyPunctuation,
    RemoveFillerWords,
    SpokenFormatting,
    ApplySpokenCorrections,
}

impl AppSettings {
    /// Mutable accessor for `set_bool_setting` — one match arm per field
    /// instead of one whole command function per field.
    pub fn field_mut(&mut self, field: SettingField) -> &mut bool {
        match field {
            SettingField::PasteAutomatically => &mut self.paste_automatically,
            SettingField::RestoreClipboard => &mut self.restore_clipboard,
            SettingField::CapitaliseSentences => &mut self.capitalise_sentences,
            SettingField::TidyPunctuation => &mut self.tidy_punctuation,
            SettingField::RemoveFillerWords => &mut self.remove_filler_words,
            SettingField::SpokenFormatting => &mut self.spoken_formatting,
            SettingField::ApplySpokenCorrections => &mut self.apply_spoken_corrections,
        }
    }
}

impl AppSettings {
    fn path() -> std::io::Result<PathBuf> {
        Ok(crate::paths::app_support_dir()?.join("settings.json"))
    }

    pub fn load() -> Self {
        let Ok(path) = Self::path() else {
            return Self::default();
        };
        let Ok(contents) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&contents).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "settings.json failed to parse, using defaults");
            Self::default()
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path()?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_match_hotkey_module_defaults() {
        let defaults = AppSettings::default();
        assert_eq!(defaults.mic_hotkey, hotkey::MIC_DICTATION_SHORTCUT_DEFAULT);
        assert_eq!(
            defaults.system_audio_hotkey,
            hotkey::SYSTEM_AUDIO_SHORTCUT_DEFAULT
        );
    }

    #[test]
    fn round_trips_through_json() {
        let mut settings = AppSettings {
            mic_hotkey: "CmdOrCtrl+Shift+Space".to_string(),
            system_audio_hotkey: "CmdOrCtrl+Shift+M".to_string(),
            grammar_llm_cleanup_enabled: true,
            ..AppSettings::default()
        };
        settings.paste_automatically = false;
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mic_hotkey, settings.mic_hotkey);
        assert_eq!(parsed.system_audio_hotkey, settings.system_audio_hotkey);
        assert_eq!(
            parsed.grammar_llm_cleanup_enabled,
            settings.grammar_llm_cleanup_enabled
        );
        assert_eq!(parsed.paste_automatically, settings.paste_automatically);
    }

    #[test]
    fn grammar_llm_cleanup_defaults_to_disabled_when_missing_from_json() {
        // Backward compatibility: a settings.json written before this field
        // existed shouldn't suddenly opt an existing user into a ~390MB
        // download and per-transcript latency they never asked for.
        let json =
            r#"{"mic_hotkey":"CmdOrCtrl+Shift+Space","system_audio_hotkey":"CmdOrCtrl+Shift+M"}"#;
        let parsed: AppSettings = serde_json::from_str(json).unwrap();
        assert!(!parsed.grammar_llm_cleanup_enabled);
    }

    /// REGRESSION (mandatory, frontend-rewrite plan's Iron Rule): an
    /// existing user's `settings.json`, written before these seven fields
    /// existed, must load with every one of them defaulting to `true` —
    /// exactly matching what `RuleBasedCleanup`'s old always-on behavior
    /// and the always-auto-paste/always-restore-clipboard injection
    /// behavior already did, so upgrading never silently changes output.
    #[test]
    fn all_seven_new_toggles_default_to_enabled_when_missing_from_json() {
        let json =
            r#"{"mic_hotkey":"CmdOrCtrl+Shift+Space","system_audio_hotkey":"CmdOrCtrl+Shift+M"}"#;
        let parsed: AppSettings = serde_json::from_str(json).unwrap();
        assert!(parsed.paste_automatically);
        assert!(parsed.restore_clipboard);
        assert!(parsed.capitalise_sentences);
        assert!(parsed.tidy_punctuation);
        assert!(parsed.remove_filler_words);
        assert!(parsed.spoken_formatting);
        assert!(parsed.apply_spoken_corrections);
    }

    #[test]
    fn field_mut_targets_the_correct_field_for_every_variant() {
        let variants = [
            SettingField::PasteAutomatically,
            SettingField::RestoreClipboard,
            SettingField::CapitaliseSentences,
            SettingField::TidyPunctuation,
            SettingField::RemoveFillerWords,
            SettingField::SpokenFormatting,
            SettingField::ApplySpokenCorrections,
        ];
        for field in variants {
            let mut settings = AppSettings::default();
            assert!(*settings.field_mut(field), "{field:?} should default true");
            *settings.field_mut(field) = false;
            assert!(
                !*settings.field_mut(field),
                "{field:?} should have flipped to false"
            );
            // No other field moved.
            let mut expected = AppSettings::default();
            *expected.field_mut(field) = false;
            let json_settings = serde_json::to_string(&settings).unwrap();
            let json_expected = serde_json::to_string(&expected).unwrap();
            assert_eq!(json_settings, json_expected);
        }
    }
}
