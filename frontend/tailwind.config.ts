import type { Config } from "tailwindcss";

// Theme extension only — colors/spacing/radii/motion all resolve through
// CSS custom properties defined in src/styles/globals.css (carried over
// verbatim from the pre-React DESIGN.md token system), not hardcoded here.
// This keeps one token source instead of two, and preserves the dashboard's
// monochrome-accent-system / pill's separate hued-accent split documented
// in DESIGN.md.
export default {
  content: ["./*.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        "glass-border": "var(--glass-border)",
        "glass-highlight": "var(--glass-highlight)",
        "text-primary": "var(--text-primary)",
        "text-secondary": "var(--text-secondary)",
        danger: "var(--danger)",
        warning: "var(--warning)",
        success: "var(--success)",
        "surface-active": "var(--surface-active)",
        "surface-filled": "var(--surface-filled)",
        "surface-inset": "var(--surface-inset)",
        "surface-track": "var(--surface-track)",
        "surface-toggle-track": "var(--surface-toggle-track)",
        "surface-hover": "var(--surface-hover)",
        "focus-ring": "var(--focus-ring)",
        "accent-violet": "var(--accent-violet)",
        "surface-control": "var(--surface-control)",
        "surface-control-hover": "var(--surface-control-hover)",
      },
      spacing: {
        "1": "var(--space-1)",
        "2": "var(--space-2)",
        "3": "var(--space-3)",
        "4": "var(--space-4)",
        "5": "var(--space-5)",
        "6": "var(--space-6)",
        "pill-sm": "var(--pill-gap-sm)",
        "pill-md": "var(--pill-gap-md)",
      },
      borderRadius: {
        pill: "var(--radius-pill)",
        panel: "var(--radius-panel)",
        small: "var(--radius-small)",
      },
      fontSize: {
        xs: ["var(--text-xs)", { fontWeight: "500" }],
        sm: ["var(--text-sm)", { fontWeight: "400" }],
        base: ["var(--text-base)", { fontWeight: "500" }],
        lg: ["var(--text-lg)", { fontWeight: "600" }],
      },
      transitionTimingFunction: {
        standard: "var(--ease-standard)",
      },
      transitionDuration: {
        fast: "var(--duration-fast)",
        base: "var(--duration-base)",
      },
      fontFamily: {
        system: [
          "-apple-system",
          "BlinkMacSystemFont",
          "SF Pro Text",
          "SF Pro Display",
          "sans-serif",
        ],
      },
    },
  },
  plugins: [],
} satisfies Config;
