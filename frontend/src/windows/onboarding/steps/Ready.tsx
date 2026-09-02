import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Mic, Monitor } from "lucide-react";
import { commands } from "@/lib/bindings";
import { toSymbols } from "@/lib/hotkey";
import {
  PermissionRow,
  TO_PERMISSION_KIND,
  canActivelyRequest,
  type DisplayStatus,
  type PermissionRowKind,
} from "@/components/PermissionRow";

const KINDS: readonly PermissionRowKind[] = ["mic", "accessibility", "system_audio"];

/** Whether a kind should be skipped entirely (real re-entry guard, design
 * review 2026-09-01, cross-model finding: a component-local `useRef`
 * one-shot flag doesn't survive a real unmount, which is exactly what
 * happens on Back→Continue since Ready.tsx stops rendering entirely when
 * the user navigates away and a fresh instance mounts on return — backed
 * by real OS/TCC state via `getPermissionStatus()` instead, which
 * persists across remounts because it isn't component state at all).
 *
 * MUST reuse `canActivelyRequest`'s exact kind-specific logic, not a
 * blanket "denied is terminal" check — a real live-verified bug (2026-09-01)
 * came from this effect originally having its own separate blanket guard:
 * Accessibility/Screen Recording both read `denied` on a fresh install
 * before ever being asked (their `refresh()` has no `not_requested` state),
 * so a blanket guard skipped requesting them entirely on the very first
 * launch — silently reproducing cross-model finding #4's bug in a second
 * place. `unavailable` is skipped regardless of kind, matching
 * `PermissionRow`'s own `showButton` gate (`mode === "action" && !granted && !unavailable`) that
 * the old click-driven path always enforced before this auto-fire path
 * existed. */
function shouldSkip(kind: PermissionRowKind, status: string): boolean {
  return status === "unavailable" || !canActivelyRequest(kind, status);
}

/** Step 1 (was step 2 before the 2026-09-01 collapse to 2 steps) — two
 * phases:
 *
 * **In-flight** ("Setting things up"): on mount, a one-shot-guarded effect
 * (the `useRef` below is defense-in-depth against a genuine second mount
 * racing this one — see `onBusyChange`'s doc below for why that's the real
 * risk now that `onboarding/main.tsx` deliberately doesn't wrap the app in
 * `<StrictMode>`; `shouldSkip`'s real re-entry guard above handles the
 * separate cross-remount case) sequentially awaits
 * `commands.requestPermission(kind)` for mic → accessibility →
 * screen_recording, skipping any kind `shouldSkip` flags. Each row shows
 * its live status via `PermissionRow`'s `mode="status"` — `queued` for
 * kinds not yet reached, `requesting` for the one in flight, then its real
 * resolved status. Each kind's request+refresh pair has its own try/catch
 * (pre-landing review, 2026-09-01, red-team finding) so a Result-shaped
 * error OR a thrown/rejected exception (bindings.ts's generated wrappers
 * re-throw real `Error` instances on IPC transport failures, not just
 * `{status:"error"}`) on one kind is logged and never blocks requesting the
 * remaining two — a bare exception previously propagated out of the whole
 * loop, silently aborting every kind not yet reached.
 *
 * **Resolved** ("You're all set"): once every kind has settled (granted,
 * denied, or unavailable — never gated on approval), the hotkey rows
 * (existing `getSettings()`-backed display) become visible alongside the
 * now-compact permission rows. This ordering exists specifically so the
 * screen never claims "all set" at the same moment it's about to interrupt
 * the user with 3 OS security prompts (design review finding: the old
 * single-phase "You're all set" heading was a closing statement contradicted
 * by what happened the instant the screen painted).
 *
 * `onBusyChange` (pre-landing review, 2026-09-01, red-team finding): reports
 * whether the request sequence is still in flight so `Onboarding.tsx` can
 * refuse to navigate away from this step until it settles. Two real bugs
 * this closes: (1) `Onboarding.tsx`'s "Open Dashboard"/Continue button had
 * no idea this sequence was running and could close the whole onboarding
 * *window* (`complete_onboarding`'s `win.close()`, not hide) while a
 * Rust-side `spawn_blocking` task was still mid-native-call — the task
 * isn't tied to the webview's lifetime, so e.g. Accessibility's "trust this
 * app" alert could pop up after the window explaining it was already gone.
 * (2) Rapid Back→Continue could unmount this component mid-sequence and
 * mount a fresh instance, whose own effect starts an independent second
 * request sequence — `startedRef` only guards React StrictMode's
 * double-invoke *within* one mount, not a genuine second mount — risking
 * two concurrent calls for the same kind. Disabling both Back and
 * Continue/Open-Dashboard while busy prevents the unmount that both bugs
 * depend on, without needing to actually cancel an in-flight native FFI
 * call from JS (not practical once the IPC message has been sent). */
export function Ready({ onBusyChange }: { onBusyChange?: (busy: boolean) => void }) {
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: () => commands.getSettings(),
  });
  const [rowStatus, setRowStatus] = useState<Record<PermissionRowKind, DisplayStatus>>({
    mic: "queued",
    accessibility: "queued",
    system_audio: "queued",
  });
  const [phase, setPhase] = useState<"in-flight" | "resolved">("in-flight");
  const startedRef = useRef(false);

  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;
    onBusyChange?.(true);

    // Defensive unmount guard (pre-landing review, 2026-09-01, adversarial
    // finding): the nav-disable-while-busy guard in `Onboarding.tsx` is what
    // actually prevents this component from unmounting mid-sequence today,
    // but nothing enforces that invariant at this level — a future change
    // to that gating (or the window being force-closed at the OS level
    // while a request is in flight, a gap already tracked in TODOS.md)
    // could otherwise call `setRowStatus`/`setPhase` on an unmounted
    // component. Checked before every state update below.
    let cancelled = false;

    async function run() {
      try {
        const current = await commands.getPermissionStatus();
        const initial = {} as Record<PermissionRowKind, DisplayStatus>;
        const skip = {} as Record<PermissionRowKind, boolean>;
        for (const kind of KINDS) {
          const real = current[kind] as DisplayStatus;
          skip[kind] = shouldSkip(kind, real);
          initial[kind] = skip[kind] ? real : "queued";
        }
        if (cancelled) return;
        setRowStatus(initial);

        for (const kind of KINDS) {
          if (skip[kind]) continue;

          if (cancelled) return;
          setRowStatus((prev) => ({ ...prev, [kind]: "requesting" }));
          // Per-kind try/catch (pre-landing review, 2026-09-01, red-team
          // finding): requestPermission()/getPermissionStatus() can reject
          // (not just resolve to a Result error) on a real IPC transport
          // failure — bindings.ts's generated wrappers re-throw actual
          // `Error` instances rather than converting them to
          // `{status:"error"}`. Without this, a single kind's thrown
          // exception propagated out of the whole loop, silently aborting
          // every kind not yet reached — not just the one that failed.
          try {
            const res = await commands.requestPermission(TO_PERMISSION_KIND[kind]);
            if (res.status === "error") {
              console.error(`request_permission(${kind}) failed:`, res.error);
            }
            const refreshed = await commands.getPermissionStatus();
            if (cancelled) return;
            setRowStatus((prev) => ({ ...prev, [kind]: refreshed[kind] as DisplayStatus }));
          } catch (e) {
            console.error(`request_permission(${kind}) threw unexpectedly:`, e);
          }
        }
      } catch (e) {
        // getPermissionStatus() (the initial call above the loop) can also
        // reject on a real IPC transport failure. Without this catch, that
        // rejection would propagate out of run() unhandled and
        // setPhase("resolved")/onBusyChange(false) below would never run,
        // leaving the UI stuck on "Setting things up" forever (pre-landing
        // review, 2026-09-01, red-team finding). Whatever kinds hadn't
        // resolved yet just keep their last-known row status; the user can
        // still reach the dashboard and grant permissions later from
        // Settings.
        console.error("onboarding permission sequence failed unexpectedly:", e);
      } finally {
        if (!cancelled) {
          setPhase("resolved");
          onBusyChange?.(false);
        }
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
  }, [onBusyChange]);

  return (
    <div>
      {phase === "in-flight" ? (
        <>
          <h1 className="text-lg font-semibold text-text-primary">Setting things up</h1>
          <p className="mt-2 text-sm text-text-secondary">
            Mutter needs a few permissions to hear you, type what you said, and (optionally)
            capture your screen for meeting notes. You'll see a few system prompts — respond to
            each one.
          </p>
          <div className="mt-4 flex flex-col gap-2">
            {KINDS.map((kind) => (
              <PermissionRow key={kind} kind={kind} status={rowStatus[kind]} mode="status" />
            ))}
          </div>
        </>
      ) : (
        <>
          <h1 className="text-lg font-semibold text-text-primary">You're all set</h1>
          <p className="mt-2 text-sm text-text-secondary">
            Press either hotkey to start dictating, press it again to stop. Both are configurable
            later in Settings.
          </p>
          <div className="mt-4 flex flex-col gap-2">
            <div className="flex items-center justify-between rounded-small bg-surface-inset px-3 py-2">
              <span className="flex items-center gap-2 text-sm text-text-primary">
                <Mic size={14} strokeWidth={2} />
                Mic Dictation
              </span>
              <span className="rounded-[5px] bg-[rgba(255,255,255,0.08)] px-2 py-1 font-mono text-sm text-text-primary">
                {settings.data ? toSymbols(settings.data.mic_hotkey) : "…"}
              </span>
            </div>
            <div className="flex items-center justify-between rounded-small bg-surface-inset px-3 py-2">
              <span className="flex items-center gap-2 text-sm text-text-primary">
                <Monitor size={14} strokeWidth={2} />
                System Audio
              </span>
              <span className="rounded-[5px] bg-[rgba(255,255,255,0.08)] px-2 py-1 font-mono text-sm text-text-primary">
                {settings.data ? toSymbols(settings.data.system_audio_hotkey) : "…"}
              </span>
            </div>
          </div>
          <div className="mt-2 flex flex-col gap-1.5">
            {KINDS.map((kind) => (
              <PermissionRow key={kind} kind={kind} status={rowStatus[kind]} mode="status" />
            ))}
          </div>
        </>
      )}
      <button
        type="button"
        onClick={() => void commands.quitApp()}
        // Disabled while a request is in flight (pre-landing review,
        // 2026-09-01, red-team finding): quitApp() -> app.exit(0) is
        // synchronous and immediate, and could otherwise tear down the
        // whole process while a spawn_blocking task is still mid-native-call
        // inside run_on_main_thread's exec_sync — the same class of risk
        // Back/Continue are already guarded against via onBusyChange/
        // navDisabled above.
        disabled={phase === "in-flight"}
        className="mt-4 rounded-small text-xs text-text-secondary transition-opacity duration-fast hover:text-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
      >
        Quit
      </button>
    </div>
  );
}
