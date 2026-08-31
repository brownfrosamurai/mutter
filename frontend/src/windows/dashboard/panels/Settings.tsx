import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronRight } from "lucide-react";
import { commands, type SettingField } from "@/lib/bindings";
import { SettingRow } from "@/components/SettingRow";
import { HotkeyCapture } from "@/components/HotkeyCapture";

const PERMISSION_LABEL: Record<string, string> = {
  granted: "Granted",
  denied: "Denied — enable in System Settings",
  not_requested: "Not yet requested",
  unavailable: "Unavailable on this device",
};

/** Every new toggle (D3's SettingField union) — title/description matches
 * the reference screenshots, with one deliberate wording adjustment
 * (Capitalise sentences) confirmed with the user in `/plan-eng-review`:
 * the toggle keeps today's real first-letter-only behavior, so the
 * description says exactly that instead of overclaiming per-sentence
 * capitalisation. */
const OUTPUT_TOGGLES: { field: SettingField; title: string; description: string }[] = [
  {
    field: "pasteAutomatically",
    title: "Paste automatically",
    description: "Paste into whatever had focus. Turn this off to only copy to the clipboard.",
  },
  {
    field: "restoreClipboard",
    title: "Restore my clipboard",
    description:
      "Put back what was on the clipboard after pasting. Turn this off if pastes arrive empty in a particular app.",
  },
  {
    field: "capitaliseSentences",
    title: "Capitalise sentences",
    description: "Capitalise the first letter of each transcript.",
  },
  {
    field: "tidyPunctuation",
    title: "Tidy punctuation",
    description: "Normalise spacing, quotes and terminal punctuation.",
  },
  {
    field: "removeFillerWords",
    title: "Remove filler words",
    description: 'Drop "um", "uh", "you know" and similar so speech reads like writing.',
  },
  {
    field: "spokenFormatting",
    title: "Spoken formatting",
    description: 'Turn "new line", "new paragraph", "comma" and "period" into the thing you said.',
  },
  {
    field: "applySpokenCorrections",
    title: "Apply spoken corrections",
    description:
      'When you correct yourself out loud — "Tuesday, sorry, I meant Wednesday" — keep only the correction. It matches spoken phrases like "I meant" and "make that"; it does not rewrite your wording.',
  },
];

export function SettingsPanel() {
  const queryClient = useQueryClient();
  const [advancedOpen, setAdvancedOpen] = useState(false);

  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: () => commands.getSettings(),
  });
  const permissions = useQuery({
    queryKey: ["permissions"],
    queryFn: () => commands.getPermissionStatus(),
  });

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
        <h2 className="mb-1 text-xs uppercase tracking-wide text-text-secondary">Output</h2>
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
        <h2 className="mb-1 text-xs uppercase tracking-wide text-text-secondary">Permissions</h2>
        <div>
          {(["mic", "accessibility", "system_audio"] as const).map((key) => (
            <div key={key} className="flex items-center justify-between border-b border-glass-border py-2 last:border-b-0">
              <span className="setting-label text-sm text-text-primary">
                {key === "mic" ? "Microphone" : key === "accessibility" ? "Accessibility" : "Screen Recording"}
              </span>
              <span className="text-xs text-text-secondary">
                {permissions.data ? PERMISSION_LABEL[permissions.data[key]] : "Checking…"}
              </span>
            </div>
          ))}
          <div className="flex items-center justify-between border-b border-glass-border py-2 last:border-b-0">
            <span className="setting-label text-sm text-text-primary">Engine</span>
            <span className="text-xs text-text-secondary" title="Apple Speech was not built — Whisper already won the Phase 0 benchmark for English.">
              Whisper (small) — English
            </span>
          </div>
        </div>
      </section>

      <section>
        <button
          type="button"
          onClick={() => setAdvancedOpen((o) => !o)}
          className="flex items-center gap-1 text-xs uppercase tracking-wide text-text-secondary"
        >
          {advancedOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          Advanced
        </button>
        {advancedOpen && settings.data && (
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
