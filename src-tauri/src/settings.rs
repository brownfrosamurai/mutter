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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            mic_hotkey: hotkey::MIC_DICTATION_SHORTCUT_DEFAULT.to_string(),
            system_audio_hotkey: hotkey::SYSTEM_AUDIO_SHORTCUT_DEFAULT.to_string(),
            grammar_llm_cleanup_enabled: false,
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
        let settings = AppSettings {
            mic_hotkey: "CmdOrCtrl+Shift+Space".to_string(),
            system_audio_hotkey: "CmdOrCtrl+Shift+M".to_string(),
            grammar_llm_cleanup_enabled: true,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mic_hotkey, settings.mic_hotkey);
        assert_eq!(parsed.system_audio_hotkey, settings.system_audio_hotkey);
        assert_eq!(
            parsed.grammar_llm_cleanup_enabled,
            settings.grammar_llm_cleanup_enabled
        );
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
}
