import { useEffect, useState } from "react";
import { commands } from "@/lib/bindings";
import { GlassPanel } from "@/components/GlassPanel";

/** Shown instead of pill/dashboard only when HistoryStore::open() returns
 * MigrationFailed (Section 11) — see lib.rs's setup(). */
export function Recovery() {
  const [backupPath, setBackupPath] = useState<string | null>(null);

  useEffect(() => {
    void commands.getRecoveryInfo().then((path) => setBackupPath(path ?? null));
  }, []);

  return (
    <div className="flex h-screen items-center justify-center p-4">
      <GlassPanel thick className="max-w-md rounded-panel p-6 text-center">
        <div aria-hidden="true" className="mx-auto mb-4 h-10 w-10 text-warning">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M12 9v4M12 17h.01M10.29 3.86l-8.18 14.18A2 2 0 0 0 3.82 21h16.36a2 2 0 0 0 1.71-2.96L13.71 3.86a2 2 0 0 0-3.42 0z" />
          </svg>
        </div>
        <h1 className="text-lg font-semibold text-text-primary">History database needs attention</h1>
        <p className="mt-2 text-sm text-text-secondary">
          Mutter couldn't safely upgrade your dictation history to its latest format, so it's not
          being touched further. Your existing history hasn't been lost.
        </p>
        <div className="mt-4 rounded-small border border-glass-border bg-surface-inset p-3 text-left">
          <p className="text-xs text-text-secondary">Your existing history was backed up to:</p>
          <code className="mt-1 block break-all text-xs text-text-primary">
            {backupPath ?? "(no backup path available)"}
          </code>
        </div>
        <p className="mt-4 text-xs text-text-secondary">
          Restart Mutter after resolving this, or contact support with the path above.
        </p>
        <button
          type="button"
          onClick={() => void commands.quitApp()}
          className="mt-5 rounded-small bg-danger px-4 py-2 text-sm font-medium text-black transition-opacity duration-fast hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
        >
          Quit
        </button>
      </GlassPanel>
    </div>
  );
}
