import { useState } from "react";
import { GlassPanel } from "@/components/GlassPanel";
import { commands } from "@/lib/bindings";
import { Welcome } from "./steps/Welcome";
import { Ready } from "./steps/Ready";

const STEPS = ["welcome", "ready"] as const;

/** First-run onboarding shell (docs/designs/onboarding-flow-plan.md) —
 * 2 steps (Welcome -> Ready, collapsed from 3 on 2026-09-01 — permissions
 * now auto-fire on Ready's mount instead of a dedicated button-driven
 * step), progress dots, back/skip/continue nav. Shown once by `lib.rs`'s
 * `setup()` when `AppSettings.onboarding_completed` is false;
 * `completeOnboarding` persists the flag and hands off to the dashboard.
 *
 * `data-tauri-drag-region="deep"` on the panel below makes the window
 * draggable everywhere except real interactive elements — verified against
 * Tauri's actual drag.js: BUTTON/A/INPUT/etc. are excluded by tag name
 * regardless of an ancestor's drag attribute, so this needs no dead-space
 * layout carving the way a bare (non-"deep") drag region would. Required
 * now that this window is chromeless (`decorations: false`, migrated
 * 2026-09-01 alongside Recovery onto the same `apply_glass_shell`
 * mechanism Dashboard/Pill already used) — it previously relied on the
 * native title bar for dragging. */
export function Onboarding() {
  const [stepIndex, setStepIndex] = useState(0);
  const [finishing, setFinishing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Whether Ready's permission-request sequence is still in flight — see
  // Ready.tsx's `onBusyChange` module doc (pre-landing review, 2026-09-01,
  // red-team finding) for why navigation must be blocked while true.
  const [readyBusy, setReadyBusy] = useState(false);
  const step = STEPS[stepIndex];
  const isLastStep = stepIndex === STEPS.length - 1;
  const navDisabled = finishing || (step === "ready" && readyBusy);

  async function finish() {
    setError(null);
    setFinishing(true);
    try {
      const res = await commands.completeOnboarding();
      if (res.status === "error") {
        setError(res.error);
      }
    } finally {
      setFinishing(false);
    }
  }

  return (
    <div className="flex h-screen items-center justify-center p-4">
      <GlassPanel
        thick
        data-tauri-drag-region="deep"
        className="flex w-full max-w-md flex-col rounded-panel p-6"
      >
        <div className="mb-5 flex items-center justify-center gap-1.5" aria-hidden="true">
          {STEPS.map((s, i) => (
            <div
              key={s}
              className="h-[5px] w-[5px] rounded-full transition-colors duration-base ease-standard"
              style={{
                backgroundColor: i <= stepIndex ? "var(--surface-filled)" : "var(--surface-track)",
              }}
            />
          ))}
        </div>

        <div className="min-h-[180px]">
          {step === "welcome" && <Welcome />}
          {step === "ready" && <Ready onBusyChange={setReadyBusy} />}
        </div>

        {error && <div className="mt-2 text-xs text-text-primary opacity-80">{error}</div>}

        <div className="mt-6 flex items-center justify-between">
          <button
            type="button"
            onClick={() => setStepIndex((i) => Math.max(0, i - 1))}
            disabled={stepIndex === 0 || navDisabled}
            className="rounded-small text-xs text-text-secondary transition-opacity duration-fast hover:text-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-0"
          >
            Back
          </button>

          <div className="flex items-center gap-4">
            {!isLastStep && (
              <button
                type="button"
                onClick={() => void finish()}
                disabled={navDisabled}
                className="rounded-small text-xs text-text-secondary transition-colors duration-fast hover:text-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
              >
                Skip
              </button>
            )}
            <button
              type="button"
              onClick={() => (isLastStep ? void finish() : setStepIndex((i) => i + 1))}
              disabled={navDisabled}
              className="rounded-small bg-surface-filled px-4 py-1.5 text-sm font-medium text-white transition-opacity duration-fast hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
            >
              {isLastStep ? (finishing ? "Opening…" : "Open Dashboard") : "Continue"}
            </button>
          </div>
        </div>
      </GlassPanel>
    </div>
  );
}
