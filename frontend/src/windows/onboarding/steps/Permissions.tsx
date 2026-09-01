import { PermissionRow, type PermissionRowKind } from "@/components/PermissionRow";
import { usePermissionsQuery } from "@/lib/hooks";

const KINDS: readonly PermissionRowKind[] = ["mic", "accessibility", "system_audio"];

/** Step 1 — reuses `getPermissionStatus` (already backs the dashboard's
 * Settings panel) via the shared `usePermissionsQuery` hook (Architecture
 * review finding #1's window-`focus` refetch lives there — see that hook's
 * doc comment) and the shared `PermissionRow` component. */
export function Permissions() {
  const permissions = usePermissionsQuery();

  return (
    <div>
      <h1 className="text-lg font-semibold text-text-primary">Grant permissions</h1>
      <p className="mt-2 text-sm text-text-secondary">
        Mutter needs these to hear you, type what you said, and (optionally) capture system
        audio for meeting transcription. You can grant these later in Settings if you'd rather
        skip for now.
      </p>
      <div className="mt-4">
        {KINDS.map((kind) => (
          <PermissionRow
            key={kind}
            kind={kind}
            status={permissions.data?.[kind]}
            onGrantAttempted={() => void permissions.refetch()}
          />
        ))}
      </div>
    </div>
  );
}
