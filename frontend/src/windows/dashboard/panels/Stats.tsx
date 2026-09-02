import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/bindings";
import { StatTile } from "@/components/StatTile";
import { ActivityChart } from "@/components/ActivityChart";
import { LatencyTable } from "@/components/LatencyTable";

const ACTIVITY_WINDOW_DAYS = 7;

function formatMinutes(min: number): string {
  const totalSeconds = Math.round(min * 60);
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** Consecutive days (including today) with at least one session — computed
 * client-side from the same daily_activity data the chart already fetches,
 * not a fabricated number or a new backend aggregate for what's cheap to
 * derive from data already on hand. Surfaced in the Activity section's own
 * meta line (not a stat tile — the design-consultation preview's 4-tile
 * grid has no room for it, see Stats panel's 2026-09-01 restructure). */
function computeStreak(activity: { date: string; count: number }[]): number {
  const byDate = new Map(activity.map((a) => [a.date, a.count]));
  let streak = 0;
  const cursor = new Date();
  for (;;) {
    const y = cursor.getFullYear();
    const m = String(cursor.getMonth() + 1).padStart(2, "0");
    const d = String(cursor.getDate()).padStart(2, "0");
    const key = `${y}-${m}-${d}`;
    if ((byDate.get(key) ?? 0) > 0) {
      streak += 1;
      cursor.setDate(cursor.getDate() - 1);
    } else {
      break;
    }
  }
  return streak;
}

export function StatsPanel() {
  const metrics = useQuery({
    queryKey: ["metrics"],
    queryFn: async () => {
      const res = await commands.getMetrics();
      if (res.status === "error") throw new Error(res.error);
      return res.data;
    },
  });
  const activity = useQuery({
    queryKey: ["daily-activity", ACTIVITY_WINDOW_DAYS],
    queryFn: async () => {
      const res = await commands.getDailyActivity(ACTIVITY_WINDOW_DAYS);
      if (res.status === "error") throw new Error(res.error);
      return res.data;
    },
  });
  const latency = useQuery({
    queryKey: ["latency-stats"],
    queryFn: async () => {
      const res = await commands.getLatencyStats();
      if (res.status === "error") throw new Error(res.error);
      return res.data;
    },
  });
  const streak = activity.data ? computeStreak(activity.data) : 0;

  return (
    <div className="flex h-full flex-col justify-between gap-4">
      {/* 4-tile grid matching the design-consultation preview's Sessions/
          Words/WPM/Saved layout exactly (2026-09-01, user-directed) — "Saved"
          uses metrics.time_saved_minutes, a real backend field that existed
          but was never wired into any panel before this. No subtext under
          tiles, matching the preview's clean look; the streak that used to
          live in "Time Spoken"'s subtext moved to the Activity section's
          meta line below instead of being dropped outright. */}
      <div className="grid grid-cols-4 gap-3">
        <StatTile
          label="Sessions"
          value={metrics.data ? String(metrics.data.sessions) : "—"}
        />
        <StatTile
          label="Words"
          value={metrics.data ? metrics.data.words.toLocaleString() : "—"}
        />
        <StatTile
          label="WPM"
          value={
            metrics.data ? String(Math.round(metrics.data.average_wpm)) : "—"
          }
        />
        <StatTile
          label="Saved"
          value={
            metrics.data ? formatMinutes(metrics.data.time_saved_minutes) : "—"
          }
        />
      </div>

      <section>
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-medium text-text-primary">Activity</h2>
          <span className="text-xs text-text-secondary">
            {activity.data?.length ?? 0} sessions · last {ACTIVITY_WINDOW_DAYS}{" "}
            days
            {streak > 0 ? ` · ${streak} day streak 🔥` : ""}
          </span>
        </div>
        <ActivityChart
          days={ACTIVITY_WINDOW_DAYS}
          activity={activity.data ?? []}
        />
      </section>

      <section>
        <h2 className="mb-2 text-sm font-medium text-text-primary">Latency</h2>
        {latency.data && <LatencyTable stats={latency.data} />}
      </section>
    </div>
  );
}
