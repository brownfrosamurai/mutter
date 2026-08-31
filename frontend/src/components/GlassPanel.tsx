import type { HTMLAttributes, ReactNode } from "react";

interface GlassPanelProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
}

/** Applies the `.glass-panel` treatment (globals.css) — currently a no-op
 * marker class (no fill, no blur, no border) shared by the dashboard's
 * #app card, its sidebar pill, and recovery, kept as one shared class so
 * a future visual treatment only needs to change in one place. */
export function GlassPanel({ children, className = "", ...rest }: GlassPanelProps) {
  return (
    <div className={`glass-panel ${className}`} {...rest}>
      {children}
    </div>
  );
}
