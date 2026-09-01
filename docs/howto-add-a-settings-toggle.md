# How to add a new Settings toggle

Add a new boolean preference, wired end-to-end: persisted in `settings.json`, live-readable on the hot transcription path with no restart, and exposed as a typed toggle in the dashboard's Settings panel.

## Prerequisites

- A working `cargo build` in `src-tauri/` (see [Getting started](tutorial-getting-started.md) if you haven't set this up yet)
- Know which behavior the toggle should actually gate — this guide wires the toggle itself, not the behavior it controls

## Steps

1. **Add the field to `AppSettings`** (`src-tauri/src/settings.rs`):

   ```rust
   #[serde(default = "true_default")] // or plain #[serde(default)] if the new behavior should be off by default
   pub my_new_toggle: bool,
   ```

   Also add it to `AppSettings::default()`'s struct literal.

   Use `#[serde(default = "true_default")]` only if the new field represents behavior that's *already* the app's real, unconditional behavior today (making it toggleable, defaulting on) — an existing user's `settings.json` predates this field and must see zero change in output on upgrade. Use plain `#[serde(default)]` (defaults to `false`) for a genuinely new, opt-in behavior.

2. **Add the variant to `SettingField`** (same file):

   ```rust
   pub enum SettingField {
       // ...existing variants...
       MyNewToggle,
   }
   ```

3. **Add the match arm to `AppSettings::field_mut`**:

   ```rust
   SettingField::MyNewToggle => &mut self.my_new_toggle,
   ```

4. **If the hot path needs to read this live** (most toggles do — anything read during transcription, not just at settings-panel render time), add a live `Arc<AtomicBool>` twin. In `lib.rs`'s `LiveToggleFlags`:

   ```rust
   struct LiveToggleFlags {
       // ...
       my_new_toggle: Arc<AtomicBool>,
   }
   ```

   Wire it in `LiveToggleFlags::from_settings` and add the match arm in `LiveToggleFlags::atomic_for`. Read it from wherever the actual behavior lives (a new atomic on a struct in `engine/`, mirroring `RuleBasedCleanupFlags`'s pattern if this is a grammar step).

5. **Regenerate the frontend bindings**:

   ```bash
   cd src-tauri && cargo test --lib export_bindings -- --ignored
   ```

   See [How to add or change a Tauri command](howto-add-a-tauri-command.md) if this is unfamiliar — `set_bool_setting`'s signature includes `SettingField`, so tauri-specta regenerates its TypeScript union automatically; you don't add a new command for a new toggle.

6. **Add the UI row.** In `frontend/src/windows/dashboard/panels/Settings.tsx`, add an entry to whichever toggle list this belongs to (or a new `<SettingRow>` if it's its own section):

   ```tsx
   <SettingRow
     title="My new toggle"
     description="What this actually does, in one sentence."
     checked={settings.data?.my_new_toggle ?? true}
     onCheckedChange={(checked) => void handleToggle("myNewToggle", checked)}
     disabled={!settings.data}
   />
   ```

7. **Write the regression test.** In `settings.rs`'s `#[cfg(test)]` module, extend `all_seven_new_toggles_default_to_enabled_when_missing_from_json` (rename it, it's no longer seven) if this toggle defaults to `true`, or add a parallel test if it defaults to `false`. This is the test that would have caught a silent behavior change on upgrade — don't skip it.

## Verification

```bash
cd src-tauri && cargo test && cargo clippy --all-targets && cargo fmt --check
cd ../frontend && npm run build
```

Then run the app (`cargo tauri dev` or a full build — see [Getting started](tutorial-getting-started.md)) and confirm the new row appears in Settings, toggling it persists across an app restart (check `~/Library/Application Support/Mutter/settings.json`), and the behavior it gates actually changes on the very next transcript without restarting.

## Troubleshooting

- **TypeScript error about a missing field on `AppSettings`** — you forgot step 5 (regenerate bindings); `bindings.ts` is generated, not hand-maintained.
- **Toggle flips in the UI but behavior doesn't change until restart** — you skipped step 4; the code path you're gating is still reading `Mutex<AppSettings>` (or nothing at all) instead of the live atomic.
- **`cargo test` fails on the backward-compat test** — you used plain `#[serde(default)]` for a field that's supposed to preserve existing always-on behavior, or vice versa. Re-check which of the two cases in step 1 actually applies.

## Related

- [`reference-settings.md`](reference-settings.md) — the full current schema
- [`reference-commands.md`](reference-commands.md) — `setBoolSetting`'s exact IPC shape
