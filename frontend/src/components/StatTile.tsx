interface StatTileProps {
  label: string;
  value: string;
  /** Optional small caption below the value. Omitted entirely by the Metrics
   * panel's 4-tile grid (matching the design-consultation preview's clean,
   * caption-free tiles) — kept optional rather than deleted since a future
   * tile might still want one. */
  sub?: string;
}

/** One Stats-page tile: small-caps label, large number, optional small
 * caption below (DESIGN.md typography: stat numbers are 17px, not
 * tokenized). The number itself uses `font-rounded` (real SF Pro Rounded via
 * WKWebView's `ui-rounded` generic family) + tabular-nums — the same
 * rounded-numeral treatment Apple's own glanceable-data widgets (Weather,
 * Fitness) use, added in the Liquid Glass pass; body/label text stays the
 * standard system stack.
 *
 * Each tile is its own small card (2026-09-01, user-directed) — the
 * design-consultation preview's `.stat-tile` always had a background/
 * padding/radius treatment (`rgba(255,255,255,0.05)`, `--space-2`,
 * `--radius-small`, centered) that never actually made it into this
 * component; tiles floated as plain text in a grid instead. Matches now via
 * `--surface-inset`, already tuned close to that exact alpha. */
export function StatTile({ label, value, sub }: StatTileProps) {
  return (
    <div className="rounded-small bg-surface-inset p-2 text-center">
      <div className="text-xs uppercase tracking-wide text-text-secondary">{label}</div>
      <div className="mt-1 font-rounded text-[17px] leading-none tracking-tight text-text-primary [font-variant-numeric:tabular-nums] [font-weight:650]">
        {value}
      </div>
      {sub && <div className="mt-1 text-xs text-text-secondary">{sub}</div>}
    </div>
  );
}
