import type { HTMLAttributes, ReactNode } from "react";

type GlassTint = "violet" | "red" | "green";

interface GlassPanelProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  /** Liquid Glass morph state (DESIGN.md's "Morph" section) — `thick` for
   * the app's large, persistent surfaces (dashboard card, onboarding/
   * recovery modals, the pill's wide states): more blur, deeper shadow,
   * stronger lensing. Omit (thin, the default) for small/narrow surfaces —
   * the sidebar rail, the pill's narrow `done` state. */
  thick?: boolean;
  /** Adaptive tint wash (DESIGN.md's unified palette) — omit for idle/
   * neutral glass. */
  tint?: GlassTint;
}

/** Applies the `.glass-panel` Liquid Glass treatment (globals.css) —
 * lensing rim, adaptive tint, and morph, shared by every glass surface in
 * the app (dashboard card + sidebar, pill, onboarding, recovery) so the
 * material only needs to change in one place. */
export function GlassPanel({
  children,
  className = "",
  thick,
  tint,
  ...rest
}: GlassPanelProps) {
  const classes = [
    "glass-panel",
    thick && "glass-panel--thick",
    tint && `glass-panel--tint-${tint}`,
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={classes} {...rest}>
      {children}
    </div>
  );
}
