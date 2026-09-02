# Mutter — open TODOs

Human-gated items an agent cannot close alone. See `CLAUDE.md`'s "Current
state" section and `docs/mutter-project-plan.md` for full context on each.

## Onboarding Ready screen: missing ARIA live region for permission status

**Surfaced 2026-09-01 by `/plan-design-review`.** `Ready.tsx`'s permission status rows update asynchronously as each of the 3 requests (mic → accessibility → screen_recording) resolves, but nothing announces those changes to a screen reader — a user relying on one gets no indication a row moved from `queued` to `granted`/`denied`. Fix: add `aria-live="polite"` to the status-row container so assistive tech picks up the changes as they happen. One-line addition once the status rows exist; not built now since the design review already resolved 6 more consequential issues (copy/hierarchy, missing interaction states, row-style mismatch, quit affordance, drag-region placement, the re-entry guard) and this seemed like the right place to stop expanding scope.

## Onboarding/Recovery chromeless migration compounds with the still-unverified mic prompt path

**Surfaced 2026-09-01 by `/plan-design-review`'s outside voice; one specific compounding risk found and fixed 2026-09-01 by `/ship`'s adversarial review.** This session bundled two changes together (user-directed, already accepted as combined scope): the entitlements fix for the mic native-prompt path (see "Onboarding's mic native-prompt path" below — still needs a human to verify the dialog actually appears), and migrating Onboarding/Recovery off native `decorations: true` onto the same chromeless `apply_glass_shell` mechanism Dashboard/Pill already use. The specific concrete risk this entry named — "an OS alert failing to attach visibly to a decorationless window" — turned out to have a real, adjacent root cause: `lib.rs`'s `setup()` was calling `.show()` on the onboarding/recovery windows *before* `apply_glass_shell()` ran on them (the reverse of dashboard/pill's shell-then-show ordering, the only ordering this native `NSGlassEffectView` reparenting mechanism has actually been live-verified against) — found and fixed by moving each window's shell call to immediately before its own `.show()`. This closes the "chromeless migration has its own bug" half of the compounding-risk concern; the mic-prompt entitlements fix itself is still unverified by a human — see that entry below.

## Ready.tsx's resolved phase may overflow the fixed 480×480 onboarding window

**Surfaced 2026-09-01 by `/ship`'s pre-landing design specialist review.** The resolved ("You're all set") phase adds substantial content on top of the in-flight phase — an h1, a paragraph, two hotkey rows, denser permission rows, and a new Quit button — inside a fixed, non-resizable 480×480 window (`tauri.conf.json`) whose content is vertically centered with no scroll container anywhere in the chain (`GlassPanel` has no built-in `overflow-y-auto`/`max-height`). The step area only has a `min-h-[180px]` floor, not a shared height budget between phases, so the panel could grow and clip against the window bounds on the phase transition. Not fixed now — moderate confidence (55/100), needs live visual confirmation at the real 480×480 size, and this doubles as the same human-verification pass the mic-prompt entry above already requires (whoever walks through onboarding to confirm the mic dialog should also watch for this). If it reproduces, cheapest fix is probably `overflow-y-auto` with a fixed max-height on the step content area.

## Onboarding's permission auto-fire and Settings' Grant button can race for the same permission kind

**Surfaced 2026-09-01 by `/ship`'s pre-landing adversarial review.** The dashboard window already exists (`visible:false`) during onboarding, and the tray's "Open Dashboard" menu item (`lib.rs`) is wired unconditionally — nothing gates it on whether onboarding is still showing or `Ready.tsx`'s permission sequence is in flight. A user could click the tray icon → Settings → "Grant" on Accessibility while `Ready.tsx`'s own auto-fire loop is mid-sequence for the same kind. `run_on_main_thread`'s `dispatch_sync` serializes the two native calls on the main thread (no crash/UB), but the user could see two overlapping/duplicate system alerts, or `Ready.tsx`'s local `rowStatus` state and Settings' TanStack Query `["permissions"]` cache drift out of sync with each other and with real OS state until their next refetch. The busy-guard added this same review pass (`onBusyChange`/`navDisabled` in `Onboarding.tsx`) only blocks the onboarding window's own Back/Continue buttons — it has no awareness of the tray's independent path into Settings. Not fixed now — this needs a product call (disable "Open Dashboard" while onboarding is incomplete? serialize `request_permission` calls in Rust with a mutex keyed by kind?), not a pure code fix, and the realistic window for triggering it (clicking the tray menu within the ~seconds the auto-fire sequence takes) is narrow.

## Tray Quit bypasses the onboarding permission-request busy-guard

**Surfaced 2026-09-01 by `/ship`'s adversarial review.** `Ready.tsx`'s own Quit button, and `Onboarding.tsx`'s Back/Continue, are disabled while `phase === "in-flight"` specifically because `commands.quitApp()` → `app.exit(0)` is synchronous and immediate, and could otherwise tear down the whole process while a `spawn_blocking` task is still mid-native-call inside `run_on_main_thread`'s `exec_sync` (the same class of risk the "Open Dashboard" race entry above already names for the tray's other menu item). But the tray menu's own "Quit" item (`lib.rs`, a Tauri predefined `quit()` menu entry) is a completely separate, always-enabled path with zero awareness of onboarding state — nothing in `request_permission`/`permissions.rs` tracks "a native permission call is currently in flight" anywhere the tray handler could see. A user can click the tray icon → Quit at any point during the mic→accessibility→screen_recording auto-fire sequence, reproducing the exact race the in-window guard was built to close, through an unguarded second door. Not fixed now — needs backend-side state tracking (e.g. an `AtomicBool` set around the native-call span, shared between `request_permission` and the tray's quit handler) and a product call on whether quit should ever be blocked, not a pure mechanical fix.

## Unverified: do Accessibility/Screen Recording's native calls block the whole app, not just the requesting window?

**Surfaced 2026-09-01 by `/ship`'s adversarial review.** Both `CGRequestScreenCaptureAccess` and `AXIsProcessTrustedWithOptions` are dispatched via `run_on_main_thread`'s `dispatch_sync`, which blocks the *calling* thread until the closure returns — but the closure itself runs on the app's one real main thread. If either native call blocks until the user actually answers the system alert (rather than returning immediately with the alert handled out-of-process), the entire app — every window, hotkey handling, the tray — would freeze for that duration, not just the onboarding window. The code's own comments already flag real uncertainty here ("often does NOT show an interactive dialog at all... confirmed via web search... not assumed"), but nothing pins down the actual blocking semantics specifically under `dispatch_sync`. Three of these calls fire back-to-back in `Ready.tsx`'s sequence, so a worst case is the whole app appearing hung for the combined duration of up to three system alerts. Needs a human to watch the real app (not just the alert) during a full onboarding run, paying attention to whether other windows/hotkeys become unresponsive, not just whether the dialogs appear — same human-verification pass the mic-prompt entry below already requires.

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

## Onboarding's mic native-prompt path — root cause found 2026-09-01, still needs a human to verify the fix

**Built 2026-08-31 (onboarding flow, `docs/designs/onboarding-flow-plan.md`), live-verification inconclusive at the time.** This entry's own closing line below ("possibly related to the self-signed, non-notarized dev build") turned out to be exactly right: a 2026-09-01 `/investigate` found `tauri.conf.json`'s `bundle.macOS.entitlements` was `null` with no `.entitlements` file anywhere in the repo, while Tauri defaults `hardenedRuntime` to `true` for any non-adhoc `signingIdentity` — so the actual signed release build had Hardened Runtime on with zero entitlements. Under Hardened Runtime, `AVCaptureDevice.requestAccessForMediaType:` for audio requires `com.apple.security.device.audio-input` or the request resolves to denied before ever reaching `tccd` — confirmed live via `codesign -d --entitlements` showing nothing, and a 45-minute `log show --predicate` query showing zero `tccd` lines mentioning the app's bundle id despite clicking Grant. **Fix applied 2026-09-01** (bundled into the onboarding auto-permission-request change): `src-tauri/entitlements.plist` declaring `com.apple.security.device.audio-input`, wired into `tauri.conf.json`. **Still not human-verified** — the fix explains the mechanism and should resolve it, but nobody has yet confirmed live that the mic dialog actually appears post-fix. Re-test by: fresh `cargo tauri build`, install, `tccutil reset Microphone com.femimeduna.mutter`, launch, and confirm the real system dialog shows.

Original findings, for the record — two real bugs were found and fixed while live-testing this on the actual installed app before the entitlements gap was discovered:

1. `Info.plist` had no `NSMicrophoneUsageDescription` key at all (now added, `src-tauri/Info.plist`, auto-merged by Tauri) — without it, macOS can't show a prompt UI and silently returns denied.
2. The AVFoundation call was originally made directly on the Rust `spawn_blocking` thread (never the main thread) — verified live that this alone also produces an instant silent denial, no prompt shown. Fixed by dispatching the actual call onto the main queue while keeping the blocking wait on the background thread.

After both fixes, repeated clicks on the onboarding/Settings "Grant" button for Microphone (via synthetic `CGEventPost` clicks, cursor-position-verified) still never produced a visible system dialog, and the app never even appeared in System Settings → Privacy & Security → Microphone's app list (which should happen the moment a request is made, granted or denied). `tccutil reset Microphone com.femimeduna.mutter` was used between attempts to rule out stale state. This is the same category of gap this project has hit before (T12's terminal-injection test, the pill-drag gesture) — synthetic input/native-permission-UI interaction doesn't always replicate faithfully under agent automation; a real human click is needed to confirm whether the dialog now appears with both fixes in place, or whether a third issue remains (possibly related to the self-signed, non-notarized dev build).

Both fallback paths (Accessibility/Screen Recording's "open System Settings" Grant, and mic's own denied-state fallback to System Settings) were confirmed working live — this TODO is specifically about the first-run *native prompt* path for mic.

## Onboarding Skip may need its own framing now that permissions are automatic

**Surfaced 2026-09-01 by an outside-voice `/plan-eng-review` challenge of the auto-permission-request onboarding change.** `Onboarding.tsx`'s Skip button (visible on any non-last step — just Welcome, now that Permissions.tsx is gone) calls `finish()` directly, closing onboarding without ever mounting `Ready.tsx` — so a user who clicks Skip on Welcome gets zero permission requests fired, same as before this change. That's unchanged, intentional behavior, not a bug. But the whole point of this change was "make permissions automatic instead of requiring clicks" — and Skip is a bigger opt-out than it used to be, since there's no longer a visible per-permission Grant-button screen for the user to understand what they're skipping. A Skip user's only path back is discovering the dashboard Settings panel's Grant buttons unassisted.

Not acted on now — no evidence yet that this is a real problem in practice, and "redo onboarding from Settings" is already explicitly out of scope per the original onboarding-flow-plan.md. Revisit if real usage shows people skip and then can't find permissions later (e.g. via support questions, or Phase 8 dogfooding). If it does need addressing, cheapest fix is probably a one-line hint in Settings.tsx's Permissions section pointing back at what onboarding would have asked for.

## Possible dialog overlap: Accessibility and Screen Recording requests don't block like Mic's does

**Surfaced 2026-09-01 by the same outside-voice challenge.** `PermissionGate<Mic>::request()` blocks (via a `dispatch_semaphore_t`) until the user actually answers the system dialog — that's why it needed the semaphore-bridge machinery in the first place. The new `PermissionGate<Accessibility>::request()` (`AXIsProcessTrustedWithOptions`) and `PermissionGate<SystemAudio>::request()` (`CGRequestScreenCaptureAccess`) are plain synchronous calls that trigger their alert as a side effect but return almost immediately, regardless of whether/when the user responds. So in `Ready.tsx`'s sequential `await` chain (mic → accessibility → screen_recording), only the mic step actually pauses for a real answer — the accessibility alert and screen recording's list-registration could fire back-to-back while the user's still looking at the accessibility alert.

Not acted on now — Screen Recording rarely shows any interactive dialog at all on current macOS (see this session's `/investigate`-adjacent finding on `CGRequestScreenCaptureAccess`'s real-world behavior), so the realistic overlap is likely rare. If live verification shows this is actually confusing, the fix is probably a small UI beat (e.g. ~500ms, or wait for window focus to return) between the accessibility and screen-recording calls specifically.

## DONE — Grammar cleanup Option B (local-LLM cleanup)

Built 2026-08-30 (`engine/llm_cleanup.rs`, `engine/pipeline.rs`), ahead of
the Phase 8 dogfooding signal the 2026-08-29 decision was waiting for — at
the user's explicit request, not because that signal arrived. See
`CLAUDE.md`'s Grammar cleanup entry for the full story (including a real
GGML linker collision this feature had to work around by switching from
`llama-cpp-2` to `candle`). Toggle lives in the dashboard Settings panel,
off by default.
