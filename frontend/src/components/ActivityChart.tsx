import type { CSSProperties } from "react";
import type { DailyActivityDto } from "@/lib/bindings";

interface ActivityChartProps {
  days: number;
  activity: DailyActivityDto[];
}

const WEEKDAY_INITIALS = ["S", "M", "T", "W", "T", "F", "S"];

/** Local `YYYY-MM-DD`, matching the backend's `date(timestamp, 'unixepoch',
 * 'localtime')` bucketing exactly — never `toISOString()` (always UTC), the
 * exact bug this project already fixed once (2026-08-30). */
function toLocalDateString(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** DESIGN.md's Activity chart spec: equal-width bars, 40px track, empty day
 * = flat surface-track sliver (min-height 3px), active day = surface-filled,
 * today marked via bold label + dot (not recolored), entrance animates
 * `scaleY(0->1)` from bottom origin (never animated `height` — compositor-
 * only, matches this app's established layout-thrash-avoidance pattern). */
export function ActivityChart({ days, activity }: ActivityChartProps) {
  const byDate = new Map(activity.map((a) => [a.date, a.count]));
  const today = new Date();
  const dayAxis: { date: string; count: number; isToday: boolean }[] = [];
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today);
    d.setDate(d.getDate() - i);
    const date = toLocalDateString(d);
    dayAxis.push({ date, count: byDate.get(date) ?? 0, isToday: i === 0 });
  }
  const max = Math.max(1, ...dayAxis.map((d) => d.count));

  return (
    <div className="flex h-10 items-end gap-1">
      {dayAxis.map(({ date, count, isToday }, i) => {
        const weekday = new Date(`${date}T00:00:00`).getDay();
        const scale = count > 0 ? Math.max(count / max, 3 / 40) : 3 / 40;
        return (
          <div key={date} className="flex flex-1 flex-col items-center gap-1">
            <div className="relative h-8 w-full">
              <div
                className="absolute bottom-0 w-full origin-bottom rounded-sm"
                style={
                  {
                    height: "100%",
                    "--bar-scale": scale,
                    backgroundColor: count > 0 ? "var(--surface-filled)" : "var(--surface-track)",
                    animation: `mutter-activity-grow ${200 + i * 20}ms var(--ease-standard) both`,
                  } as CSSProperties
                }
              />
            </div>
            <span
              className={`text-[9px] text-text-secondary ${isToday ? "font-semibold text-text-primary" : ""}`}
            >
              {WEEKDAY_INITIALS[weekday]}
            </span>
            {isToday && <span aria-hidden="true" className="h-1 w-1 -mt-0.5 rounded-full bg-text-primary" />}
          </div>
        );
      })}
    </div>
  );
}
