interface StatTileProps {
  label: string;
  value: string;
  sub: string;
}

/** One Stats-page tile: small-caps label, large number, small caption
 * below (DESIGN.md typography: stat numbers are a raw 22px, not tokenized). */
export function StatTile({ label, value, sub }: StatTileProps) {
  return (
    <div>
      <div className="text-xs uppercase tracking-wide text-text-secondary">{label}</div>
      <div className="mt-1 text-[18px] font-semibold leading-none text-text-primary">{value}</div>
      <div className="mt-1 text-xs text-text-secondary">{sub}</div>
    </div>
  );
}
