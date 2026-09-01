# Design System — Mutter

**Status: rewritten 2026-08-31, user-directed (`/design-consultation`), replacing the prior dark-monochrome-glassmorphism system wholesale.** The old system was a real, live-verified implementation (native vibrancy masking, corner-radius shims, etc.) — the mechanism notes worth keeping are folded into this file below. The visual/material language itself is new: it follows Apple's current design system, **Liquid Glass** (introduced WWDC 2025, shipping in macOS Tahoe), rather than the flat 2023-era frosted-glass look the prior system was built to. See the design-consultation preview generated the same day for the visual reference this file was written from.

## Product Context

- **What this is:** a macOS-only, local-first speech-to-text dictation app. Two surfaces: a floating "pill" HUD that appears only during an active recording, and a settings/metrics dashboard opened from the menu bar.
- **Who it's for:** the developer themself, first — dictating specs and bug reports to AI coding agents, plus general dictation. Single-user v1.
- **Space:** competes conceptually with SuperWhisper, WhisperFlow, Aqua Voice; UI posture is closer to a native macOS widget (Control Center, Notification Center, desktop widgets) than to any of those apps' own UIs.
- **Project type:** two small native-feeling desktop surfaces, not a web app. No page navigation, no marketing surface.
- **The one thing to remember:** *this feels like it shipped from Apple.* Every decision below is in service of that — not "looks nice," specifically "reads as first-party."

## Aesthetic Direction

- **Direction:** Native Liquid Glass, legibility-calibrated. Real Liquid Glass principles (lensing, morph, concentricity, adaptive tint) — not a copy of them filtered through generic "glassmorphism" — but deliberately not chasing maximum transparency. Several macOS Tahoe reviews flagged Liquid Glass's transparent menu bar as illegible against busy wallpaper; Mutter's pill has to stay readable mid-sentence, so contrast is a hard floor here, not a variable to relax for purity.
- **Decoration level:** intentional. The glass material itself — lensing rim, morph-driven thickness, adaptive tint — is the only decoration. No icons in colored circles, no illustration, no gradients-as-ornament.
- **Mood:** quiet, precise, native. A widget that happens to be excellent, not an app trying to look impressive.
- **Reference:** Apple's Liquid Glass HIG (Materials section, updated post-WWDC25); macOS Tahoe's own widget gallery, Control Center, and Dock treatment of glass materials.

## Material system (the core of this redesign)

This replaces the old flat `.glass-panel` (blur + saturate + flat fill + 1px hairline border) with three real Liquid Glass mechanisms, approximated in CSS/WKWebView since true real-time refraction isn't available outside SwiftUI's native `glassEffect`:

### 1. Lensing (replaces the flat hairline border)

Liquid Glass defines edges by bending/concentrating light at the boundary, not by drawing a line. Approximated via a gradient border that's brightest where light would catch the glass (top-left) and fades around the rest — not a uniform-opacity hairline.

```css
.glass {
  position: relative;
  background: var(--glass-bg);
  backdrop-filter: blur(26px) saturate(1.5);
  -webkit-backdrop-filter: blur(26px) saturate(1.5);
  border-radius: var(--radius-panel);
  isolation: isolate;
}
.glass::before {
  content: "";
  position: absolute; inset: 0;
  border-radius: inherit;
  padding: 1px;
  background: linear-gradient(135deg,
    rgba(255,255,255,0.55) 0%,
    rgba(255,255,255,0.12) 30%,
    rgba(255,255,255,0.04) 60%,
    rgba(255,255,255,0.14) 100%);
  -webkit-mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
  -webkit-mask-composite: xor;
  mask-composite: exclude;
  pointer-events: none;
}
```

### 2. Morph (glass gets "thicker" as it grows — a real, documented HIG behavior)

Apple's own example: a menu expanding from a toolbar button gets deeper shadow, more pronounced lensing, and softer light scatter as it grows. Mutter's pill already resizes on every state change (`listening` is wide — waveform, timer, controls; `done`/`canceling` are narrow) — this behavior maps onto that directly, and didn't exist in the prior design pass at all.

| State | Material | Blur | Shadow |
|---|---|---|---|
| `.thin` (narrow: `done`, `canceling` without the wide countdown) | `--glass-bg` | `blur(26px) saturate(1.5)` | `0 6px 16px -8px rgba(0,0,0,0.4)` |
| `.thick` (wide: `listening`, dashboard's main card) | `--glass-bg-thick` | `blur(34px) saturate(1.65)` | `0 18px 40px -12px rgba(0,0,0,0.55), inset 0 1px 0 rgba(255,255,255,0.08)` |

Transition both on the same `--duration-base`/`--ease-standard` the resize itself already uses — thickness change and width change read as one motion, not two.

### 3. Adaptive tint (replaces the old pill-is-colorful / dashboard-is-monochrome split)

**This is the real departure from the prior system.** The old design ran two different color philosophies between the app's own two windows — the pill had a violet/red/green identity, the dashboard was strictly monochrome. That reads as two apps, not one first-party surface. Liquid Glass's actual behavior is content-adaptive tinting: a soft, context-derived wash blended into the glass, not a flat color fill. One tint system now covers both surfaces:

```css
.glass::after {
  content: "";
  position: absolute; inset: 0;
  border-radius: inherit;
  background: radial-gradient(120% 140% at 20% -10%,
    rgba(var(--tint, var(--tint-neutral)), var(--tint-strength, 0)) 0%, transparent 60%);
  mix-blend-mode: soft-light;
  pointer-events: none;
  transition: background 260ms var(--ease-standard);
}
```

| Token | Value | Use |
|---|---|---|
| `--tint-violet` | `139, 124, 246` | Listening / active — pill mic state **and** dashboard's active nav icon, selected states |
| `--tint-red` | `255, 69, 58` | Canceling / destructive — pill cancel countdown **and** dashboard Quit button |
| `--tint-green` | `48, 209, 88` | Done / success — pill confirmation **and** dashboard success confirmations |
| `--tint-neutral` | `255, 255, 255` | Idle — no state, `--tint-strength: 0` |
| `--tint-strength` (`--tint-violet`) | `0.55` | Set by `.glass-panel--tint-violet` |
| `--tint-strength` (`--tint-red` / `--tint-green`) | `0.5` | Set by `.glass-panel--tint-red` / `--tint-green` |
| `--tint-strength` (`--surface-active`, dashboard selection wash) | `0.18` | Not the pseudo-element mechanism above — see "Surface / neutral tokens" below |

## Surface / neutral tokens

The small solid-fill elements (nav-icon selection, toggle tracks, activity/language bar fills, inset boxes) don't run the full lensing/tint pseudo-element mechanism — that's reserved for real glass panels with their own backdrop-filter. These are flat `background-color` fills reading the same two tint values so retinting them is one token change, not a per-component rewrite:

| Token | Value | Use |
|---|---|---|
| `--surface-active` | `rgba(139, 124, 246, 0.18)` | Sidebar's active nav-icon background |
| `--surface-filled` | `rgba(139, 124, 246, 0.55)` | Toggle's on-track, activity chart's active-day bar, language bar's fill, onboarding's active progress dot, PermissionRow's Grant button and Onboarding/Continue CTA fill |
| `--surface-inset` | `rgba(255, 255, 255, 0.06)` | Recessed boxes — search input, HotkeyCapture's card, onboarding's Ready-step hotkey rows, History's copy button |
| `--surface-track` | `rgba(255, 255, 255, 0.08)` | Empty/inactive bar track (activity chart, language bar) |
| `--surface-toggle-track` | `rgba(255, 255, 255, 0.12)` | Toggle's off-track, HotkeyCapture's click-to-capture button |
| `--surface-hover` | `rgba(255, 255, 255, 0.16)` | Hover fill on secondary buttons/rows |
| `--focus-ring` | `rgba(139, 124, 246, 0.6)` | Every `:focus-visible` ring, both surfaces |

**No box borders on recessed elements** — matching the design-consultation preview's consistent treatment (rely on the ancestor `.glass-panel`'s own lensing rim for edge definition, not a second border on nested controls). Search inputs, hotkey rows/cards, and the onboarding Permissions-step wrapper all dropped their `border-glass-border` for this reason (2026-09-01). `--glass-border` itself is unchanged and still used for row-divider hairlines (`border-b`) — that's a different concept (separating stacked rows) from a box outline around one control.

## Color

| Token | Value | Use |
|---|---|---|
| `--glass-bg` | `rgba(22, 22, 24, 0.72)` | Base fill, `.thin` glass — cooler/more neutral than the prior `rgba(20,20,20,0.82)`, tuned lower-opacity to let lensing/tint actually read, still well above the transparency level that made macOS Tahoe's own menu bar criticized as illegible |
| `--glass-bg-thick` | `rgba(22, 22, 24, 0.80)` | Base fill, `.thick` glass (morph state) |
| `--text-primary` | `rgba(255, 255, 255, 0.94)` | Primary text/icons, both surfaces |
| `--text-secondary` | `rgba(255, 255, 255, 0.5)` | Secondary text, timestamps, hints |

No separate "pill palette" / "dashboard palette" tables anymore — see Adaptive tint above. This is a deliberate reversal of the prior file's explicit "these are two real, permanently divergent palettes" call.

**Traffic lights stay real macOS red/yellow/green**, set inline, not routed through the tint tokens — a fixed system convention, unrelated to app state.

## Typography

System font stack, unchanged — this is what makes it read as native for free, and isn't up for revision:

```css
font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", sans-serif;
```

**New: rounded numerals for glanceable data.** Apple's own glanceable-data widgets (Weather, Fitness) use SF Pro Rounded for exactly this reason — numbers you're meant to read at a glance, not study. `ui-rounded` is a real CSS generic-family keyword that resolves to actual SF Pro Rounded in WKWebView (Tauri's rendering engine) — zero bundle cost, not a web font.

```css
.stat-value {
  font-family: ui-rounded, "SF Pro Rounded", -apple-system, BlinkMacSystemFont, sans-serif;
  font-variant-numeric: tabular-nums;
  font-weight: 650;
  font-size: 17px;
}
```

Used only for the dashboard's four stat-tile numbers (sessions/words/WPM/time-saved). Everywhere else (body text, labels, pill status text) stays the standard system stack — this is a targeted accent, not a font swap across the app.

| Token | Size | Weight | Use |
|---|---|---|---|
| `--text-xs` | 10px | 500 | Timestamps, metadata |
| `--text-sm` | 12px | 400 | Body text, settings labels |
| `--text-base` | 13px | 500 | Pill status text, primary dashboard labels |
| `--text-lg` | 15px | 600 | Dashboard panel title |
| stat-value (rounded) | 17px | 650 | The Metrics stat tiles only |

Unchanged from the prior system — this scale was already right for the app's size and doesn't need relitigating.

**A handful of small controls use one-off arbitrary sizes slightly outside this scale** — PermissionRow's Granted/Denied pills and Grant button (9px text, 7–9px padding), History's copy button (20px, 6px radius), onboarding's keycap badges (5px radius). These match the design-consultation preview's own computed values exactly, verified via `getComputedStyle()` against the preview HTML rather than eyeballed — see each component's own code comment for the specific value, not duplicated here since they're genuinely one-off, not a new scale step.

## Shape & concentricity

| Token | Value | Use |
|---|---|---|
| `--radius-pill` | `999px` (full capsule) | The pill HUD |
| `--radius-panel` | `16px` | Dashboard card, large glass surfaces |
| `--radius-small` | `9px` | Inputs, stat tiles, small controls |

**Concentricity rule (new — formalized, wasn't derived from anything before):** when one rounded shape sits inside another with padding `p`, the outer radius should equal the inner radius plus `p`, not an independently-chosen value. Example: a `--radius-small` (9px) control sitting in `--space-2` (8px) of padding inside a container should give that container roughly `17px`, not an arbitrary `--radius-panel`. Apply this rule when adding any new nested glass shape rather than picking a number that "looks right."

**Squircle corners (flagged follow-up, not fully resolved by this pass).** Apple's continuous-corner curve is visibly different from a plain CSS `border-radius` arc up close — it's one of the most recognizable "this is a real Apple surface" tells. The preview demonstrates the effect via a fixed `clip-path` superellipse path at one specific size; a real implementation needs a small helper (JS, computed per element's actual width/height — likely alongside the existing vibrancy-layout reporting mechanism below) to generate that path parametrically instead of hardcoding it. Not done in this design pass — real engineering work, tracked as a TODO.

## Spacing

Unchanged, 4px base unit — already correct for a 36px-tall capsule and a ~380px-wide dashboard card:

`--space-1: 4px` · `--space-2: 8px` · `--space-3: 12px` · `--space-4: 16px` · `--space-5: 24px` · `--space-6: 32px`

## Motion

| Token | Value | Use |
|---|---|---|
| `--ease-standard` | `cubic-bezier(0.2, 0.8, 0.2, 1)` | All transitions, including the new thin↔thick morph |
| `--duration-fast` | `120ms` | Hover states |
| `--duration-base` | `200ms` | Pill appear/dismiss, dashboard transitions, morph thickness change |

Still subtle and fast — this is a widget, not a marketing site. The one new rule: **thickness and width change together, on the same duration** — never animate the blur/shadow morph independently of the resize it's responding to, or the two reads as two separate effects instead of one material behaving physically.

## Layout

**Unchanged from the prior system, deliberately — flagged, not silently kept.** The sidebar-pill + content-card dashboard shell and the single-row pill capsule are both real, live-verified, backed by non-trivial native code (the two-shape vibrancy mask, `mask_to_shapes`, the corner-radius shims documented in git history). This redesign is about material, color, and motion — rewriting the window skeleton on top of that would discard working native engineering for no demonstrated visual gain. Revisit only if a specific problem with the current skeleton shows up.

- **Pill:** single row, icon-left, status-text-right. No settings, no branding, no chrome. Every active state (`listening`/`canceling`/`done`) reads through one consistent status-dot + glow language now (2026-09-01) — the mic-icon SVG that used to be `listening`'s own one-off indicator is gone, replaced by the same violet dot the other two states already used in red/green. Horizontal padding/internal gap now read the shared `--space-4`/`--space-2` tokens directly, not pill-only spacing values (the old `--pill-gap-sm`/`--pill-gap-md` tokens are removed).
- **Dashboard:** floating icon sidebar (not top-tabs), vertically centered next to a full-height content card. Sidebar stays sized to its own content, not stretched — a short floating capsule, not a conventional full-height app rail. Sidebar width is `40px` (nav icons `26px`, `--radius-small` corners — **squarish chips, not circles**, matching the design-consultation preview's `.nav-icon` exactly). The sidebar rail itself stays `--radius-pill` (a real, deliberate divergence from the preview's own computed 16px — see Sidebar.tsx's code comment: the preview's `.dash-sidebar` never overrides `.glass`'s default radius, a real gap in that file rather than an intended design, and a 40px-wide rail at 16px corners reads as a squared card, not the floating capsule this app's identity depends on). No decorative brand mark above the nav icons anymore (removed 2026-09-01, user-directed) — nav icons only.
- **No dock icon, no permanent window.** Both surfaces exist only when summoned.
- **Settings panel section order:** Permissions → Hotkey → Output → Updates (2026-09-01). Hotkey configuration (click-to-capture, `HotkeyCapture`) is now an always-visible section, promoted out of a collapsed "Advanced" disclosure — the design-consultation preview shows it as first-class, not a hidden power-user detail. Still click-to-capture, not a typed-string-plus-Save field: that interaction was deliberately replaced pre-redesign for a real, tested UX reason (see `HotkeyCapture.tsx`'s own module doc), and the preview's simpler static mock isn't grounds to regress it.

## Implementation notes (native rendering, unchanged mechanism)

The actual frosted-glass effect requires real macOS `NSVisualEffectView` vibrancy (`windowEffects: { effects: ["hudWindow"] }` in `tauri.conf.json`) underneath the CSS — `backdrop-filter` alone in a transparent WKWebView window blurs only the page's own render tree, not the real desktop. Both windows' vibrancy is corner-masked via a native Objective-C shim (`native/vibrancy_mask_shim.m`) using the public `-[NSVisualEffectView setMaskImage:]` API, driven by real-time layout geometry reported from the frontend (`reportVibrancyLayout()` → `set_vibrancy_layout` / `set_pill_vibrancy_layout` commands). This mechanism is unaffected by this redesign — the new material system (lensing, tint, morph) is a CSS-layer change on top of the same native vibrancy substrate. If the squircle-corner follow-up above is built, its geometry needs to flow through this same reporting path so the native mask matches the CSS clip-path exactly.

## Decisions Log

| Date | Decision | Rationale |
|---|---|---|
| 2026-08-31 | Full material-system rewrite via `/design-consultation`, replacing flat glassmorphism with Liquid Glass-derived lensing/morph/tint | User-directed: "ignore the current design system... embrace the apple design philosophy" — the prior system predated Apple's current (WWDC25+) design language |
| 2026-08-31 | Unified adaptive tint across pill + dashboard, retiring the colorful-pill/monochrome-dashboard split | Two color philosophies in one app's own two windows contradicted the "shipped from Apple" goal |
| 2026-08-31 | `ui-rounded` (SF Pro Rounded) added for dashboard stat numerals only | Matches Apple's own use of rounded numerals for glanceable data (Weather, Fitness widgets); zero cost via native WKWebView font resolution |
| 2026-08-31 | Dashboard/pill window skeleton kept as-is | Real native vibrancy-masking engineering already invested and live-verified; redesign scoped to material/color/motion, not window architecture |
| 2026-08-31 | Squircle corner-radius left as a flagged follow-up, not implemented | Needs a parametric path-generation helper tied to the existing vibrancy-layout reporting mechanism; out of scope for a design-tokens pass |
| 2026-09-01 | Sidebar decorative wave-mark icon removed entirely | User-directed; the preview's sidebar is nav icons only |
| 2026-09-01 | Pill unified to one status-dot language across listening/canceling/done, replacing listening's one-off mic-icon SVG | User-directed literal preview match; closes a real inconsistency (only one of three states had its own icon) |
| 2026-09-01 | Toggle resized to 34×19px track / 15px thumb, border removed; Settings/onboarding recessed boxes lost their box borders app-wide | User-directed literal preview match — preview relies on the ancestor glass panel's lensing rim for edge definition, not per-control borders |
| 2026-09-01 | Settings panel restructured: Hotkey promoted out of collapsed "Advanced" into an always-visible section; order becomes Permissions → Hotkey → Output → Updates | Preview shows hotkey config as first-class, not hidden; click-to-capture interaction itself preserved (not regressed to a text-input+Save field) |
| 2026-09-01 | PermissionRow's Grant button restyled to a solid violet rounded-pill; Granted/Denied became tinted status pills | Preview's `.grant-btn`/`.status-pill` treatment — the green/red tint tokens weren't doing any real signaling work before this |
| 2026-09-01 | `--surface-inset` bumped 0.04 → 0.06; pill-only spacing tokens (`--pill-gap-sm/md`) retired in favor of the shared `--space-2`/`--space-4` scale; several small controls (status pills, Grant button, History's copy button, onboarding keycaps) sized to one-off values; sidebar nav icons changed from circles to `--radius-small` chips | Found via direct `getComputedStyle()` diffing of the preview HTML against the shipped app (`/design-review`), not visual inspection — see PR/commit for the full value-by-value list |
