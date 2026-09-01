# Onboarding flow — plan

Inspired by 4 reference screenshots (a "Aura" transcription app's 4-step wizard:
Welcome → System Access → AI Core Engine → Ready), adapted to Mutter's real
feature set and scoped down via Step 0 (see GSTACK REVIEW REPORT at the bottom
for the scope-reduction record).

## Problem

Mutter has no first-run experience at all today. `lib.rs`'s `setup()` never
auto-shows any window at startup — pill and dashboard are both created
`visible: false` and only ever shown by user action (hotkey, tray click). A
brand-new user installs Mutter and sees nothing but a tray icon, with no
indication of what to do next, what permissions it needs, or what the hotkey
is.

## Scope (post Step 0 reduction)

**3 steps, not 4:** Welcome → Permissions → Ready. The reference's 4th step
(model picker with per-model download progress) doesn't map to Mutter, which
has exactly one engine/model (Whisper Small, English-only — a settled
Phase 0 decision) and no download-progress-reporting subsystem at all. Building
one for a screen the user sees once, when the pill already shows the real
first-load message today ("Warming up engine… Initial lazy-load latency (once
per session)"), was rejected as scope creep. See "NOT in scope" below.

**Accessibility/Screen Recording "Grant" buttons open System Settings; Microphone
gets a real native permission prompt.** `permissions.rs` is check-only today
(`refresh()`/preflight, no `AXIsProcessTrustedWithOptions(prompt: true)` or
`CGRequestScreenCaptureAccess()` calls exist, and `permissions_shim.m` only
has a status-check function, `mutter_mic_auth_status`, no active-request
function). Building `AXIsProcessTrustedWithOptions(prompt: true)`/
`CGRequestScreenCaptureAccess()` active-request shims was rejected as scope
creep for this pass — see "NOT in scope" below — **but mic is different**
(Outside Voice finding #2, resolved): `AVCaptureDevice.requestAccess(for:
.audio)` is macOS's standard, cheap, already-half-built (the shim already
imports `AVFoundation` and calls a sibling API on the same class) active-
request API, unlike Accessibility/Screen Recording's heavier
prompt-then-deep-link APIs — routing mic through System Settings when a
one-call native prompt is this close would be a strictly worse first-run
experience for zero savings. See "Backend changes" item 5 below for the new
shim function + command this adds.

## Architecture

A 4th window, `onboarding`, following the exact same pattern `recovery`
already establishes: its own HTML entry, its own React root, shown instead of
the normal pill/dashboard flow under one condition, closed permanently once
that condition is satisfied.

```
STARTUP FLOW (lib.rs setup(), after history::open() resolves)

  history::open() Ok
        │
        ▼
  AppSettings.onboarding_completed?
        │
   ┌────┴────┐
   │ false   │ true
   ▼         ▼
 show        (existing behavior, unchanged)
 "onboarding"  pill/dashboard stay hidden until
 window,       hotkey/tray-click, exactly as today
 THEN
 win.set_focus()
 (Outside Voice
 finding #1 —
 see below)
   │
   │  user clicks through 3 steps, or Skip
   ▼
 complete_onboarding command
   │
   ├─ AppSettings.onboarding_completed = true, saved to settings.json
   └─ onboarding window closes
```

```
ONBOARDING WINDOW — 3-step shell (mirrors recovery's window-per-flow pattern)

  Onboarding.tsx (shell: step index, progress bar, back/skip/continue nav)
        │
        ├─ step 0: Welcome.tsx        — static copy, no backend call
        │
        ├─ step 1: Permissions.tsx    — reuses commands.getPermissionStatus()
        │                                (already exists, already used by
        │                                Settings.tsx's Permissions section)
        │                                Mic "Grant" → commands.requestMicAccess()
        │                                (new — a real native permission PROMPT,
        │                                not a System Settings redirect; see
        │                                Outside Voice finding #2 below)
        │                                Accessibility/Screen Recording "Grant" →
        │                                commands.openPermissionSettings(kind)
        │                                (new, thin — opens one System Settings
        │                                pane per permission via `open`; macOS has
        │                                no active-request API for either, so
        │                                System Settings is the only real path)
        │                                Refetches on window `focus` (Architecture
        │                                review finding #1) — Grant sends the user
        │                                to System Settings and back (or, for mic,
        │                                the native prompt resolves in-process but
        │                                a refetch after it still confirms the
        │                                real post-prompt status rather than
        │                                trusting the promise's own return value)
        │                                so a stale one-shot fetch would keep
        │                                showing "not granted" after they actually
        │                                granted it
        │
        └─ step 2: Ready.tsx          — reuses commands.getSettings() for the
                                         real mic_hotkey/system_audio_hotkey
                                         (already exists)
                                         "Open Dashboard" → commands.completeOnboarding()
                                         then shows dashboard, closes onboarding
```

## Backend changes

1. **`settings.rs`**: `AppSettings` gains `onboarding_completed: bool`,
   `#[serde(default)]` (defaults `false` — existing `settings.json` files on
   disk get onboarding on next launch, which is correct: an existing user who
   upgrades still hasn't seen it. New installs also default `false`, same
   field, same meaning, no separate "is this a fresh install" signal needed).

2. **`tauri.conf.json`**: new `onboarding` window entry, closely mirroring
   `recovery`'s shape (`transparent: true`, `windowEffects: hudWindow`,
   `decorations: true` — native traffic lights, matching the reference
   screenshots' native chrome — `resizable: false`, sized to fit the 3-step
   content at Mutter's existing type scale). `visible: false` at declaration
   time like every other window; shown conditionally in `setup()`, not via
   the config's own default.

3. **`lib.rs`**:
   - `setup()`: after `history::open()` resolves (success or recovery-mode
     branch, unchanged), check `app_settings.onboarding_completed`. If
     `false` and not in recovery mode, show the `onboarding` window **and
     call `win.set_focus()` right after `.show()`**, mirroring `recovery`'s
     own exact pattern (`lib.rs:665-666`) — Outside Voice finding #1
     (resolved): the app runs `ActivationPolicy::Accessory` (no Dock icon),
     so a newly-shown window does not auto-foreground on its own; without
     an explicit `set_focus()` call a fresh install's very first window
     could show up behind whatever the user was already looking at, the
     worst possible first impression for a first-run flow specifically.
     Insertion point relative to existing branches (Outside Voice minor
     finding, folded in directly, no AskUserQuestion needed): this check
     sits in the `history::open()` success branch, after the existing
     recovery-mode early-return and before the existing tray-menu-setup /
     `CloseRequested` override wiring for the dashboard window — those are
     unconditional regardless of onboarding state, so ordering relative to
     them doesn't matter, but must come after the recovery-mode branch so a
     corrupt-DB fresh install shows recovery, never onboarding (see Failure
     modes table below).
     (pill/dashboard themselves are unaffected — they just don't get shown
     yet either, same as today, until onboarding finishes or the user
     switches away).
   - New command `complete_onboarding()`: sets `onboarding_completed = true`
     on the shared `Mutex<AppSettings>`, persists via the existing
     `AppSettings::save()`, closes the onboarding window, shows the
     dashboard. Same `Mutex<AppSettings>` + `.save()` pattern every other
     settings-writing command already uses (`set_bool_setting`,
     `set_hotkey`) — no new persistence mechanism.
   - New command `open_permission_settings(kind: PermissionKind)`: a `String`-free
     enum (matching `SettingField`'s own established pattern — D3 from the
     frontend-rewrite plan explicitly chose enums over stringly-typed
     command args) with 3 variants (`Microphone`, `Accessibility`,
     `ScreenRecording`), each mapping to the matching
     `x-apple.systempreferences:` URL, opened via `tauri-plugin-opener`
     (already a Tauri-maintained plugin; check before adding — see Code
     Quality review below) or a direct `std::process::Command::new("open")`
     call (matching how this app already shells out to `open`-equivalent
     native calls elsewhere, e.g. nowhere yet — this would be the first,
     which the Code Quality review below flags as worth deciding
     explicitly).

4. **`capabilities/onboarding.json`**: new capability file, `"windows": ["onboarding"]`,
   granting only what the 3 steps need — no `core:window:*` beyond what's
   already implicit (this window has native decorations, so no custom
   titlebar drag-region permission needed, unlike dashboard/pill).

5. **`open_permission_settings`** — **Accessibility and Screen Recording
   only now** (mic moved to its own native-prompt command, item 7 below) —
   shells out via `std::process::Command::new("open")` directly (Code
   Quality finding #4) — no new dependency. This is a macOS-only app by hard
   constraint; `tauri-plugin-opener`'s cross-platform URL-opening value buys
   nothing here, and this is a single fixed, trusted URL per permission, not
   user-controlled input. Returns `Result<(), String>` (Test review
   finding #5) — same shape as `copy_history_text` — so a shell-out failure
   surfaces as a real inline error in Permissions.tsx instead of a silent
   no-op. `PermissionKind` keeps all 3 variants (`Microphone` stays a valid
   deep-link target too, since a user who denies the native mic prompt can
   only recover through System Settings — see the failure-mode note in item
   7 below), the command itself is just no longer *mic's own* Grant path.

6. **`complete_onboarding`** is awaited by the frontend, with the "Open
   Dashboard" button disabled while pending (Test review finding #6) — a
   `settings.json` save failure (disk full, permissions) surfaces instead of
   silently leaving `onboarding_completed=false`, which would otherwise only
   be discoverable as "onboarding weirdly reappears next launch."

7. **New command `request_mic_access()`** and a new native function,
   `mutter_request_mic_access`, added to the existing
   `native/permissions_shim.m` (Outside Voice finding #2, resolved) —
   `permissions_shim.m` today only has `mutter_mic_auth_status` (a
   status-*check*, `AVCaptureDevice authorizationStatusForMediaType:`); this
   adds the sibling active-*request* call,
   `[AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio
   completionHandler:]`. The completion handler is async and fires on an
   arbitrary internal AVFoundation queue; the shim function bridges it to a
   synchronous `BOOL` return via a `dispatch_semaphore_t` (signal from the
   completion handler, wait on the calling thread) — the same "block a
   background thread until a callback resolves" shape already established
   in this codebase for FFI boundaries (`std::panic::catch_unwind`-wrapped
   native calls elsewhere per CLAUDE.md's hard constraints). The Rust command
   wraps the blocking call in `tauri::async_runtime::spawn_blocking` (never
   called from the main/UI thread — Apple's own docs note
   `requestAccess:completionHandler:` can be slow on first call) and returns
   `Result<bool, String>` (granted or not, or a shim-level error) —
   Permissions.tsx refetches `getPermissionStatus()` afterward regardless of
   the returned bool (per the `focus`-refetch note above) so the displayed
   status always reflects the real OS-level grant, not just the promise's
   own resolution value. **Only called once, on first click of the mic
   Grant button** — macOS shows its native system prompt exactly once per
   app install; a user who denies it must go to System Settings for any
   subsequent attempt (standard macOS behavior, not a Mutter limitation) —
   this is why `open_permission_settings(Microphone)` stays available too
   (item 5 above), as the fallback path once the one-shot native prompt has
   already been answered.

## Frontend changes

New `frontend/onboarding.html` + `frontend/src/windows/onboarding/{main.tsx,Onboarding.tsx,steps/{Welcome,Permissions,Ready}.tsx}`,
added as a 4th Vite multi-page entry in `vite.config.ts` (mechanical — same
pattern as the existing 3).

Visual language, reused directly from the existing design system (no new
tokens):
- `.glass-panel` for the window's own content card (native `hudWindow`
  vibrancy underneath, exactly like `recovery` and `pill` already do).
- `--text-lg`/`--text-base`/`--text-sm`/`--text-xs` type scale, unchanged.
- A new shared `PermissionRow` component (icon/title/description + a
  right-side slot for either a plain status string or a Grant button) —
  used by both `Settings.tsx` (refactored to use it, replacing its current
  inline read-only block — Outside Voice finding #3, resolved) and the new
  `Permissions.tsx` step (Code Quality finding #2). Takes a `kind` prop
  (`mic | accessibility | system_audio`) and internally routes its own
  Grant action — mic calls `commands.requestMicAccess()`, the other two
  call `commands.openPermissionSettings(kind)` — so both call sites (the
  onboarding step and Settings.tsx) get the correct per-permission behavior
  for free from one component, not two near-duplicate implementations.
  Refetches `getPermissionStatus()` on window `focus` (Architecture
  finding #1), so returning from System Settings — or finishing the native
  mic prompt — shows the real, current status. **Settings.tsx today only
  renders permission status text with no action at all** (confirmed by
  reading `Settings.tsx`'s current Permissions section, `Settings.tsx:237-
  246`); adding Grant buttons there closes a real dead end Outside Voice
  flagged: a user who Skips onboarding, or grants nothing during it, had no
  path back to grant permissions afterward short of finding System Settings
  themselves outside the app entirely.
- `--surface-active`/`--focus-ring`/`--danger` tokens for step-dot progress
  indicator, focus rings, and (if a permission is denied) a status color,
  matching how these tokens are already used everywhere else.
- Ready.tsx displays the two configured shortcuts read-only via a `toSymbols()`
  helper extracted out of `HotkeyCapture.tsx` into `frontend/src/lib/hotkey.ts`
  (Code Quality finding #3) — `HotkeyCapture` itself switches to importing it
  too, one implementation instead of two.

## NOT in scope

- **4th step (model picker + download progress)** — no real Mutter feature
  to represent; the pill's existing "loading" state already covers first-load
  latency communication. Deferred indefinitely, not a TODO (there's nothing
  to build until multi-model support itself becomes a real feature, which
  Phase 0's "Yoruba parked, v1 scope narrowed to English" decision already
  ruled out for v1).
- **Active permission-request native shims for Accessibility/Screen
  Recording** (`AXIsProcessTrustedWithOptions(prompt: true)`,
  `CGRequestScreenCaptureAccess()`) — real, buildable, but deferred behind
  "open System Settings" for this pass per the Step 0 decision. **Mic is no
  longer in this bucket** — `AVCaptureDevice.requestAccess(for: .audio)` is
  in scope for this plan (Outside Voice finding #2, "Backend changes"
  item 7) since it's a single cheap API call on infrastructure
  (`permissions_shim.m`) that already exists, unlike Accessibility/Screen
  Recording's heavier prompt-then-deep-link APIs. → TODOS.md candidate for
  the remaining two (see review report).
- **Re-showing onboarding from Settings** ("redo onboarding" / "show me that
  again") — not requested, no evidence of need. If wanted later, it's cheap
  (unset `onboarding_completed`, show the window) — not worth building
  speculatively now.
- **Onboarding window resizing/reflow** — fixed size, matching `recovery`'s
  own `resizable: false` precedent, since the content is fully static/known
  (no variable-length user data ever renders in this window, unlike History's
  transcripts).

## What already exists (reused, not rebuilt)

- `get_permission_status` command + its exact DTO shape — Settings.tsx
  already renders this; Permissions.tsx is a second consumer of the same
  data, not a new backend query.
- `get_settings` command — Ready.tsx reads `mic_hotkey`/`system_audio_hotkey`
  from the same settings object Settings.tsx already displays/edits.
- `AppSettings::save()` / the shared `Mutex<AppSettings>` persistence
  pattern — `complete_onboarding` uses it exactly as `set_bool_setting`/
  `set_hotkey` already do.
- The `recovery` window's entire shape (transparent + hudWindow vibrancy +
  native decorations + fixed size + closed-once-done lifecycle) — onboarding
  is architecturally a sibling of recovery, not a new pattern.
- `.glass-panel`, the full color/spacing/type token set — zero new design
  tokens needed.

## Failure modes

| Codepath | Realistic failure | Test? | Error handling? | User sees |
|---|---|---|---|---|
| `open_permission_settings` shell-out | `open` binary missing or blocked (rare, but real on a locked-down machine) | Yes (Test review #5) | `Result<(), String>`, surfaced inline | A real error message, not silence |
| `complete_onboarding`'s `AppSettings::save()` | Disk full, `settings.json` permissions issue | Yes (Test review #6) | Awaited, button disabled until resolved | Button stays in a pending/error state instead of a false "success" |
| `setup()`'s onboarding-vs-recovery branch ordering | Recovery mode AND `onboarding_completed=false` at the same time (fresh install whose history DB is somehow already corrupt — plausible if a user restores an old, incompatible `history.sqlite` before first launch) | Yes (Test review, branch-ordering test) | Recovery screen takes priority by construction (checked first in `setup()`) | Recovery screen, never onboarding — correct, but only if the branch order is actually tested, not just assumed |
| `request_mic_access()`'s native prompt | User denies the one-shot macOS mic prompt (or it was already answered on a previous install/reset, so this call is a no-op returning the existing denied/granted status without ever showing UI — standard macOS one-time-prompt behavior) | Yes (native shim path, exercised via the existing live-verification discipline this project uses for shim changes, not a `cargo test`-only claim) | `Result<bool, String>`, and Permissions.tsx always refetches real status after the call regardless of the returned bool | "Denied — enable in System Settings" (same `PERMISSION_LABEL` text `Settings.tsx` already shows today) plus the Grant button falls back to `open_permission_settings(Microphone)` on any subsequent click, since the native prompt won't reappear |

No critical gaps: all four identified failure modes now have both a planned test and explicit error handling (per the Test Review decisions above).

## NOT in scope, TODOS.md candidates

One item from "NOT in scope" is a genuine TODOS.md candidate — active permission-request native shims for Accessibility and Screen Recording only (`AXIsProcessTrustedWithOptions(prompt: true)`, `CGRequestScreenCaptureAccess()`), deferred behind "open System Settings" for this pass (mic's own active-request shim is now in scope for this plan, see Outside Voice finding #2). See AskUserQuestion below for the add/skip/build-now decision.

## Worktree parallelization strategy

Two largely independent lanes:

| Step | Modules touched | Depends on |
|------|-----------------|------------|
| Rust: settings field, `complete_onboarding`, `open_permission_settings`, `request_mic_access` + `mutter_request_mic_access` native shim, `setup()` branch + `set_focus()`, capabilities | `src-tauri/src/{settings,lib}.rs`, `src-tauri/native/permissions_shim.m`, `src-tauri/capabilities/` | — |
| Frontend: onboarding window, 3 steps, `PermissionRow` extraction (mic + accessibility + system_audio variants), `toSymbols` extraction, `Settings.tsx` Grant-buttons wiring | `frontend/src/windows/onboarding/`, `frontend/src/components/`, `frontend/src/lib/`, `frontend/src/windows/dashboard/panels/Settings.tsx` | Command *signatures* only (already fully specified in this plan) |

**Execution order:** launch both lanes in parallel worktrees. Frontend work can proceed immediately against the command signatures specified above (typed by hand until bindings regenerate) — it does not need to wait for the Rust implementation to land. Merge both, then run `cargo test --lib export_bindings -- --ignored` once to regenerate real bindings and reconcile any drift between the hand-typed stubs and the real generated types — **mandatory step, not optional**, since this plan adds a new command (`request_mic_access`) that the frontend lane's hand-typed stub must reconcile against.

**Conflict flag:** both lanes touch `Settings.tsx` (Rust lane: none directly; frontend lane: refactoring its inline permission rows into the shared `PermissionRow` component, now also wiring real Grant buttons per Outside Voice finding #3). Since only the frontend lane touches this file, no real cross-lane conflict — noted for completeness only.

## GSTACK REVIEW REPORT

### Outside Voice (Claude subagent)

CODEX_MODE was `not_installed` on this machine, so per the skill's fallback the outside voice ran as a fresh Claude subagent with no shared context with the primary review, dispatched to independently re-examine the plan against the real source tree.

3 substantive findings, all presented to and resolved by the user via individual AskUserQuestion calls, all now folded into the plan above:

1. **Missing `win.set_focus()` on the onboarding window.** The app runs `ActivationPolicy::Accessory` (no Dock icon) — a newly-shown window does not auto-foreground without an explicit call. Verified against `recovery`'s own existing pattern at `lib.rs:665-666`. Resolved: onboarding's `setup()` branch now calls `.set_focus()` right after `.show()`, matching `recovery` exactly.
2. **Mic permission miscalibration.** The first draft bundled Mic/Accessibility/Screen Recording into one "Grant opens System Settings" decision. Mic has a real, cheap, standard active-request API (`AVCaptureDevice.requestAccess(for: .audio)`) that `permissions_shim.m` is already halfway built for (it already imports `AVFoundation` and calls a sibling API on the same class); Accessibility/Screen Recording don't have an equivalently cheap path and correctly stay on System Settings. Resolved: mic gets a real native prompt via a new `request_mic_access` command + `mutter_request_mic_access` shim function; Accessibility/Screen Recording unchanged.
3. **Skip/no-grant dead end.** `Settings.tsx`'s Permissions section today only renders status text, no action (confirmed by reading `Settings.tsx:237-246`) — a user who Skips onboarding or grants nothing had no path back except finding System Settings themselves, outside the app. Resolved: the new shared `PermissionRow` component (built for the onboarding step regardless) also replaces `Settings.tsx`'s inline block, giving it real Grant buttons for free.

2 minor findings, folded in directly per the plan's own stated intent (no AskUserQuestion needed — non-blocking, unambiguous fixes):

4. `setup()`'s onboarding-check insertion point relative to the recovery-mode branch, dashboard `CloseRequested` override, and tray-menu setup was underspecified. Clarified in "Backend changes" item 3: must run after the recovery-mode early-return (so a corrupt-DB fresh install shows recovery, never onboarding), ordering relative to the unconditional tray/dashboard wiring doesn't matter.
5. The implementation checklist didn't mention regenerating bindings after adding `request_mic_access`. Folded into the Worktree parallelization strategy's execution-order note as a mandatory (not optional) step, since this plan is the first to add a genuinely new command on top of the existing three.

**CROSS-MODEL:** no tension to adjudicate — all 3 substantive findings were gaps the primary review missed, not disagreements with it. Finding #2's premise (that `permissions_shim.m` only has a status-check function today) was independently verified by reading the file directly before accepting the finding, not taken on faith.

### Implementation Tasks

- [ ] `settings.rs`: add `onboarding_completed: bool` field, `#[serde(default)]`
- [ ] `tauri.conf.json`: add `onboarding` window entry (transparent, `hudWindow` vibrancy, native decorations, fixed size, `visible: false`)
- [ ] `lib.rs` `setup()`: after `history::open()` success + non-recovery branch, check `onboarding_completed`; if false, show `onboarding` window and call `.set_focus()`
- [ ] `lib.rs`: new command `complete_onboarding()` — sets `onboarding_completed = true`, persists via `AppSettings::save()`, closes onboarding window, shows dashboard
- [ ] `lib.rs`: new command `open_permission_settings(kind: PermissionKind)` — Accessibility/Screen Recording (and mic as a post-native-prompt fallback), shells out via `std::process::Command::new("open")`, returns `Result<(), String>`
- [ ] `native/permissions_shim.m`: add `mutter_request_mic_access` — `AVCaptureDevice requestAccessForMediaType:completionHandler:` bridged to a synchronous `BOOL` via `dispatch_semaphore_t`
- [ ] `lib.rs`: new command `request_mic_access()` — wraps the shim call in `tauri::async_runtime::spawn_blocking`, returns `Result<bool, String>`
- [ ] `capabilities/onboarding.json`: new capability file scoped to `"windows": ["onboarding"]`
- [ ] `frontend/vite.config.ts`: add `onboarding` as a 4th multi-page entry
- [ ] `frontend/onboarding.html` + `src/windows/onboarding/{main.tsx,Onboarding.tsx}` — 3-step shell (progress dots, back/skip/continue nav)
- [ ] `src/windows/onboarding/steps/Welcome.tsx` — static copy
- [ ] `src/windows/onboarding/steps/Permissions.tsx` — mic Grant → `requestMicAccess()`; accessibility/screen-recording Grant → `openPermissionSettings(kind)`; refetch on window `focus`
- [ ] `src/windows/onboarding/steps/Ready.tsx` — reads `mic_hotkey`/`system_audio_hotkey` via `toSymbols()`, "Open Dashboard" → `completeOnboarding()` (awaited, button disabled while pending)
- [ ] `frontend/src/lib/hotkey.ts`: extract `toSymbols()` out of `HotkeyCapture.tsx`; `HotkeyCapture` imports it too
- [ ] `frontend/src/components/PermissionRow.tsx`: new shared component (`kind: mic | accessibility | system_audio`), routes Grant per-kind, used by both `Permissions.tsx` and `Settings.tsx`
- [ ] `Settings.tsx`: refactor its inline Permissions block to use `PermissionRow` (adds real Grant buttons — closes the skip dead-end)
- [ ] Tests: branch-ordering test (recovery-mode takes priority over onboarding), `complete_onboarding` save-failure handling, `request_mic_access` shim path (live-verified per this project's established discipline for native shim changes, not `cargo test`-only)
- [ ] `cargo test --lib export_bindings -- --ignored` — regenerate `bindings.ts` for the new `request_mic_access`/`open_permission_settings`/`complete_onboarding` commands and `PermissionKind`/`SettingField`-style enums
- [ ] `cargo build && cargo test && cargo fmt --check && cargo clippy --all-targets` clean
- [ ] `npm run build` (frontend) clean, zero TS errors
- [ ] Live-launch verification: fresh install (onboarding_completed unset) shows onboarding focused in front; Skip and full-completion paths both correctly set the flag and don't reappear next launch; mic Grant triggers the real macOS system prompt; Settings.tsx's new Grant buttons work post-onboarding

### Completion Summary

- **Step 0 (Scope Challenge):** scope reduced from the reference's 4 steps to 3 (model-picker step cut — no real Mutter feature to represent), "Grant" behavior reduced to System-Settings-redirect only in the first pass, later refined by Outside Voice finding #2 to give mic a real native prompt.
- **Architecture review:** 1 finding (window `focus` refetch after Grant/System-Settings round-trip) — resolved, folded into `PermissionRow`'s design.
- **Code Quality review:** 4 findings (shared `PermissionRow` component, `toSymbols()` extraction, `open`-shell-out vs. `tauri-plugin-opener` decision, `PermissionKind` enum over stringly-typed args) — all resolved.
- **Test review:** 2 findings (`open_permission_settings`/shell-out error surfacing, `complete_onboarding` awaited + disabled-button handling) — both resolved as CRITICAL-class per the failure-modes table.
- **Performance review:** 0 issues found — this window only reads already-existing, already-cheap commands (`get_permission_status`, `get_settings`); no new hot path.
- **Outside voice (Claude subagent, codex not installed):** 5 findings, 3 substantive/blocking (all resolved via AskUserQuestion: `.set_focus()`, mic-native-prompt, Settings.tsx Grant buttons), 2 minor (folded in directly: `setup()` branch-ordering clarification, mandatory bindings-regen step).
- **NOT in scope:** 4 items (model-picker step, Accessibility/Screen-Recording active-request shims, re-showing onboarding from Settings, window resizing) — 1 routed to TODOS.md (the 2 remaining active-request shims).
- **What already exists:** 5 reused commands/patterns identified (`get_permission_status`, `get_settings`, `AppSettings::save()`, the `recovery` window's whole shape, the full design-token set) — none unnecessarily rebuilt.
- **Failure modes:** 4 codepaths analyzed (shell-out failure, save failure, branch-ordering, mic-prompt-denied), 0 critical gaps remaining — all have both a planned test and explicit error handling.
- **Worktree parallelization:** 2 lanes (Rust / Frontend), disjoint files, one real dependency (frontend needs the Rust lane's command *signatures*, already fully specified above, not its implementation) — safe to run in parallel.
- **Lake Score:** 9/10 — one real architecture gap (window focus) and one real UX miscalibration (mic permission path) caught and corrected before implementation, both root-caused against actual source rather than assumed; the one point held back is that the native mic-prompt shim itself is new, untested-in-this-codebase surface (a `dispatch_semaphore_t` bridge for an async AVFoundation callback) that deserves real live verification, not just a clean compile, before this plan is called fully proven. **Implemented and live-tested 2026-08-31 — this held-back point was justified.** Everything else in this plan verified working live on the real installed app (onboarding shows focused on fresh install, all 3 steps navigate, Permissions/Settings both render real per-permission status, Accessibility/Screen-Recording Grant → System Settings works, mic's denied-state fallback to System Settings works, dashboard handoff works). The mic native-prompt path itself found and fixed two real bugs (missing `NSMicrophoneUsageDescription`, AVFoundation call needing the main thread) but still didn't produce a visible dialog under synthetic-click testing even after both fixes — see `TODOS.md`'s "Onboarding's mic native-prompt path" entry. Flagged for a human to verify directly, matching this project's established pattern for synthetic-input limits (T12, pill-drag).

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 0 | — | not run |
| Codex Review | `/codex review` | Independent 2nd opinion | 0 | — | not run (codex CLI not installed) |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 1 | CLEAR | 11 findings (Architecture ×1, Code Quality ×4, Test ×2, Outside Voice ×5), all resolved |
| Design Review | `/plan-design-review` | UI/UX gaps | 0 | — | not run |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | — | not run |

**CODEX:** not applicable — codex CLI not installed on this machine; outside voice ran as a Claude subagent instead (dispatched fresh, no shared context with the primary review).

**VERDICT:** ENG CLEARED — ready to implement.

NO UNRESOLVED DECISIONS
