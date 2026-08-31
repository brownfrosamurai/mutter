# Mutter — Design System

Source: `docs/mutter-idea-dump.md` ("Super minimal glassmorphic UI... should feel like it's an Apple product... a widget, not a huge desktop application") and `docs/mutter-project-plan.md` Section 4.

Two surfaces only. No design system beyond what these two surfaces need — do not add components speculatively.

**Frontend rewrite, 2026-08-31**: the tokens below now live as CSS custom properties in `frontend/src/styles/globals.css` (React + Tailwind, replacing the old per-window `dashboard.css`/`pill.css`/`recovery.css`) — same values, same meanings, one file instead of three. Tailwind's `tailwind.config.ts` resolves utility classes through these same variables rather than duplicating the values, so this file is still the one source of truth to update.

## Surfaces

1. **The Pill** — floating, always-on-top HUD. Visible only during an active recording/canceling cycle, plus a brief reappearance for "done". **Status, 2026-08-30 (user-directed): no processing-state visuals.** States: `loading` (first-ever model warm-up only — the one deliberate exception, see `session.rs`'s module docs "Pill has no processing-state visuals"), `listening` (mic icon + waveform + elapsed timer + pause/cancel controls, no status text), `canceling` (danger-colored icon + status), `done` (success-colored icon + status). There is no `transcribing` state anymore — the pill window simply hides the instant recording stops and reappears once transcription actually finishes, rather than showing a spinner for a gap that's usually under 2 seconds. Only one state's elements are visible at a time — see `pill.css`'s per-state rules.
2. **Dashboard/Settings window** — opened from the menu-bar icon. Metrics, history, hotkey config, language, engine selection, permissions, Quit. Draws its own titlebar (`decorations: false` in `tauri.conf.json`, `shadow: false` to match) — custom traffic-light close/minimize/zoom buttons integrated directly into `#panel-header` next to the panel title, not macOS's native ones, so both surfaces now draw their own chrome. The close button hides the window rather than destroying it (it's reused for the app's lifetime, reopened via the tray's "Open Dashboard"); `lib.rs` intercepts the native `CloseRequested` event the same way as a fallback for any other close vector (e.g. Cmd+W).

Both share the same visual language below. Neither is a "big app window" — see Layout.

## Color

Dark-leaning glassmorphism (frosted glass reads better on a translucent dark base than a light one, and matches the macOS menu-bar-adjacent aesthetic this app lives in).

**Status, 2026-08-30: the Pill and Dashboard now run genuinely different accent palettes, at the user's explicit direction — the Pill keeps its hue (violet mic/waveform, blue processing spinner); the Dashboard went fully monochrome (black/white/gray only), because "purple accent color and all other text colors not black" read as visual noise once the real frosted-glass background (below) made the panel itself read as a serious, quiet, native-feeling surface.** This is a real, deliberate divergence, not an inconsistency — each surface's palette is documented separately below instead of one shared table implying they still match.

### Shared (both surfaces)

| Token | Value | Use |
|---|---|---|
| `--glass-bg` | `rgba(20, 20, 20, 0.82)` | Base fill behind the blur, both surfaces — darker/more opaque than the first draft, matched against reference mockups |
| `--glass-border` | `rgba(255, 255, 255, 0.10)` | 1px hairline border on every glass panel |
| `--glass-highlight` | `rgba(255, 255, 255, 0.06)` | Subtle top-edge inner highlight (linear-gradient), gives the "glass" a light source |
| `--text-primary` | `rgba(255, 255, 255, 0.94)` | Primary text/icons |
| `--text-secondary` | `rgba(255, 255, 255, 0.48)` | Secondary text, timestamps, hints |

### Pill only

| Token | Value | Use |
|---|---|---|
| `--accent-violet` | `#8B7CF6` | Mic icon + listening waveform |
| `--danger` | `#FF453A` | Cancel state (icon + text), cancel countdown |
| `--success` | `#30D158` | Done/confirmation state |

### Dashboard only

| Token | Value | Use |
|---|---|---|
| `--surface-active` | `rgba(255, 255, 255, 0.18)` | A persistent "this is selected" background — the active sidebar nav icon. Replaces `--accent-violet` there as of 2026-08-30 |
| `--surface-filled` | `rgba(255, 255, 255, 0.55)` | The "filled" portion of a bar (activity chart, language breakdown) or an "on" toggle track — brightness carries the signal a hue used to |
| `--focus-ring` | `rgba(255, 255, 255, 0.5)` | Every `:focus-visible` outline on this surface. Replaces `--accent-violet` there as of 2026-08-30 |
| `--danger` | `#FF453A` | A denied permission or a failed save — brightness + weight (`color: var(--text-primary); font-weight: 600;`) carries most status signaling on this surface, `--danger`/`--success`/`--warning` are the rare exceptions still in real use |
| `--success` | `#30D158` | Rare positive-confirmation accents |
| `--warning` | `#FFBD2E` | Rare warning accents (e.g. the Recovery window's icon) |

**Traffic lights are real macOS red/yellow/green again — reversed a second time, 2026-08-31.** The frontend rewrite briefly went monochrome to match a reference screenshot's plain gray dots; the user then asked for real colors back — `#FF5F57`/`#FEBC2E`/`#28C840`, the literal macOS values, set inline in `frontend/src/components/TrafficLights.tsx` (not routed through `--danger`/`--success`/`--warning`, which stay reserved for the rare accents in the row above — these are fixed brand colors for a universally-recognized system convention, not semantic status colors that should move if the accent palette ever changes).

**The sidebar's Quit/power button uses `--danger`, 2026-08-31, user-directed.** A deliberate, if small, exception to "the dashboard is monochrome" — a destructive action gets a real color cue rather than relying on icon shape alone. The sidebar's decorative wave-mark icon is now always shown in the same highlighted state a selected nav icon gets (`--surface-active`), also user-directed — a permanent mark, not a selection state.

No custom color beyond these two tables without updating them here first.

## Typography

System font stack only — never bundle a custom font, this is what makes it "feel like an Apple product" for free:

```css
font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", sans-serif;
```

| Token | Size | Weight | Use |
|---|---|---|---|
| `--text-xs` | 10px | 500 | Timestamps, metadata in history rows |
| `--text-sm` | 12px | 400 | Body text, settings labels |
| `--text-base` | 13px | 500 | Pill status text, primary dashboard labels |
| `--text-lg` | 15px | 600 | The dashboard panel title ("Stats"/"History"/"Settings") only |

**Reduced a further step across the board, 2026-08-31, user-directed** (11/13/15/18 → 10/12/13/15) — a tighter, quieter scale reads more like a native macOS widget. Stat-tile numbers are their own raw value, also stepped down alongside this pass, `22px` → `18px` (not this token).

## Spacing

4px base unit. Use the scale, never an arbitrary value:

`--space-1: 4px` · `--space-2: 8px` · `--space-3: 12px` · `--space-4: 16px` · `--space-5: 24px` · `--space-6: 32px`

## Glass effect (the actual CSS)

```css
.glass-panel {
  background: var(--glass-bg);
  backdrop-filter: blur(28px) saturate(1.5);
  -webkit-backdrop-filter: blur(28px) saturate(1.5);
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-panel);
  box-shadow: inset 0 1px 0 var(--glass-highlight);
}
```

**Status, 2026-08-30 — the real fix this whole file was hedging on until now.** Both surfaces now set `windowEffects: { effects: ["hudWindow"], state: "active" }` on their `tauri.conf.json` window entries — real macOS `NSVisualEffectView` vibrancy, not just a transparent window plus CSS `backdrop-filter`. That combination alone was never enough: `backdrop-filter` in a plain transparent WKWebView window blurs content *within the page's own render tree*, not the actual desktop behind a non-vibrant `NSWindow` — verified live by an A/B screenshot pair (same window, same desktop, `windowEffects` on vs. off): without it, the panel is a flat, sharp-edged, blue-tinted see-through pane (you can read individual rocks and mountain ridges straight through it); with it, the background actually goes dark and diffuse the way the color table above always assumed. This is what "frosted," not just "glassy," actually requires — the CSS blur was never wrong, it just had nothing native underneath it to blur.

**The outer drop shadow that used to be here is gone, at the user's explicit direction (2026-08-30) — do not re-add it.** It was added earlier the same day reasoning that a glass panel needs a shadow to read as "floating, not pasted" — true when the panel had no real depth cue at all, but wrong once real vibrancy (above) started carrying that job itself, the way native macOS vibrant panels (Control Center, Notification Center) read as floating from the blur alone, without a separate drawn shadow competing with it. `.glass-panel`'s `box-shadow` is the inset highlight only now, on both surfaces.

**Fixed, 2026-08-30 — the dashboard's vibrancy layer is now genuinely rounded, via a native shim, not a CSS mitigation.** Native vibrancy fills the whole rectangular `NSWindow`, sharp corners included, while `#app`/`#sidebar` are rounded (`--radius-panel`) and inset from the window edge by `#shell`'s padding — so a thin rectangular band of vibrancy used to show outside the rounded content, sharpest at the corners. The obvious fix, `tauri.conf.json`'s `windowEffects.radius`, is unusable: it routes through `window-vibrancy`'s `NSVisualEffectViewTagged::setCornerRadius`, which that crate's own source comment admits is an undocumented, private AppKit selector ("might be private, but it works") — setting it renders the dashboard window **completely blank** (vibrancy only, zero WKWebView content), reproduced twice and confirmed via a clean A/B. Corroborated by an unresolved upstream issue, [tauri-apps/tauri#14165](https://github.com/tauri-apps/tauri/issues/14165) (opened Sept 2025, still "needs triage"), reporting a different symptom of the same feature — this is a genuinely fragile area of Tauri on macOS right now, not unique to this machine.

**The real fix:** a small Objective-C shim, `native/vibrancy_mask_shim.m`, that finds the `NSVisualEffectView` Tauri's declarative `windowEffects` config already created (by the same tag `window-vibrancy` itself uses, `91376254`) and reshapes its visible region via the public, documented `-[NSVisualEffectView setMaskImage:]` — never touching `window-vibrancy`'s broken private call at all. Same pattern this codebase already uses for ScreenCaptureKit and permissions (`native/system_audio_shim.{h,m}`, `native/permissions_shim.m`); `CLAUDE.md`'s "no Swift/AppKit" constraint already carves out exactly this exception.

**One real bug hit and fixed building the mask itself:** the first version used `+[NSImage imageWithSize:flipped:drawingHandler:]` to draw the rounded-rect mask — it silently produced an image with **no alpha channel at all** (confirmed via the image's own debug description: `Alpha=NO`), so `maskImage` masked nothing; the shape was drawn as an opaque rectangle, not a transparency mask, and vibrancy kept showing everywhere unclipped with zero visible error. Fixed by building the mask manually via a `CGBitmapContext` created with `kCGImageAlphaPremultipliedLast`, which guarantees a real alpha channel.

**Second pass, same day, user-directed ("the shell should be invisible, only the dashboard visible"):** the first version of this fix masked the vibrancy to one rounded rect matching the whole window's outer shape — closing the square-corner mismatch, but still showing a visible blurred/tinted band everywhere `#shell`'s padding and the sidebar's floating gap actually are (i.e. wherever there's no real content). The current version masks to the exact union of `#app`'s card and `#sidebar`'s pill instead — two independent rounded shapes — so the gap between them and the outer margin are fully transparent: real desktop, no blur, no tint, visible only where the dashboard's actual UI sits. `mutter_apply_vibrancy_mask_regions` (the shim function) rebuilds a full-window-sized alpha bitmap on every call rather than a small stretchable capInsets image — the earlier concentric-single-rect approach could get away with a nine-patch stretch since it was one uniform border, but two independently-placed shapes can't be represented that way.

The geometry itself is **not hardcoded from CSS constants** — `dashboard.js`'s `reportVibrancyLayout()` reads `#app` and `#sidebar`'s real `getBoundingClientRect()` (on load and on every `resize`, debounced via `requestAnimationFrame`) and sends it to Rust via a `set_vibrancy_layout` command, which calls `vibrancy::mask_to_shapes()`. This stays correct if the CSS layout ever changes without a matching Rust edit — the earlier concentric-rect version needed a "keep this in sync by hand" comment for exactly this reason, and this version doesn't.

`#shell`'s padding (`--space-1`, 4px) and `--radius-panel` (10px) — shrunk during the earlier CSS-only mitigation attempts, before either shim existed — were left as-is; `--radius-panel` is still what `lib.rs`'s `set_vibrancy_layout` command uses for `#app`'s corner radius (`DASHBOARD_APP_CARD_RADIUS`), and the padding still shapes where `#app` actually sits, which the live-reported geometry now follows automatically either way.

**Third pass, same day, user-directed ("fix the shell on the pill just like you fixed the dashboard").** The pill window has the identical underlying bug: `#pill`'s width is content-driven (wider in `listening` — waveform, timer, controls — narrower in `done`/`canceling` — icon + status only) and never fills the fixed 190×36 window, while `body` has no centering, so unmasked vibrancy used to fill the window's dead space up to its square corners. `vibrancy.rs`'s two-shape API was generalized (`mask_to_shapes(primary, secondary)` for the dashboard, plus a `mask_to_shape(shape)` convenience for a window with only one visible region) rather than writing a second native function — the shim's `mutter_apply_vibrancy_mask_regions` already accepted a `secondary` rect for exactly this reuse (a zero-size secondary rect is a harmless no-op fill). `pill.js`'s new `reportPillVibrancyLayout()` reports `#pill`'s own `getBoundingClientRect()` to a new `set_pill_vibrancy_layout` command — driven by a `ResizeObserver` on `#pill` itself, not a window `resize` event, since the pill window is fixed-size and non-resizable (`resizable: false`) and never fires one; only `#pill`'s own content-driven size changes.

**Verified two ways before trusting the visual result:** the shim's generated mask bitmap was dumped to a PNG and inspected directly — a pixel-perfect capsule at the exact reported geometry, confirming the masking math itself is correct. A visible soft "wisp" trailing past the pill's rounded end in the live screenshot was then confirmed via a raw pixel scanline to be a smooth color gradient back to bare desktop, not a second hard-edged shape — i.e. the native vibrancy's own internal blur radius (not CSS `backdrop-filter`, which is clipped to `#pill`'s own box and can't paint past it) bleeding a soft ~50px falloff past the mask's hard alpha edge before reaching full transparency. That blur radius is intrinsic to `NSVisualEffectView`'s chosen material and isn't exposed by any public API to tune down — the same class of edge softness the dashboard's masked shapes had before the fix below.

**Fourth pass, same day, user-directed ("remove the shell which still shows on the pill... I want the design to be flawless") — the wisp is gone now, not just narrowed.** Masking couldn't fully close this: any mask still needs *somewhere* transparent to draw a hard alpha edge against, and native vibrancy's own blur bleeds a fixed distance past whatever edge that is, regardless of how tightly the mask itself is drawn. The dashboard's masked shapes are large enough that this bleed is proportionally invisible; the pill's fixed 190×36 window left up to ~57px of genuinely dead space around a state-driven ~133px-wide capsule, large enough (relative to the whole window) for the bleed to read as a visible wisp rather than a rounding error.

The real fix works a level up: stop leaving dead space to bleed into at all. `session.rs` gained `resize_pill_to_content()`, which sets the pill *window* itself (not just the vibrancy mask) to exactly `#pill`'s current content width, then repositions it (still bottom-center above the Dock — `resizable: false` in `tauri.conf.json` only gates the user's own drag, not programmatic resize from here). `lib.rs`'s `set_pill_vibrancy_layout` command calls this before masking, and — since a snug-fit window guarantees `#pill` (`body`'s only child, no margin) renders at exactly `(0, 0)` — builds the mask from that known post-resize geometry directly rather than the rect the frontend reported against the window's *previous*, wider size, which would be stale by the time the resize completes. With zero dead space left in the window, there's nowhere for the blur to bleed into that's actually visible — confirmed live at both a wide state (`listening`, waveform + timer + controls) and a narrow one (`done`, icon + status only): both render with genuinely crisp, symmetric rounded edges, no wisp at either width, and the pill stays correctly centered above the Dock at each size.

## Toggle switch

The one control besides buttons/inputs/traffic-lights this system needs — added 2026-08-30 for the dashboard's first on/off setting (grammar cleanup, Section 5 Option B). A real `<input type="checkbox">`, visually hidden (clipped, not `display:none`, so it stays keyboard-focusable and in the accessibility tree) with a sibling-selector-driven track + thumb standing in for it — the standard accessible-toggle pattern, not a bespoke widget.

| Property | Value |
|---|---|
| Track | 36×20px, `--radius-pill`, `1px solid --glass-border`, `rgba(255,255,255,0.12)` off / `--surface-filled` on |
| Thumb | 14px circle, slides via `transform: translateX(16px)` — `--text-primary` (light) off, `#1c1c1e` (near-black) on |
| Transition | `--duration-base` `--ease-standard`, matching every other state change in this system |

**Status, 2026-08-30:** on/off no longer reads via hue (`--accent-violet`) — it inverts. Off is a dark track with a light thumb; on flips to a light track (`--surface-filled`) with a dark thumb. Same brightness-carries-the-signal idea as `.nav-btn.active`'s `--surface-active`, and the inversion itself makes "flipped" unambiguous without needing color at all.

## Activity chart

Added 2026-08-30 replacing the Metrics panel's old Latency-table placeholder — a real bar chart of the trailing 14 days' session counts (`HistoryStore::daily_activity`), not a decorative sparkline standing in for data that doesn't exist.

| Property | Value |
|---|---|
| Bars | Equal-width (`flex: 1` each), fixed 56px track, fill height = `count / max(count)` of the visible window |
| Empty day | Flat `rgba(255,255,255,0.10)` sliver (`min-height: 3px`) — present, not absent, so the axis reads as continuous |
| Active day | `--surface-filled` fill — the same token the language bars and toggle "on" state use for "there's real activity/selection here" (was `--accent-violet` before 2026-08-30's monochrome pass) |
| Today | Not a second color — a bold weekday-initial label plus a small `--text-primary` dot beneath it. One chart, one signal (brightness); today is marked, not recolored |
| Entrance | Each bar's fill grows in via `transform: scaleY(0 → 1)` from `transform-origin: bottom`, `--duration-base` `--ease-standard` — never an animated `height`, which forces layout reflow on every frame |
| Labels | Single-letter weekday initials, `9px`, `text-secondary` — a real day axis, not decoration |

## Shape

| Token | Value | Use |
|---|---|---|
| `--radius-pill` | `999px` (full capsule) | The pill HUD itself |
| `--radius-panel` | `14px` | Dashboard panels, cards, buttons |
| `--radius-small` | `8px` | Inputs, small controls |

## Pill dimensions

- Height is fixed at `36px` (down from `44px` — 2026-08-30's compact pass); width is **not** fixed — as of the same day's "flawless shell" pass, the *window itself* resizes to exactly fit `#pill`'s current content width (`session.rs`'s `resize_pill_to_content()`, driven by `pill.js`'s `ResizeObserver` on every state change, not just at startup), rather than sitting inside a fixed-width window with dead space around a narrower state. `190px` in `tauri.conf.json` is only the pre-JS fallback width for the very first paint, not a real dimension the user ever sees rendered — it's superseded within the same frame.
- **Default position:** bottom-center, clear of the Dock — `session.rs`'s `position_pill_above_dock()` computes this from `Monitor::work_area()` (already excludes the Dock and menu bar) on every resize and every show, so it stays correct across Dock resize/auto-hide/monitor changes *and* across the pill's own width changes. This is the default only — see "Draggable" below for what overrides it.
- **Draggable, 2026-08-30, user-directed.** `#pill` carries `data-tauri-drag-region` (`capabilities/pill-drag.json` grants the one permission this needs, `core:window:allow-start-dragging`, scoped to the pill window only) — the same mechanism the dashboard's own titlebar drag handle already uses. `#pill-pause`/`#pill-cancel`, real `<button>` elements nested inside, stay independently clickable without any extra markup, the same way the dashboard's nested traffic-light buttons already do. Once the user drags the pill anywhere, that becomes durable for the rest of the process's life: `position_pill_above_dock` stops repositioning it on every show, and `resize_pill_to_content` preserves its *current* horizontal center and vertical position on every subsequent state-driven width change instead of snapping back to dock-center. `session.rs`'s `PILL_USER_POSITIONED`/`PILL_PROGRAMMATIC_MOVE` (both plain atomics) are how a genuine user drag is told apart from the window's own dock-anchoring/recentering calls, both of which fire the same `WindowEvent::Moved` a real drag does.
- Expandable slightly wider when showing the cancel countdown (needs room for a numeral) — grow, don't reflow; keep the capsule shape. Same mechanism as any other state-driven width change now.
- Never taller than 36px. If content doesn't fit at that height, cut the content, not the height.

**A real, now-fixed corner-notch bug, 2026-08-30.** `reportPillVibrancyLayout()` used to fire twice within ~39ms of page load — once from `ResizeObserver`'s own spec-guaranteed initial callback, once from an extra explicit call right after `.observe()` — and each call resizes the pill window before masking it. The two rapid, overlapping resize+remask cycles left a real, persistent AppKit compositing artifact: a pixel-level ASCII luminance map of a live screenshot showed two distinct rounded-corner arcs superimposed with a step discontinuity between them, not one smooth curve — confirmed and re-confirmed with the same method, not eyeballed. Explicitly forcing `effectView.needsDisplay`/`superview.needsDisplay` after the mask reassignment was tried and confirmed *not* sufficient. The actual fix: debounce `reportPillVibrancyLayout` via `requestAnimationFrame` so both triggers collapse into one call per frame, removing the race at its source rather than fighting its visual symptom after the fact.

**A real, now-fixed WindowServer compositing ghost, 2026-08-30 — a persistent extra "blurred rectangle" behind the dashboard with no backing window.** First misdiagnosed as a stale dev binary (a fresh rebuild happened to look clean at the time); the user restarted `cargo tauri dev` and it was still there, disproving that. Root cause, found via `CGWindowListCopyWindowInfo` reporting only the dashboard's own real window (killing the process cleared the ghost, tying it to this app, not a coincidence): `pill.js` reports its layout on *every* page load regardless of the pill window's own visibility, which meant `set_pill_vibrancy_layout` was resizing and remasking the pill's real `NSVisualEffectView` while it was still genuinely hidden (`visible: false`) — on every single launch, not a rare edge case. Mutating a hidden vibrant window's size/position/mask left a compositing artifact with no window backing it at all. Fixed by gating: `session::apply_pill_layout` (replacing the old unconditional resize+mask call) only touches the real window while `is_visible()` is true — `reveal_pill` now calls `.show()` *before* applying the layout, not after, so the window is genuinely visible by the time anything touches its size or mask. The last-reported content width is still remembered (`PILL_LAST_CONTENT_WIDTH`) so the pill is correctly sized the instant it's shown, it's just never touched while sitting hidden.

## Motion

Subtle and fast — this is a widget, not a marketing site. No motion longer than 200ms except the cancel countdown itself (which is functional, not decorative).

| Token | Value | Use |
|---|---|---|
| `--ease-standard` | `cubic-bezier(0.2, 0.8, 0.2, 1)` | All transitions |
| `--duration-fast` | `120ms` | Pill control hover states |
| `--duration-base` | `200ms` | Pill appear/dismiss, dashboard panel transitions |

The pill's appearance/dismissal should feel like it materializes and dissolves, not slides — a quick opacity + scale (0.96 → 1) on appear, reverse on dismiss.

## Layout principles

- **Pill:** single row, icon-left, status-text-right, nothing else. No settings, no branding, no chrome.
- **Dashboard:** a settings-window layout, not a dashboard-app layout — a floating icon sidebar (not top-tabs) navigates Metrics / History / Settings plus Quit, each section simple enough to fit without scrolling on a laptop screen where reasonable. The Metrics panel itself is four stat tiles (sessions/words/WPM/time saved), a real 14-day activity chart (bars, real `HistoryStore::daily_activity` data — see "Activity chart" below), and a per-language breakdown with per-language WPM — this is the one section with real layout density, everything else stays a single simple list. **Status, 2026-08-30:** the Latency table (stage × p50/p95/samples) this section used to end with is gone — it was a permanent, never-wired placeholder from the reference mockup, not part of Section 8's real metric list, and always showed an empty apology instead of data. The activity chart replaces it with something that's actually true.
- **The sidebar rail is a pill, not a rail.** It's sized to its own content (nav icons + a divider + Quit) and vertically centered next to the full-height content card, not stretched to the window's height — a short floating capsule, consistent with `--radius-pill` and the "widget, not a big desktop app" framing above, rather than a conventional full-height app sidebar. The transparent margin this leaves above/below it is intentional (the window is genuinely transparent there, same mechanism as the Pill surface) — don't "fix" it by giving the sidebar a full-height background.
- **No dock icon, no permanent window.** Both surfaces exist only when summoned (pill: during a recording cycle; dashboard: when opened from the menu-bar icon).
- **Compact, 2026-08-30 (user-directed).** Both surfaces run measurably tighter than their original sizing: dashboard window `600×400` (was `720×480`), sidebar nav icons `26px` (was `32px`), stat numbers `22px` (was `28px`), section/row spacing pulled down a step on the `--space-*` scale throughout. Pill: `190×36` (was `220×44`), every icon/font/control inside scaled down to match. Verified live — the compact dashboard was rendered with real data at its new size, and the compact pill was screenshotted in the real running app (not previewed) at both `listening` and `done`.

## Titlebar overlay (dashboard)

Added 2026-08-30, user-directed: `#panel-header` floats over `#panels` instead of sitting above it in normal flow, the way a native macOS toolbar does. Originally the point was that scrolled content passes underneath and shows through a blur — **that part is gone now** (see "Header background" below); `#panel-header` still floats over `#panels` rather than pushing it down, but scrolled content passing under it is sharp, not blurred.

| Property | Value |
|---|---|
| Header | `position: absolute; top/left/right: 0`, fixed `--header-height` (`44px`), `z-index: 2` — taken out of `#app`'s flex flow entirely, not `sticky` (sticky alone doesn't make content pass *underneath*, since it never overlaps by default) |
| Header background | **No fill, no backdrop-filter — a plain transparent overlay showing `#app`'s own fill straight through (2026-08-30, user-directed, final state).** Three things were tried in order, each reverted for a real, measured reason: (1) a lighter `rgba(20,20,20,0.45)` fill, reverted — looked wrong, read as its own band; (2) repainting `var(--glass-bg)` a second time (matching `#app`'s own fill) plus a blur, reverted — `#panel-header` is a child of `#app`, which is already `.glass-panel` and already paints that exact fill underneath, so the second layer compounded #app's ~0.82 alpha with itself to ~0.97, reading as a flatter, more opaque black than the rest of `#app`; (3) dropping the fill but keeping just `blur()` (radius tried from 6px to 20px, `saturate()` on or off), reverted — measured live at ~25% darker/less blue than the body regardless of radius (header ~RGB(28,31,32) vs. body ~RGB(39,53,65)), which turned out to be a real WebKit limitation: a `backdrop-filter` nested inside an ancestor that already has its own `backdrop-filter` doesn't sample the true blurred desktop behind it, only a flatter composite of the ancestor's own fill — not a value to tune around. Given "match the body" and "keep the blur" turned out to be mutually exclusive here, the user chose matching the body; the small remaining color difference at the very top of the window is the real desktop photo's own gradient, not a bug |
| Border | None. The old `border-bottom` is gone — there's no separator between header and content at all now, just a shared fill |
| `#panels` | Fills the full card height (no longer reduced by the header's flex space) with `padding-top: calc(var(--header-height) + var(--space-2))`, so first-paint content still starts visually below the header even though it's now technically full-bleed behind it |

## Audit fixes, 2026-08-30

`/impeccable audit` ran a technical quality pass across all three surfaces (14/20, "Good"); all six recommended fixes landed the same day, in priority order:

- **Recovery `#quit-btn` contrast + focus.** White text on `--danger` measured ~3.38:1 (below 4.5:1 AA); text is now black on the same red (~6.2:1). The button — the only interactive control in that window — also had no `:focus-visible` at all; added, using a new `--focus-ring` token in `recovery.css`'s own `:root` (that file previously had none).
- **Dashboard `minWidth`/`minHeight`.** The window was `resizable: true` with no floor anywhere — `.hotkey-input`'s fixed `220px` width could overflow a user-shrunk window. Set to `420×320` in `tauri.conf.json`.
- **Pill waveform: `height` → `transform: scaleY()`.** `.pill-waveform .bar`'s `wave` animation was the one place the codebase's own established fix for this exact anti-pattern (see the Activity chart section) never got applied — animating a layout property, infinitely, on 5 elements, for the full length of every recording. Now transform-only; `align-items: center` on the parent already centered the bar as its height changed, so `scaleY`'s default center origin reproduces the same growth with no layout cost.
- **`prefers-reduced-motion` for the waveform.** The app's only continuous, unbounded animation had no accommodation anywhere. Added a media query that freezes the bars to distinct static heights (still reads as a waveform, just a still frame) instead of animating.
- **Settings panel: `<label>` → `<span class="setting-label">`.** Four rows (Microphone/Accessibility/Screen Recording/Engine) used `<label>` with no `for` and no wrapped control — semantically incorrect for read-only status rows. No CSS needed; both render identically inline.
- **Un-tokenized opacity fills, both files.** `dashboard.css` had five neutral fill values repeated inline, two of them (`0.08` and `0.10`) doing the same "bar track" job at slightly different, likely-accidental values — now one `--surface-track` token, plus `--surface-inset`/`--surface-toggle-track`/`--surface-hover` for the other three roles. `pill.css` got its own local `--surface-control`/`--surface-control-hover` for `.pill-btn`'s two states. Pure rename — every replaced value is numerically identical to what was there before, confirmed live (pixel-identical Settings-panel screenshot before/after).
- **Recovery window `windowEffects`.** The one surface still missing native vibrancy (flagged, never actioned, in an earlier pass) — added the same `hudWindow` config pill and dashboard already have. Verified live: the panel now reads as genuinely frosted, not flat/glassy.

`cargo build`/`fmt --check`/`clippy`/`test` all clean; every fix verified live (temporary default-panel edits for Settings/History, reverted and diffed after each).

## What this file is not

Not a component library, not a build system. The vibrancy/transparency approach above was the risk area named in `docs/mutter-project-plan.md` Section 4 — resolved, not hypothetical: the Phase 0 pill-feasibility spike confirmed real per-pixel transparency and the capsule shape, and the "Glass effect" section's `windowEffects` fix (2026-08-30) confirmed real native blur on top of that, both verified live rather than assumed. The rectangular-HUD fallback this paragraph used to name is no longer needed.
