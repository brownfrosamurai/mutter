import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands } from "@/lib/bindings";

type PillStateName = "loading" | "listening" | "canceling" | "done";

// Hand-written twins of session.rs's PillState<'a>/Elapsed/CancelCountdown
// event payload structs (mutter://pill-state, mutter://elapsed-seconds,
// mutter://cancel-countdown) — deliberately NOT run through tauri-specta's
// Event derive: those structs are tiny, stable, and PillState<'a> borrows
// a &str, which doesn't play well with the Event macro's 'static-leaning
// expectations. Commands (the actual evolving IPC surface) get full
// type-safety via bindings.ts; this is a small, low-churn exception.
interface PillStatePayload {
  state: string;
}
interface ElapsedPayload {
  seconds: number;
}
interface CancelCountdownPayload {
  secondsRemaining: number;
}

const STATUS_TEXT: Record<PillStateName, string> = {
  loading: "Warming up engine…",
  listening: "Listening…",
  canceling: "Stopping recording",
  done: "Done",
};

// Liquid Glass morph/tint per state (DESIGN.md's "Material system" —
// unifies the pill's palette with the dashboard's instead of the two
// surfaces running separate color philosophies). `loading` and `listening`
// are the pill's wide states (two lines of text; waveform + timer +
// controls) so they morph "thick"; `canceling` also runs wide (countdown
// numeral); `done` is narrow, so it stays "thin" — deeper shadow/blur is
// reserved for the states that are actually visually larger.
const PILL_TINT: Record<PillStateName, "violet" | "red" | "green" | undefined> = {
  loading: undefined,
  listening: "violet",
  canceling: "red",
  done: "green",
};
const PILL_THICK: Record<PillStateName, boolean> = {
  loading: true,
  listening: true,
  canceling: true,
  done: false,
};

const WAVEFORM_BAR_COUNT = 5;
// Static fallback heights for prefers-reduced-motion — still reads as a
// waveform shape, just frozen, not flat bars (see the pre-rewrite pill's
// own accessibility fix this preserves).
const STATIC_BAR_SCALES = [0.4, 0.7, 1, 0.7, 0.4];

function formatElapsed(totalSeconds: number): string {
  const m = Math.floor(totalSeconds / 60);
  const s = totalSeconds % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function Pill() {
  const [state, setState] = useState<PillStateName>("loading");
  const [elapsed, setElapsed] = useState(0);
  const [countdown, setCountdown] = useState(0);

  const pillRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const unlistenState = listen<PillStatePayload>("mutter://pill-state", (e) => {
      const s = e.payload.state;
      if (s === "loading" || s === "listening" || s === "canceling" || s === "done") {
        setState(s);
      }
    });
    const unlistenElapsed = listen<ElapsedPayload>("mutter://elapsed-seconds", (e) => {
      setElapsed(e.payload.seconds);
    });
    const unlistenCountdown = listen<CancelCountdownPayload>("mutter://cancel-countdown", (e) => {
      setCountdown(e.payload.secondsRemaining);
    });

    return () => {
      void unlistenState.then((f) => f());
      void unlistenElapsed.then((f) => f());
      void unlistenCountdown.then((f) => f());
    };
  }, []);

  // Reports #pill's real rendered width to Rust so the native vibrancy
  // layer resizes/masks itself to exactly that shape — see
  // session::apply_pill_layout's docs (src-tauri) for why this is the
  // one piece of native-vibrancy machinery a React rewrite still has to
  // replicate. Debounced via a single requestAnimationFrame guard,
  // mirroring the original vanilla JS exactly — this is what the
  // pre-rewrite pill needed to fix a real double-fire compositing-seam
  // bug (ResizeObserver's spec-guaranteed initial callback firing
  // alongside an explicit startup call), and it's real defense here too
  // regardless of StrictMode being off for this window (see main.tsx).
  useEffect(() => {
    const el = pillRef.current;
    if (!el) return;

    const report = () => {
      if (rafRef.current !== null) return;
      rafRef.current = requestAnimationFrame(() => {
        rafRef.current = null;
        const rect = el.getBoundingClientRect();
        void commands.setPillVibrancyLayout({
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
        });
      });
    };

    const observer = new ResizeObserver(report);
    observer.observe(el);
    report();

    return () => {
      observer.disconnect();
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
    };
  }, []);

  const showWaveform = state === "listening";
  // One consistent status-dot language across every active state (preview's
  // exact treatment — listening/canceling/done all read via the same dot +
  // glow, not a one-off mic icon reserved for listening alone). `loading`
  // has no dot — the preview never depicted that state.
  const showDot = state === "listening" || state === "canceling" || state === "done";
  const showText = state !== "listening";
  const dotColor =
    state === "canceling" ? "var(--danger)" : state === "done" ? "var(--success)" : "var(--accent-violet)";
  // Soft glow halo, matching the design-consultation preview's dot treatment
  // exactly (0 0 8px at 90% alpha) — a state-colored light source, not just
  // a flat marker.
  const dotGlow =
    state === "canceling"
      ? "rgba(255, 69, 58, 0.9)"
      : state === "done"
        ? "rgba(48, 209, 88, 0.9)"
        : "rgba(139, 124, 246, 0.9)";

  const pillClasses = [
    "glass-panel",
    PILL_THICK[state] && "glass-panel--thick",
    PILL_TINT[state] && `glass-panel--tint-${PILL_TINT[state]}`,
    "inline-flex h-9 items-center gap-2 whitespace-nowrap rounded-pill px-4 text-text-primary",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      ref={pillRef}
      id="pill"
      data-tauri-drag-region
      data-state={state}
      className={pillClasses}
    >
      {showDot && (
        <span
          aria-hidden="true"
          className="h-2 w-2 shrink-0 rounded-full"
          style={{ backgroundColor: dotColor, boxShadow: `0 0 8px ${dotGlow}` }}
        />
      )}

      {showWaveform && (
        <div className="flex items-center gap-0.5" aria-hidden="true">
          {Array.from({ length: WAVEFORM_BAR_COUNT }).map((_, i) => (
            <span
              key={i}
              className="w-0.5 rounded-full bg-accent-violet"
              style={{
                height: "12px",
                animation: `mutter-pill-wave 900ms ease-in-out ${i * 90}ms infinite`,
                transform: `scaleY(${STATIC_BAR_SCALES[i]})`,
              }}
            />
          ))}
        </div>
      )}

      {showText && <span className="text-base">{STATUS_TEXT[state]}</span>}

      {state === "loading" && (
        <span className="text-xs text-text-secondary">
          Initial lazy-load latency (once per session)
        </span>
      )}

      {state === "listening" && <span className="font-mono text-base">{formatElapsed(elapsed)}</span>}

      {state === "canceling" && <span className="font-mono text-base">{countdown}</span>}

      {state === "listening" && (
        <div className="flex items-center gap-1">
          <button
            type="button"
            aria-label="Pause (not yet implemented)"
            className="flex h-6 w-6 items-center justify-center rounded-full bg-surface-control text-text-primary transition-colors duration-fast hover:bg-surface-control-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-violet"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
              <rect x="1" y="0" width="3" height="10" fill="currentColor" />
              <rect x="6" y="0" width="3" height="10" fill="currentColor" />
            </svg>
          </button>
          <button
            type="button"
            aria-label="Cancel"
            onClick={() => void commands.cancelRecording()}
            className="flex h-6 w-6 items-center justify-center rounded-full bg-surface-control text-text-primary transition-colors duration-fast hover:bg-surface-control-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-violet"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
              <path d="M1 1l8 8M9 1l-8 8" stroke="currentColor" strokeWidth="1.5" />
            </svg>
          </button>
        </div>
      )}
    </div>
  );
}
