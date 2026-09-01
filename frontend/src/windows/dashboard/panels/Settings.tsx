import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { commands, type SettingField } from "@/lib/bindings";
import { SettingRow } from "@/components/SettingRow";
import { HotkeyCapture } from "@/components/HotkeyCapture";
import { PermissionRow, type PermissionRowKind } from "@/components/PermissionRow";
import { usePermissionsQuery } from "@/lib/hooks";

/** Title/description matches the reference screenshots, with one deliberate
 * wording adjustment (Capitalise sentences) confirmed with the user in
 * `/plan-eng-review`: the toggle keeps today's real first-letter-only
 * behavior, so the description says exactly that instead of overclaiming
 * per-sentence capitalisation.
 *
 * Trimmed to exactly the two toggles the design-consultation preview's
 * "Cleanup" section shows (2026-09-01, user-directed: "match the preview
 * exactly... remove things if necessary") — pasteAutomatically,
 * restoreClipboard, tidyPunctuation, spokenFormatting, and
 * applySpokenCorrections are no longer exposed here. Their backend fields
 * and behavior are untouched (still real `AppSettings` fields, still wired
 * into the grammar pipeline, still readable/editable via settings.json) —
 * only the UI controls for them are gone. Easy one-line-per-row revert if
 * that turns out to be the wrong call; see git history for the removed
 * entries. */
const OUTPUT_TOGGLES: { field: SettingField; title: string; description: string }[] = [
  {
    field: "capitaliseSentences",
    title: "Capitalise sentences",
    description: "Capitalise the first letter of each transcript.",
  },
  {
    field: "removeFillerWords",
    title: "Remove filler words",
    description: 'Drop "um", "uh", "you know" and similar so speech reads like writing.',
  },
];

type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up-to-date" }
  | { kind: "available"; update: Update }
  | { kind: "downloading"; percent: number | null }
  | { kind: "ready" }
  | { kind: "error"; message: string };

/** Real `tauri-plugin-updater` flow, not a placeholder — `check()` hits the
 * endpoint pinned in tauri.conf.json (a `latest.json` manifest published by
 * the repo's own GitHub Actions release workflow, `.github/workflows/
 * release.yml`), and a found update is Ed25519-verified against the pubkey
 * already pinned there before anything downloads. `downloadAndInstall`
 * replaces the app bundle on disk; `relaunch` (a separate plugin,
 * `tauri-plugin-process`) is the one explicit step needed to actually run
 * the new binary — the updater itself never restarts the app on its own. */
function UpdateRow() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });

  async function handleCheck() {
    setState({ kind: "checking" });
    try {
      const update = await check();
      setState(update ? { kind: "available", update } : { kind: "up-to-date" });
    } catch (e) {
      setState({ kind: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }

  async function handleInstall(update: Update) {
    setState({ kind: "downloading", percent: null });
    try {
      let downloaded = 0;
      let total: number | undefined;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setState({
            kind: "downloading",
            percent: total ? Math.round((downloaded / total) * 100) : null,
          });
        }
      });
      setState({ kind: "ready" });
    } catch (e) {
      setState({ kind: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }

  const statusText = (() => {
    switch (state.kind) {
      case "idle":
        return "Check GitHub Releases for a newer build.";
      case "checking":
        return "Checking…";
      case "up-to-date":
        return "You're on the latest version.";
      case "available":
        return `Version ${state.update.version} is available.`;
      case "downloading":
        return state.percent === null ? "Downloading…" : `Downloading… ${state.percent}%`;
      case "ready":
        return "Downloaded — restart to finish updating.";
      case "error":
        return state.message;
    }
  })();

  return (
    <div className="flex items-start justify-between gap-4 border-b border-glass-border py-3 last:border-b-0">
      <div className="min-w-0">
        <div className="text-sm font-medium text-text-primary">Software update</div>
        <div className="mt-0.5 text-xs text-text-secondary">{statusText}</div>
      </div>
      {state.kind === "available" ? (
        <button
          type="button"
          onClick={() => void handleInstall(state.update)}
          className="shrink-0 rounded-small border border-glass-border bg-surface-toggle-track px-3 py-1.5 text-sm text-text-primary transition-colors duration-fast hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
        >
          Install
        </button>
      ) : state.kind === "ready" ? (
        <button
          type="button"
          onClick={() => void relaunch()}
          className="shrink-0 rounded-small border border-glass-border bg-surface-toggle-track px-3 py-1.5 text-sm text-text-primary transition-colors duration-fast hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
        >
          Restart
        </button>
      ) : (
        <button
          type="button"
          onClick={() => void handleCheck()}
          disabled={state.kind === "checking" || state.kind === "downloading"}
          className="shrink-0 rounded-small border border-glass-border bg-surface-toggle-track px-3 py-1.5 text-sm text-text-primary transition-colors duration-fast hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
        >
          {state.kind === "checking" ? "Checking…" : "Check for Updates"}
        </button>
      )}
    </div>
  );
}

export function SettingsPanel() {
  const queryClient = useQueryClient();

  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: () => commands.getSettings(),
  });
  const permissions = usePermissionsQuery();

  async function handleToggle(field: SettingField, enabled: boolean) {
    // Optimistic update, reverted on error — same pattern as the pre-
    // rewrite grammar-cleanup toggle.
    queryClient.setQueryData(["settings"], (prev: typeof settings.data) =>
      prev ? { ...prev, [fieldToSnakeCase(field)]: enabled } : prev,
    );
    const res = await commands.setBoolSetting(field, enabled);
    if (res.status === "error") {
      queryClient.setQueryData(["settings"], (prev: typeof settings.data) =>
        prev ? { ...prev, [fieldToSnakeCase(field)]: !enabled } : prev,
      );
    }
  }

  async function handleGrammarLlmToggle(enabled: boolean) {
    queryClient.setQueryData(["settings"], (prev: typeof settings.data) =>
      prev ? { ...prev, grammar_llm_cleanup_enabled: enabled } : prev,
    );
    const res = await commands.setGrammarLlmCleanupEnabled(enabled);
    if (res.status === "error") {
      queryClient.setQueryData(["settings"], (prev: typeof settings.data) =>
        prev ? { ...prev, grammar_llm_cleanup_enabled: !enabled } : prev,
      );
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <section>
        <h2 className="mb-1 text-xs uppercase tracking-wide text-text-secondary">Permissions</h2>
        <div>
          {(["mic", "accessibility", "system_audio"] as const satisfies readonly PermissionRowKind[]).map(
            (key) => (
              <PermissionRow
                key={key}
                kind={key}
                status={permissions.data?.[key]}
                onGrantAttempted={() => void permissions.refetch()}
              />
            ),
          )}
        </div>
      </section>

      {/* Promoted out of a collapsed "Advanced" disclosure — the preview
          shows hotkey configuration as a first-class, always-visible
          section, not a hidden power-user detail. Still click-to-capture
          (HotkeyCapture), not a typed-string + Save field — that flow was
          deliberately replaced pre-redesign for a real, tested UX reason
          documented in HotkeyCapture's own module doc; the preview's
          simpler text-input mock isn't a good enough reason to regress it. */}
      <section>
        <h2 className="mb-1 text-xs uppercase tracking-wide text-text-secondary">Hotkey</h2>
        {settings.data && (
          <div className="mt-3 grid grid-cols-2 gap-3">
            <HotkeyCapture
              title="Mic Dictation"
              description="Records from default mic"
              shortcut={settings.data.mic_hotkey}
              onCapture={async (shortcut) => {
                const res = await commands.setHotkey("mic", shortcut);
                if (res.status === "error") throw new Error(res.error);
                await queryClient.invalidateQueries({ queryKey: ["settings"] });
              }}
            />
            <HotkeyCapture
              title="System Audio"
              description="Captures internal audio"
              shortcut={settings.data.system_audio_hotkey}
              onCapture={async (shortcut) => {
                const res = await commands.setHotkey("system_audio", shortcut);
                if (res.status === "error") throw new Error(res.error);
                await queryClient.invalidateQueries({ queryKey: ["settings"] });
              }}
            />
          </div>
        )}
      </section>

      <section>
        <h2 className="mb-1 text-xs uppercase tracking-wide text-text-secondary">Cleanup</h2>
        <div>
          {OUTPUT_TOGGLES.map(({ field, title, description }) => (
            <SettingRow
              key={field}
              title={title}
              description={description}
              checked={settings.data ? Boolean(settings.data[fieldToSnakeCase(field) as keyof typeof settings.data]) : true}
              onCheckedChange={(checked) => void handleToggle(field, checked)}
              disabled={!settings.data}
            />
          ))}
          <SettingRow
            title="AI grammar cleanup"
            description="Runs a small local model on every transcript for real grammar/word-choice correction, on top of the always-on punctuation cleanup. Off by default: downloads a ~390MB model on first use, adds latency to every dictation, and may occasionally reword precise technical terms — worth leaving off while dictating to an AI coding agent."
            checked={settings.data?.grammar_llm_cleanup_enabled ?? false}
            onCheckedChange={(checked) => void handleGrammarLlmToggle(checked)}
            disabled={!settings.data}
          />
        </div>
      </section>

      <section>
        <h2 className="mb-1 text-xs uppercase tracking-wide text-text-secondary">Updates</h2>
        <div>
          <UpdateRow />
        </div>
      </section>
    </div>
  );
}

/** SettingField is camelCase (matches the TS convention tauri-specta
 * generates); AppSettings' own fields are snake_case (matches Rust/serde's
 * default). Both describe the same seven fields — this just bridges the
 * naming convention gap between the command's enum and the settings
 * object's shape. */
function fieldToSnakeCase(field: SettingField): string {
  return field.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}
