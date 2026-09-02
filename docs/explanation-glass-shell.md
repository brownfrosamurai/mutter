# Why the native window "glass shell" works the way it does

Every window in Mutter is `transparent: true` with real macOS window vibrancy underneath — CSS `backdrop-filter` alone in a transparent WKWebView window only blurs the page's own render tree, not the real desktop behind it, so a genuinely frosted look needs a real `NSVisualEffectView`/`NSGlassEffectView` under the CSS, not instead of it. This document explains how that native layer is applied today, and the two failed approaches that led here.

## The current mechanism: `vibrancy::apply_glass_shell`

One function (`src-tauri/src/vibrancy.rs`), called once per window at startup from `lib.rs`'s `setup()` — never re-applied on resize or layout change. All four windows go through it now (onboarding and recovery migrated 2026-09-01, matching dashboard/pill's earlier migration):

```rust
vibrancy::apply_glass_shell(&dashboard_window, DASHBOARD_SHELL_RADIUS);  // 16.0
vibrancy::apply_glass_shell(&pill_window, PILL_SHELL_RADIUS);            // session::PILL_HEIGHT / 2.0
vibrancy::apply_glass_shell(&onboarding_window, DASHBOARD_SHELL_RADIUS); // same radius as dashboard
vibrancy::apply_glass_shell(&recovery_window, DASHBOARD_SHELL_RADIUS);   // same radius as dashboard
```

Onboarding and recovery share the dashboard's radius and its dead-space trade-off (below), but not its call-site timing: both are shown directly inside `setup()` itself (recovery on a migration failure, onboarding on first run), so the shell has to be applied *before* `.show()` right there — the shared post-`setup()` block dashboard/pill use, whose own comment claims correctness "whenever the window is later shown," never actually covers a window shown from inside `setup()` itself. Getting this ordering backwards was a real bug caught by a pre-landing adversarial review (2026-09-01): a window briefly visible without its glass shell applied.

On macOS 26+ (Tahoe), this is `window-vibrancy` 0.8's `apply_liquid_glass` — Apple's real `NSGlassEffectView`, with a genuine public `cornerRadius` property and a `content_view` option that **reparents the WKWebView into the glass view** instead of leaving it a sibling. On older macOS it falls back to plain `apply_vibrancy` (flat, unrounded `NSVisualEffectView`).

Because `NSGlassEffectView.cornerRadius` is a real CALayer-backed property, it tracks the view's own frame automatically on resize — there is nothing to re-report or re-apply as the window changes size. This is a meaningful simplification versus what came before.

## Why this needed two earlier, abandoned attempts to get right

### Attempt 1: `tauri.conf.json`'s declarative `windowEffects.radius`

The obvious-looking fix. It routes through `window-vibrancy`'s `setCornerRadius:`, which that crate's own source comment admits is an **undocumented, private AppKit selector** ("might be private, but it works"). Calling it made the dashboard window render as vibrancy-only — zero WKWebView content, a completely blank window. Reproduced twice, confirmed via a clean A/B, and corroborated by an open upstream Tauri issue reporting a different symptom of the same feature. Reverted entirely.

### Attempt 2: a hand-built `NSVisualEffectView.maskImage` shim

The next fix was a small Objective-C shim (`vibrancy_mask_shim.m`, now deleted) that found the `NSVisualEffectView` Tauri's declarative `windowEffects` config already created, and reshaped only its visible region via the one *public, documented* API for this — `-[NSVisualEffectView setMaskImage:]`. The frontend reported real-time layout geometry (`getBoundingClientRect()`, via a `ResizeObserver`) on every content-size change, and the shim regenerated a CoreGraphics bitmap mask to match.

**This genuinely worked for the pill** — verified live, including pixel-level ASCII-luminance-map forensics confirming a single smooth rounded curve at every corner, no discontinuity. It never worked reliably for the dashboard: the native call reported success (`applied=true`) with mathematically-correct geometry, but the surrounding dead space (the gap around the floating sidebar) kept rendering full, square-cornered vibrancy regardless of what shape was requested.

**Root cause, found via actual research, not another guess:** `NSVisualEffectView.maskImage` only reliably masks when the effect view *is* the window's content view — not a subview sitting alongside a WKWebView sibling, which is this app's exact setup in every window. `apply_liquid_glass`'s `content_view` reparenting (the current mechanism, above) fixes this at the actual source: there's no compositing-order mismatch left to fight, because the WebView is no longer a sibling of the glass view at all.

## Why the dashboard and the pill end up with different trade-offs today

Both windows now call the same `apply_glass_shell` function, but they get a materially different result, for a real geometric reason:

- **The pill has zero dead-space trade-off.** `session::resize_pill_to_content` already resizes the pill *window itself* to exactly fit `#pill`'s content on every state change (originally built to fix a separate blur-bleed "wisp" past the old mask's hard alpha edge — see below). Combined with the pill's constant height (`PILL_HEIGHT = 36.0`, only its width ever changes), the window's content view is *always* precisely the capsule shape already, at a fixed corner radius of `height / 2`. A single constant radius, applied once, has nothing left to get wrong.
- **The dashboard has a real, deliberate dead-space trade.** Its floating sidebar-plus-card layout leaves a genuine gap between the two shapes (and around them), and that gap is a real design element, not incidental — it can't be resized away the way the pill's dead space could. The whole window's content view — sidebar, card, and the gap between them — shares one glass shell rounded at the window's own outer radius (`DASHBOARD_SHELL_RADIUS = 16px`, matching `--radius-panel`). The gap is therefore vibrant too, not real unblurred desktop. This was an explicit, user-confirmed trade for reliability over the masking approach's fragility, made 2026-08-31, and the pill's later migration onto the same mechanism (2026-09-01) inherited it as a known, accepted asymmetry between the two windows rather than a bug to chase.

## The pill's "wisp" bug, and why it's now structurally impossible

Before the window-resize fix existed, the pill window was fixed-size (190×36) regardless of its narrower `done`/`canceling` states' actual content width. That left real dead space inside the window, and native vibrancy's own internal blur radius (intrinsic to `NSVisualEffectView`'s material, no public API to control it) bled a soft "wisp" past the pill's rounded end into that dead space — a real platform limitation, not a mask-geometry bug, since the blur needs *some* transparent space next to a hard alpha edge to bleed into.

The actual fix worked one level up from masking entirely: `resize_pill_to_content` shrinks the *window* to have no dead space left at all, removing the space the blur had to bleed into rather than trying to mask around it. This is also, as a side effect, exactly what makes the current `apply_glass_shell` migration correct with zero compromise — see above.

## Related

- [`reference-architecture.md`](reference-architecture.md) — the four windows and their `tauri.conf.json` shape
- `DESIGN.md`'s "Implementation notes (native rendering)" and "Glass effect" sections — the full incident-by-incident history with live-verification detail
