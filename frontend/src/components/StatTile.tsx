interface StatTileProps {
  label: string;
  value: string;
  sub: string;
}

/** One Stats-page tile: small-caps label, large number, small caption
 * below (DESIGN.md typography: stat numbers are a raw 18px, not tokenized).
 * The number itself uses `font-rounded` (real SF Pro Rounded via WKWebView's
 * `ui-rounded` generic family) + tabular-nums — the same rounded-numeral
 * treatment Apple's own glanceable-data widgets (Weather, Fitness) use,
 * added in the Liquid Glass pass; body/label text stays the standard
 * system stack. */
export function StatTile({ label, value, sub }: StatTileProps) {
  return (
    <div>
      <div className="text-xs uppercase tracking-wide text-text-secondary">{label}</div>
      <div className="mt-1 font-rounded text-[17px] leading-none tracking-tight text-text-primary [font-variant-numeric:tabular-nums] [font-weight:650]">
        {value}
      </div>
      <div className="mt-1 text-xs text-text-secondary">{sub}</div>
    </div>
  );
}
