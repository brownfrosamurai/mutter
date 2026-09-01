import { useState } from "react";
import { commands, type PermissionKind } from "@/lib/bindings";

export type PermissionRowKind = "mic" | "accessibility" | "system_audio";

const TITLE: Record<PermissionRowKind, string> = {
  mic: "Microphone",
  accessibility: "Accessibility",
  system_audio: "Screen Recording",
};

/** `PermissionRowKind` (matches `PermissionStatusDto`'s field names) ->
 * the Rust `PermissionKind` wire enum (`openPermissionSettings`/
 * `requestMicAccess`'s target). Two different shapes for the same three
 * permissions because the status DTO predates this component and its
 * field names (`mic`/`system_audio`) don't match `PermissionKind`'s
 * (`microphone`/`screen_recording`) — not worth a breaking DTO rename for. */
const TO_PERMISSION_KIND: Record<PermissionRowKind, PermissionKind> = {
  mic: "microphone",
  accessibility: "accessibility",
  system_audio: "screen_recording",
};

const STATUS_LABEL: Record<string, string> = {
  granted: "Granted",
  denied: "Denied",
  not_requested: "Not yet requested",
  unavailable: "Unavailable on this device",
};

interface PermissionRowProps {
  kind: PermissionRowKind;
  /** Current status from `getPermissionStatus()` — `undefined` while
   * loading. */
  status: string | undefined;
  /** Called after a Grant attempt resolves (mic's native prompt, or the
   * System Settings deep-link having been opened) so the caller can
   * refetch real status — see `PermissionRow`'s own window-`focus`
   * refetch note in the onboarding/Settings callers for why a one-shot
   * refetch right here isn't enough on its own for the System-Settings
   * path (the user hasn't acted yet at the moment this fires). */
  onGrantAttempted: () => void;
}

/** One permission row — status text, or a real Grant action, per kind
 * (Outside Voice finding #2/#3, onboarding-flow plan review). Mic gets a
 * real native permission prompt (`requestMicAccess`); Accessibility and
 * Screen Recording deep-link to System Settings (macOS has no active-
 * request API for either) — same component, different Grant behavior per
 * `kind`, shared by the onboarding Permissions step and the dashboard's
 * Settings panel so both get correct behavior from one implementation. */
export function PermissionRow({ kind, status, onGrantAttempted }: PermissionRowProps) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleGrant() {
    setError(null);
    setPending(true);
    try {
      const res =
        kind === "mic" && status !== "denied"
          ? await commands.requestMicAccess()
          : await commands.openPermissionSettings(TO_PERMISSION_KIND[kind]);
      if (res.status === "error") {
        setError(res.error);
      }
    } finally {
      setPending(false);
      onGrantAttempted();
    }
  }

  const granted = status === "granted";
  const denied = status === "denied";
  const unavailable = status === "unavailable";
  const canGrant = !granted && !unavailable;

  return (
    <div className="border-b border-glass-border py-2 last:border-b-0">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm text-text-primary">{TITLE[kind]}</span>
        {canGrant ? (
          <button
            type="button"
            onClick={() => void handleGrant()}
            disabled={pending}
            className="shrink-0 rounded-pill px-[9px] py-[3px] text-[9px] font-semibold text-white transition-opacity duration-fast hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
            style={{ backgroundColor: "var(--surface-filled)" }}
          >
            {pending ? "Requesting…" : "Grant"}
          </button>
        ) : granted ? (
          // Tinted status pill (design-consultation preview's "status-pill"
          // treatment) — the green tint token doing real signaling work,
          // not just plain secondary text.
          <span
            className="shrink-0 rounded-pill px-[7px] py-0.5 text-[9px] font-medium"
            style={{ backgroundColor: "rgba(48, 209, 88, 0.18)", color: "#8FEDB0" }}
          >
            Granted
          </span>
        ) : denied ? (
          <span
            className="shrink-0 rounded-pill px-[7px] py-0.5 text-[9px] font-medium"
            style={{ backgroundColor: "rgba(255, 69, 58, 0.18)", color: "#FF9D97" }}
          >
            Denied
          </span>
        ) : (
          <span className="text-xs text-text-secondary">
            {status ? (STATUS_LABEL[status] ?? status) : "Checking…"}
          </span>
        )}
      </div>
      {error && <div className="mt-1 text-xs text-text-primary opacity-80">{error}</div>}
    </div>
  );
}
