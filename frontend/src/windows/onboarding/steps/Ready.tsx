import { useQuery } from "@tanstack/react-query";
import { Mic, Monitor } from "lucide-react";
import { commands } from "@/lib/bindings";
import { toSymbols } from "@/lib/hotkey";

/** Step 2 — reuses `getSettings` (already backs the dashboard's hotkey
 * cards) to show the two real, currently-configured shortcuts read-only,
 * via the shared `toSymbols` helper. */
export function Ready() {
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: () => commands.getSettings(),
  });

  return (
    <div>
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
          <span className="rounded-small bg-surface-toggle-track px-2 py-1 font-mono text-sm text-text-primary">
            {settings.data ? toSymbols(settings.data.mic_hotkey) : "…"}
          </span>
        </div>
        <div className="flex items-center justify-between rounded-small bg-surface-inset px-3 py-2">
          <span className="flex items-center gap-2 text-sm text-text-primary">
            <Monitor size={14} strokeWidth={2} />
            System Audio
          </span>
          <span className="rounded-small bg-surface-toggle-track px-2 py-1 font-mono text-sm text-text-primary">
            {settings.data ? toSymbols(settings.data.system_audio_hotkey) : "…"}
          </span>
        </div>
      </div>
    </div>
  );
}
