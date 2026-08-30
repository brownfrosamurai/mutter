# Mutter — Design System

Source: `docs/mutter-idea-dump.md` ("Super minimal glassmorphic UI... should feel like it's an Apple product... a widget, not a huge desktop application") and `docs/mutter-project-plan.md` Section 4.

Two surfaces only. No design system beyond what these two surfaces need — do not add components speculatively.

## Surfaces

1. **The Pill** — floating, always-on-top HUD. Visible only during an active recording/transcribing/canceling cycle. States: `loading` (first use only), `listening`, `transcribing`, `canceling`, `done`.
2. **Dashboard/Settings window** — opened from the menu-bar icon. Metrics, history, hotkey config, language, engine selection, permissions, Quit.

Both share the same visual language below. Neither is a "big app window" — see Layout.

## Color

Dark-leaning glassmorphism (frosted glass reads better on a translucent dark base than a light one, and matches the macOS menu-bar-adjacent aesthetic this app lives in).

| Token | Value | Use |
|---|---|---|
| `--glass-bg` | `rgba(28, 28, 30, 0.55)` | Base fill behind the blur, both surfaces |
| `--glass-border` | `rgba(255, 255, 255, 0.12)` | 1px hairline border on every glass panel |
| `--glass-highlight` | `rgba(255, 255, 255, 0.06)` | Subtle top-edge inner highlight (linear-gradient), gives the "glass" a light source |
| `--text-primary` | `rgba(255, 255, 255, 0.92)` | Primary text/icons |
| `--text-secondary` | `rgba(255, 255, 255, 0.55)` | Secondary text, timestamps, hints |
| `--accent` | `#0A84FF` | macOS system blue — active states, the listening waveform, primary buttons |
| `--danger` | `#FF453A` | Cancel countdown, error markers (`[transcription failed]`) |
| `--success` | `#30D158` | Done/confirmation state |

No custom color beyond this table without updating it here first.

## Typography

System font stack only — never bundle a custom font, this is what makes it "feel like an Apple product" for free:

```css
font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", sans-serif;
```

| Token | Size | Weight | Use |
|---|---|---|---|
| `--text-xs` | 11px | 500 | Timestamps, metadata in history rows |
| `--text-sm` | 13px | 400 | Body text, settings labels |
| `--text-base` | 15px | 500 | Pill status text, primary dashboard labels |
| `--text-lg` | 20px | 600 | Dashboard section headers, metric numbers |

## Spacing

4px base unit. Use the scale, never an arbitrary value:

`--space-1: 4px` · `--space-2: 8px` · `--space-3: 12px` · `--space-4: 16px` · `--space-5: 24px` · `--space-6: 32px`

## Glass effect (the actual CSS)

```css
.glass-panel {
  background: var(--glass-bg);
  backdrop-filter: blur(24px) saturate(1.4);
  -webkit-backdrop-filter: blur(24px) saturate(1.4);
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-panel);
  box-shadow:
    inset 0 1px 0 var(--glass-highlight),
    0 8px 32px rgba(0, 0, 0, 0.35);
}
```

Tauri window setup must pair with this: `transparent: true` on the window, and macOS window-effects vibrancy (`WindowEffect::HudWindow` or `Sidebar`, whichever survives the Phase 0 feasibility spike — plan Section 4 flags real risk here, don't assume the CSS alone is sufficient without a transparent native window backing it).

## Shape

| Token | Value | Use |
|---|---|---|
| `--radius-pill` | `999px` (full capsule) | The pill HUD itself |
| `--radius-panel` | `14px` | Dashboard panels, cards, buttons |
| `--radius-small` | `8px` | Inputs, small controls |

## Pill dimensions

- Collapsed/listening: `~180px × 44px` capsule, centered near the top of the screen (or near the menu bar — confirm exact position during the Phase 0 feasibility spike).
- Expandable slightly wider when showing the cancel countdown (needs room for a numeral) — grow, don't reflow; keep the capsule shape.
- Never taller than 44px. If content doesn't fit at that height, cut the content, not the height.

## Motion

Subtle and fast — this is a widget, not a marketing site. No motion longer than 200ms except the cancel countdown itself (which is functional, not decorative).

| Token | Value | Use |
|---|---|---|
| `--ease-standard` | `cubic-bezier(0.2, 0.8, 0.2, 1)` | All transitions |
| `--duration-fast` | `120ms` | State icon swaps (listening → transcribing) |
| `--duration-base` | `200ms` | Pill appear/dismiss, dashboard panel transitions |

The pill's appearance/dismissal should feel like it materializes and dissolves, not slides — a quick opacity + scale (0.96 → 1) on appear, reverse on dismiss.

## Layout principles

- **Pill:** single row, icon-left, status-text-right, nothing else. No settings, no branding, no chrome.
- **Dashboard:** a settings-window layout, not a dashboard-app layout — sidebar or top-tab navigation between Metrics / History / Settings, each section simple enough to fit without scrolling on a laptop screen where reasonable.
- **No dock icon, no permanent window.** Both surfaces exist only when summoned (pill: during a recording cycle; dashboard: when opened from the menu-bar icon).

## What this file is not

Not a component library, not a build system, not a claim about implementation feasibility — the vibrancy/transparency approach above is exactly the risk area named in `docs/mutter-project-plan.md` Section 4 and is subject to the Phase 0 pill-feasibility spike. If that spike finds the native-window + CSS combination doesn't hold up, the fallback is a simpler rectangular HUD using the same color/type/spacing tokens above, just without the custom capsule shape.
