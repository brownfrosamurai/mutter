import { useState } from "react";
import { commands, type PermissionKind, type Result } from "@/lib/bindings";

export type PermissionRowKind = "mic" | "accessibility" | "system_audio";

const TITLE: Record<PermissionRowKind, string> = {
  mic: "Microphone",
  accessibility: "Accessibility",
  system_audio: "Screen Recording",
};

/** `PermissionRowKind` (matches `PermissionStatusDto`'s field names) ->
 * the Rust `PermissionKind` wire enum (`openPermissionSettings`/
 * `requestPermission`'s target). Two different shapes for the same three
 * permissions because the status DTO predates this component and its
 * field names (`mic`/`system_audio`) don't match `PermissionKind`'s
 * (`microphone`/`screen_recording`) — not worth a breaking DTO rename for. */
export const TO_PERMISSION_KIND: Record<PermissionRowKind, PermissionKind> = {
  mic: "microphone",
  accessibility: "accessibility",
  system_audio: "screen_recording",
};

/** `queued`/`requesting` are UI-only transient states this component never
 * derives itself — only `Ready.tsx`'s status-mode caller passes them,
 * layered on top of the real backend `PermissionStatusDto` values (which
 * have no such distinction). See `Ready.tsx`'s module doc. */
export type DisplayStatus = "queued" | "requesting" | "granted" | "denied" | "not_requested" | "unavailable";

const STATUS_LABEL: Record<string, string> = {
  granted: "Granted",
  denied: "Denied",
  not_requested: "Not yet requested",
  unavailable: "Unavailable on this device",
  queued: "Queued",
  requesting: "Requesting…",
};

/** Whether a kind's active-request path is worth trying again given its
 * current real backend status. Mic's native prompt is a true one-shot —
 * denied is permanent, retrying is pointless, System Settings is the only
 * recovery path. Accessibility and Screen Recording have no such
 * permanence: `refresh()` for both (`permissions.rs`) never reports
 * `not_requested`, only `granted`/`denied` — Denied is their default read
 * before the user has ever been asked anything, not a real terminal
 * decision — so their active-request path should always be tried while
 * not yet Granted (design review, cross-model finding #4, 2026-09-01: the
 * original shared `status !== "denied"` gate silently no-op'd for these
 * two, since they read as "denied" from the very first check).
 *
 * Exported for `Ready.tsx`'s auto-fire effect to reuse directly — a real
 * live-verified bug (2026-09-01) came from that effect having its own
 * separate, blanket "denied is terminal" guard instead of this kind-specific
 * one: Accessibility/Screen Recording both read as `denied` on a fresh
 * install before ever being asked, so the separate guard skipped requesting
 * them entirely, silently reproducing the exact bug this function exists
 * to prevent, in a second place. Single source of truth now. */
export function canActivelyRequest(kind: PermissionRowKind, status: string | undefined): boolean {
  if (kind === "mic") return status !== "denied" && status !== "granted";
  return status !== "granted";
}

interface PermissionRowProps {
  kind: PermissionRowKind;
  /** Current status. In `mode: "action"` (default), this is real backend
   * status from `getPermissionStatus()` — `undefined` while loading. In
   * `mode: "status"`, the caller (`Ready.tsx`) fully controls this,
   * including the two UI-only transient values above. */
  status: DisplayStatus | string | undefined;
  /** Called after a Grant attempt resolves (mic's native prompt, or the
   * System Settings deep-link having been opened) so the caller can
   * refetch real status. Required in `mode: "action"`, unused in
   * `mode: "status"` (no button, nothing to attempt). */
  onGrantAttempted?: () => void;
  /** `"action"` (default) — the existing click-to-Grant row, used by
   * Settings.tsx and (pre-2026-09-01) the old onboarding Permissions
   * step. `"status"` — read-only, no button, used by `Ready.tsx`'s
   * auto-fire flow where the caller drives every state transition
   * directly instead of this component's own click handler. */
  mode?: "action" | "status";
}

/** One permission row — status pill, or (in `mode: "action"`) a real Grant
 * button. Shared by Settings.tsx and Ready.tsx's auto-fire status display
 * so both get correct per-permission behavior from one implementation, not
 * two near-duplicates.
 *
 * Row style (design review, 2026-09-01): matches the hotkey rows'
 * `bg-surface-inset` card treatment, no `border-b` divider — the two
 * previously used different visual languages, only visible as a mismatch
 * once both appeared together on `Ready.tsx`. DESIGN.md's "no box borders
 * on recessed elements" rule already called for this app-wide; this
 * component was the one holdout. */
export function PermissionRow({ kind, status, onGrantAttempted, mode = "action" }: PermissionRowProps) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Shared setError/setPending/try-finally/onGrantAttempted wrapper
  // (simplification specialist, pre-landing review, 2026-09-01) — both
  // handleGrant and handleOpenSettings below only ever differ in which
  // command they call, not in how the attempt is tracked or reported.
  async function attempt(action: () => Promise<Result<unknown, string>>) {
    setError(null);
    setPending(true);
    try {
      const res = await action();
      if (res.status === "error") {
        setError(res.error);
      }
    } finally {
      setPending(false);
      onGrantAttempted?.();
    }
  }

  async function handleGrant() {
    await attempt(() =>
      canActivelyRequest(kind, status)
        ? commands.requestPermission(TO_PERMISSION_KIND[kind])
        : commands.openPermissionSettings(TO_PERMISSION_KIND[kind]),
    );
  }

  /** Explicit System Settings escape hatch, Accessibility/Screen Recording
   * only (pre-landing review, 2026-09-01, adversarial finding). Unlike mic
   * (a true one-shot — `canActivelyRequest` returns false once denied, so
   * the SAME Grant button above already falls back to
   * `openPermissionSettings` via `handleGrant`'s own branching),
   * `canActivelyRequest` never returns false for these two kinds short of
   * `granted` — their `refresh()` has no true one-shot semantics, so
   * `handleGrant` always re-issues the native re-prompt, never the deep
   * link. That's fine while the OS keeps re-showing the "trust this app"
   * alert, but real-world TCC behavior for Accessibility can stop
   * re-showing it after repeat denials, silently turning Grant into a dead
   * click with no way back to System Settings. This link is a second,
   * always-available escape hatch once actually denied — not a replacement
   * for the primary button's re-prompt behavior. */
  async function handleOpenSettings() {
    await attempt(() => commands.openPermissionSettings(TO_PERMISSION_KIND[kind]));
  }

  const granted = status === "granted";
  const denied = status === "denied";
  const unavailable = status === "unavailable";
  const showButton = mode === "action" && !granted && !unavailable;
  const showPill = granted || denied;
  const showSettingsFallback = mode === "action" && kind !== "mic" && denied;

  return (
    <div className="rounded-small bg-surface-inset px-3 py-2">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm text-text-primary">{TITLE[kind]}</span>
        {showButton ? (
          <button
            type="button"
            onClick={() => void handleGrant()}
            disabled={pending}
            className="shrink-0 rounded-pill px-[9px] py-[3px] text-[9px] font-semibold text-white transition-opacity duration-fast hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
            style={{ backgroundColor: "var(--surface-filled)" }}
          >
            {pending ? "Requesting…" : "Grant"}
          </button>
        ) : showPill ? (
          <span
            className="shrink-0 rounded-pill px-[7px] py-0.5 text-[9px] font-medium"
            style={
              granted
                ? { backgroundColor: "rgba(48, 209, 88, 0.18)", color: "#8FEDB0" }
                : { backgroundColor: "rgba(255, 69, 58, 0.18)", color: "#FF9D97" }
            }
          >
            {granted ? "Granted" : "Denied"}
          </span>
        ) : (
          <span className="text-xs text-text-secondary">
            {status ? (STATUS_LABEL[status] ?? status) : "Checking…"}
          </span>
        )}
      </div>
      {showSettingsFallback && (
        <button
          type="button"
          onClick={() => void handleOpenSettings()}
          disabled={pending}
          className="mt-1 text-[10px] text-text-secondary underline decoration-dotted underline-offset-2 transition-opacity duration-fast hover:text-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
        >
          Open System Settings
        </button>
      )}
      {error && <div className="mt-1 text-xs text-text-primary opacity-80">{error}</div>}
    </div>
  );
}
