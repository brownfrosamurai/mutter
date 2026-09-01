# Settings reference

`AppSettings` (`src-tauri/src/settings.rs`) is the complete set of user-configurable preferences, persisted as pretty-printed JSON at `~/Library/Application Support/Mutter/settings.json`. Missing or corrupt settings fall back to `AppSettings::default()` rather than refusing to launch — this is preference data, not the kind of integrity contract the history database's backup-then-migrate path exists for.

## Fields

| Field | Type | Default | Effect |
|---|---|---|---|
| `mic_hotkey` | `String` | `"CmdOrCtrl+Shift+Space"` | Toggle shortcut for mic dictation |
| `system_audio_hotkey` | `String` | `"CmdOrCtrl+Shift+M"` | Toggle shortcut for system-audio capture |
| `grammar_llm_cleanup_enabled` | `bool` | `false` | Turns on Option B (local-LLM cleanup) on top of the always-on rule-based cleanup — see [`explanation-grammar-pipeline.md`](explanation-grammar-pipeline.md) |
| `paste_automatically` | `bool` | `true` | Off = copy the transcript to the clipboard only, never call `insert_at_cursor` |
| `restore_clipboard` | `bool` | `true` | Only affects the clipboard-*fallback* injection path (terminals) — restores the original clipboard contents after the synthetic paste |
| `capitalise_sentences` | `bool` | `true` | Grammar step: capitalises the transcript's first character (not real per-sentence capitalisation — see the field's own doc comment for why that scope is deliberate) |
| `tidy_punctuation` | `bool` | `true` | Grammar step: collapses internal whitespace, ensures terminal punctuation |
| `remove_filler_words` | `bool` | `true` | Grammar step: strips "um", "uh", "you know", standalone "like" |
| `spoken_formatting` | `bool` | `true` | Grammar step: turns spoken phrases ("comma", "new line") into literal characters |
| `apply_spoken_corrections` | `bool` | `true` | Grammar step: detects "I meant X" / "make that X" and keeps only the correction |
| `onboarding_completed` | `bool` | `false` | Gates the first-run onboarding window — never reset by the app itself |

All fields except `mic_hotkey`/`system_audio_hotkey`/`grammar_llm_cleanup_enabled`/`onboarding_completed` are `#[serde(default = "true_default")]`, not plain `#[serde(default)]` — an existing `settings.json` written before a field existed must load with that field `true`, matching the pre-toggle always-on behavior exactly. This is a mandatory regression contract (`settings.rs`'s own tests enforce it), not an arbitrary choice — flipping the default to `false` would silently change output for every existing user on upgrade.

## The five rule-based grammar steps, run in a fixed order

```
apply_spoken_corrections -> apply_spoken_formatting -> remove_filler_words
  -> capitalise_first_letter -> tidy_punctuation
```

Each is independently toggleable via `capitalise_sentences`/`tidy_punctuation`/`remove_filler_words`/`spoken_formatting`/`apply_spoken_corrections`, but the *order* they run in when multiple are on is fixed and load-bearing — see `engine/grammar.rs`'s module doc for exactly why this order (corrections before formatting so punctuation words in a discarded prefix don't get misread as literal punctuation; capitalise after filler-removal so the real first word gets capitalised; tidy-punctuation last to clean up whatever the earlier steps left behind).

## `SettingField` — the generic toggle command's selector

```rust
pub enum SettingField {
    PasteAutomatically,
    RestoreClipboard,
    CapitaliseSentences,
    TidyPunctuation,
    RemoveFillerWords,
    SpokenFormatting,
    ApplySpokenCorrections,
}
```

One `set_bool_setting(field: SettingField, enabled: bool)` command backs all seven of these fields (`grammar_llm_cleanup_enabled` keeps its own separate `set_grammar_llm_cleanup_enabled` command, predating this enum) — a deliberate DRY choice over seven near-identical commands, made during the frontend rewrite's `/plan-eng-review` (decision D3). `SettingField` is a real Rust enum, not a stringly-typed key, so tauri-specta generates a fully-typed TypeScript union — the frontend can never pass a typo'd field name. See [How to add a new Settings toggle](howto-add-a-settings-toggle.md) for adding an eighth.

## Live flags vs. persisted settings

Every boolean toggle exists in two places at runtime, kept in sync by the command that sets it:

1. **`Mutex<AppSettings>`** — the source of truth, saved to `settings.json` on every change.
2. **A live `Arc<AtomicBool>`** (or, for the five grammar steps, `RuleBasedCleanupFlags` — five `Arc<AtomicBool>`s) — what the actual hot transcription path (`RuleBasedCleanup::process`, `segment_worker`) reads on every call, without locking the same mutex the Settings-panel commands use.

Flipping a toggle in the dashboard takes effect on the very next transcript, no restart needed — `set_bool_setting`/`set_grammar_llm_cleanup_enabled` update both the mutex and the atomic in the same call.

## Related

- [How to add a new Settings toggle](howto-add-a-settings-toggle.md)
- [`explanation-grammar-pipeline.md`](explanation-grammar-pipeline.md) — Option A vs. Option B, and why cleanup is always-on
- [`reference-commands.md`](reference-commands.md) — the IPC commands that read/write this
