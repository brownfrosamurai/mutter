import { useId, useLayoutEffect, useRef, type CSSProperties } from "react";
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

interface Point {
  x: number;
  y: number;
}

/** Catmull-Rom → cubic-Bezier conversion (uniform, tension 1) — the standard
 * way to draw one smooth curve through a set of points without per-segment
 * overshoot. Endpoints are clamped by reusing the first/last point as their
 * own neighbor. */
function smoothPath(points: Point[]): string {
  if (points.length < 2) return "";
  const d = [`M ${points[0].x} ${points[0].y}`];
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[i === 0 ? 0 : i - 1];
    const p1 = points[i];
    const p2 = points[i + 1];
    const p3 = points[i + 2 < points.length ? i + 2 : points.length - 1];
    const c1x = p1.x + (p2.x - p0.x) / 6;
    const c1y = p1.y + (p2.y - p0.y) / 6;
    const c2x = p2.x - (p3.x - p1.x) / 6;
    const c2y = p2.y - (p3.y - p1.y) / 6;
    d.push(`C ${c1x} ${c1y} ${c2x} ${c2y} ${p2.x} ${p2.y}`);
  }
  return d.join(" ");
}

const TOP_PAD = 0.14;
const BOTTOM_PAD = 0.1;
const FLOOR_SCALE = 3 / 40; // same visual floor the old bar chart used for empty days
// Width in viewBox y-units (viewBox height is exactly 1 unit == the 40px
// track) — deliberately NOT `vector-effect="non-scaling-stroke"`: that
// reinterprets stroke-dasharray/-dashoffset in screen pixels instead of the
// path's own user-space units, which breaks the getTotalLength()-based
// draw-in animation below (live-verified 2026-09-02 — see the component doc
// comment). The line is mostly near-horizontal, so on-screen thickness is
// dominated by this value times the y-axis scale, not the x-axis scale;
// the viewBox's non-uniform x/y stretch (from `preserveAspectRatio="none"`
// on a wide-and-short container) only shows up as a faint thickness
// variation on the steepest part of the curve, not a functional bug.
const STROKE_WIDTH = 0.035;

/** DESIGN.md's Activity chart spec (2026-09-02 wave revision): one smooth
 * spline through the week's counts instead of discrete equal-width bars —
 * ties the dashboard's data viz back to the product's voice/waveform
 * identity without literally reusing the pill's own listening waveform
 * (`Pill.tsx`'s `.pill-waveform`), which represents something different
 * (live audio, not historical counts). Today is still marked via bold
 * weekday label + dot (not recolored), plus a small glowing point on the
 * curve itself — rendered as a plain HTML overlay div (not an SVG
 * `<circle>`), since a circle inside this SVG's non-uniformly scaled
 * viewBox (`preserveAspectRatio="none"` on a wide/short container) would
 * render as a visibly squashed ellipse, not a circle. Draw-in animates
 * `stroke-dashoffset` from the line's real measured length (via
 * `getTotalLength()`) down to 0, so the wave draws in as one continuous
 * stroke, not a per-point stagger — matching this app's established rule
 * that a material's parts should read as one motion, not several
 * (DESIGN.md's Morph section). SVG's `pathLength` attribute would do this
 * without a measurement effect, but combined with this same non-uniform
 * scale it renders as a genuinely dashed line in WebKit (live-verified
 * 2026-09-02, not assumed) — `getTotalLength()` is the traditional,
 * engine-independent version of the same technique and doesn't hit that
 * bug. The stroke draw, fill fade, and dot pop are all
 * opacity/stroke-dashoffset/transform only — no layout-affecting property,
 * same compositor-only discipline the old per-bar `scaleY` animation
 * followed. */
export function ActivityChart({ days, activity }: ActivityChartProps) {
  const gradientId = `mutter-wave-fill-${useId().replace(/:/g, "")}`;
  const lineRef = useRef<SVGPathElement>(null);
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
  const todayIndex = dayAxis.findIndex((d) => d.isToday);

  const points: Point[] = dayAxis.map((d, i) => {
    const scale = d.count > 0 ? Math.max(d.count / max, FLOOR_SCALE) : FLOOR_SCALE;
    return {
      x: i + 0.5,
      y: TOP_PAD + (1 - scale) * (1 - TOP_PAD - BOTTOM_PAD),
    };
  });
  const linePath = smoothPath(points);
  const first = points[0];
  const last = points[points.length - 1];
  const areaPath = `${linePath} L ${last.x} 1 L ${first.x} 1 Z`;
  const todayPoint = points[todayIndex];

  useLayoutEffect(() => {
    const path = lineRef.current;
    if (!path) return;
    const length = path.getTotalLength();
    path.style.transition = "none";
    path.style.strokeDasharray = `${length}`;
    path.style.strokeDashoffset = `${length}`;
    path.getBoundingClientRect(); // force layout so the transition below isn't coalesced away
    requestAnimationFrame(() => {
      path.style.transition = "stroke-dashoffset 560ms var(--ease-standard)";
      path.style.strokeDashoffset = "0";
    });
  }, [linePath]);

  return (
    <div className="flex flex-col gap-1">
      <div className="relative h-10 w-full">
        <svg
          viewBox={`0 0 ${days} 1`}
          preserveAspectRatio="none"
          className="h-full w-full"
          aria-hidden="true"
        >
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="rgb(139, 124, 246)" stopOpacity="0.32" />
              <stop offset="100%" stopColor="rgb(139, 124, 246)" stopOpacity="0" />
            </linearGradient>
          </defs>
          <path
            d={areaPath}
            fill={`url(#${gradientId})`}
            stroke="none"
            style={
              {
                opacity: 0,
                animation: "mutter-wave-fill 420ms var(--ease-standard) 260ms forwards",
              } as CSSProperties
            }
          />
          <path
            ref={lineRef}
            d={linePath}
            fill="none"
            stroke="var(--surface-filled)"
            strokeWidth={STROKE_WIDTH}
            strokeLinecap="round"
          />
        </svg>
        {todayPoint && (
          <div
            aria-hidden="true"
            className="absolute h-1.5 w-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-text-primary shadow-[0_0_0_4px_var(--surface-filled)]"
            style={
              {
                left: `${(todayPoint.x / days) * 100}%`,
                top: `${todayPoint.y * 100}%`,
                opacity: 0,
                animation: "mutter-wave-dot 220ms var(--ease-standard) 520ms forwards",
              } as CSSProperties
            }
          />
        )}
      </div>
      <div className="flex">
        {dayAxis.map(({ date, isToday }, i) => {
          const weekday = new Date(`${date}T00:00:00`).getDay();
          return (
            <div key={date} className="flex flex-1 flex-col items-center gap-0.5">
              <span
                className={`text-[9px] text-text-secondary ${isToday ? "font-semibold text-text-primary" : ""}`}
              >
                {WEEKDAY_INITIALS[weekday]}
              </span>
              {isToday && (
                <span aria-hidden="true" className="h-1 w-1 rounded-full bg-text-primary" />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
