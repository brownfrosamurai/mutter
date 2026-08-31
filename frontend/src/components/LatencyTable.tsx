import type { LatencyStatsDto } from "@/lib/bindings";

interface LatencyTableProps {
  stats: LatencyStatsDto;
}

function fmt(ms: number | null): string {
  if (ms === null) return "—";
  return ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(1)} s`;
}

/** Real, backend-computed p50/p95/sample-count per stage — see
 * `HistoryStore::latency_stats`. Deliberately shows "—" for a stage with
 * zero samples rather than 0ms, so it never looks like a suspiciously-fast
 * real measurement. */
export function LatencyTable({ stats }: LatencyTableProps) {
  const rows = [
    { label: "Recording Latency", percentiles: stats.recording },
    { label: "Inference (Whisper)", percentiles: stats.inference },
  ];

  return (
    <div className="overflow-hidden rounded-small border border-glass-border">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-glass-border bg-surface-inset text-xs text-text-secondary">
            <th className="px-3 py-2 text-left font-medium">Stage</th>
            <th className="px-3 py-2 text-right font-medium">p50</th>
            <th className="px-3 py-2 text-right font-medium">p95</th>
            <th className="px-3 py-2 text-right font-medium">Samples</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ label, percentiles }) => (
            <tr key={label} className="border-b border-glass-border last:border-b-0">
              <td className="px-3 py-2 text-text-primary">{label}</td>
              <td className="px-3 py-2 text-right text-text-primary">{fmt(percentiles.p50_ms)}</td>
              <td className="px-3 py-2 text-right text-text-primary">{fmt(percentiles.p95_ms)}</td>
              <td className="px-3 py-2 text-right text-text-secondary">{percentiles.samples}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
