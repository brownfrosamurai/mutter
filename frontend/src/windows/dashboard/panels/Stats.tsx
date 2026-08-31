import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/bindings";
import { StatTile } from "@/components/StatTile";
import { ActivityChart } from "@/components/ActivityChart";
import { LanguageBar } from "@/components/LanguageBar";
import { LatencyTable } from "@/components/LatencyTable";

const ACTIVITY_WINDOW_DAYS = 14;

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
 * derive from data already on hand. */
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
  const languages = useQuery({
    queryKey: ["language-breakdown"],
    queryFn: async () => {
      const res = await commands.getLanguageBreakdown();
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
  // English only (2026-08-31, user-directed) — this app's v1 scope is
  // English-only per CLAUDE.md/mutter-project-plan.md Section 6/17; Whisper
  // still auto-detects and transcribes other languages if dictated, but the
  // Languages breakdown only ever needs to report on the one it's scoped to.
  const englishOnly = languages.data?.filter((l) => l.language === "en") ?? [];
  const maxLanguageCount = englishOnly.length
    ? Math.max(...englishOnly.map((l) => l.count))
    : 1;

  return (
    <div className="flex flex-col gap-5">
      <div className="grid grid-cols-3 gap-4">
        <StatTile
          label="Transcriptions"
          value={metrics.data ? String(metrics.data.sessions) : "—"}
          sub={metrics.data ? `${metrics.data.sessions} sessions total` : ""}
        />
        <StatTile
          label="Words"
          value={metrics.data ? metrics.data.words.toLocaleString() : "—"}
          sub={metrics.data ? `${Math.round(metrics.data.average_wpm)} avg wpm` : ""}
        />
        <StatTile
          label="Time Spoken"
          value={metrics.data ? formatMinutes(metrics.data.total_dictation_minutes) : "—"}
          sub={streak > 0 ? `${streak} day streak 🔥` : ""}
        />
      </div>

      <section>
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-medium text-text-primary">Activity</h2>
          <span className="text-xs text-text-secondary">
            {activity.data?.length ?? 0} sessions · last {ACTIVITY_WINDOW_DAYS} days
          </span>
        </div>
        <ActivityChart days={ACTIVITY_WINDOW_DAYS} activity={activity.data ?? []} />
      </section>

      <section>
        <h2 className="mb-2 text-sm font-medium text-text-primary">Languages</h2>
        <div className="flex flex-col gap-2">
          {englishOnly.length ? (
            englishOnly.map((l) => (
              <LanguageBar
                key={l.language}
                language={l.language}
                count={l.count}
                averageWpm={l.average_wpm}
                fraction={l.count / maxLanguageCount}
              />
            ))
          ) : (
            <p className="text-sm text-text-secondary">No dictations yet.</p>
          )}
        </div>
      </section>

      <section>
        <h2 className="mb-2 text-sm font-medium text-text-primary">Latency</h2>
        {latency.data && <LatencyTable stats={latency.data} />}
      </section>
    </div>
  );
}
