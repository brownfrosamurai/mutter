import { useEffect, useState } from "react";
import { toSymbols } from "@/lib/hotkey";

interface HotkeyCaptureProps {
  title: string;
  description: string;
  shortcut: string; // Tauri shortcut string, e.g. "CmdOrCtrl+Shift+Space"
  onCapture: (shortcut: string) => Promise<void>;
}

/** Layout-independent physical-key mapping (KeyboardEvent.code, not .key) to
 * the key names Tauri's global-shortcut plugin expects. Covers letters,
 * digits, space, and function keys — the realistic range for a dictation
 * hotkey; not attempting to be a complete general-purpose keybinding
 * editor's mapping table. */
function codeToKeyName(code: string): string | null {
  if (code === "Space") return "Space";
  if (code.startsWith("Key")) return code.slice(3); // "KeyA" -> "A"
  if (code.startsWith("Digit")) return code.slice(5); // "Digit1" -> "1"
  if (/^F([1-9]|1[0-9])$/.test(code)) return code; // "F1".."F19"
  if (code === "Escape") return null; // reserved for canceling capture
  return null;
}

/** Click-to-capture hotkey editor (frontend-rewrite plan, confirmed with
 * the user over today's type-a-raw-string-and-Save UI). Click the keycap,
 * press the real combo, release — building the Tauri shortcut string from
 * live KeyboardEvent modifiers instead of a hand-typed string. Rejects a
 * bare, unmodified key client-side before ever calling `set_hotkey`,
 * mirroring `hotkey.rs`'s own `parse_shortcut` rejection (the same "a
 * global hotkey needs at least one modifier" incident documented there) —
 * instant feedback instead of a round-trip error. */
export function HotkeyCapture({ title, description, shortcut, onCapture }: HotkeyCaptureProps) {
  const [capturing, setCapturing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!capturing) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Escape") {
        setCapturing(false);
        return;
      }

      const keyName = codeToKeyName(e.code);
      if (!keyName) return; // modifier-only keydown, or an unsupported key — wait for a real key

      const modifiers: string[] = [];
      if (e.metaKey) modifiers.push("CmdOrCtrl");
      if (e.ctrlKey && !e.metaKey) modifiers.push("Ctrl");
      if (e.altKey) modifiers.push("Alt");
      if (e.shiftKey) modifiers.push("Shift");

      if (modifiers.length === 0) {
        setError("Needs at least one modifier key (⌘/⌃/⌥/⇧)");
        return;
      }

      const newShortcut = [...modifiers, keyName].join("+");
      setCapturing(false);
      setError(null);
      setSaving(true);
      onCapture(newShortcut)
        .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
        .finally(() => setSaving(false));
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [capturing, onCapture]);

  return (
    <div className="rounded-small border border-glass-border bg-surface-inset p-3">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="text-sm font-medium text-text-primary">{title}</div>
          <div className="text-xs text-text-secondary">{description}</div>
        </div>
        <button
          type="button"
          onClick={() => {
            setError(null);
            setCapturing(true);
          }}
          disabled={saving}
          className="rounded-small border border-glass-border bg-surface-toggle-track px-3 py-1.5 font-mono text-sm text-text-primary transition-colors duration-fast hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring disabled:opacity-50"
        >
          {capturing ? "Press keys…" : toSymbols(shortcut)}
        </button>
      </div>
      {error && <div className="mt-2 text-xs text-text-primary opacity-80">{error}</div>}
    </div>
  );
}
