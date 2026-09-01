/** Renders a Tauri shortcut string as keycap symbols, e.g.
 * "CmdOrCtrl+Shift+Space" -> "⌘⇧Space" — matches the reference screenshots'
 * keycap badge look. macOS-only (this app's hard constraint), so
 * CmdOrCtrl always renders as ⌘. Shared by `HotkeyCapture` (Settings) and
 * the onboarding window's Ready step, which both need to render the same
 * two configured shortcuts (Code Quality finding #3, onboarding-flow plan
 * review). */
export function toSymbols(shortcut: string): string {
  const parts = shortcut.split("+");
  const key = parts.pop() ?? "";
  const symbolFor: Record<string, string> = {
    CmdOrCtrl: "⌘",
    Cmd: "⌘",
    Ctrl: "⌃",
    Alt: "⌥",
    Option: "⌥",
    Shift: "⇧",
  };
  const modifiers = parts.map((m) => symbolFor[m] ?? m).join("");
  return `${modifiers} ${key}`;
}
