interface LanguageBarProps {
  language: string;
  count: number;
  averageWpm: number;
  fraction: number; // 0..1, this language's count relative to the top language
}

/** `Intl.DisplayNames` for the human-readable name — built into WKWebView
 * since Safari 14.1, well under this app's macOS 14 minimum. No hand-
 * maintained code-to-name table to drift as new languages show up. */
function displayName(code: string): string {
  if (code === "unknown") return "Unknown";
  try {
    return new Intl.DisplayNames(["en"], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

export function LanguageBar({ language, count, averageWpm, fraction }: LanguageBarProps) {
  return (
    <div className="flex items-center gap-3">
      <span className="w-[140px] shrink-0 truncate text-sm text-text-primary">
        {displayName(language)}
      </span>
      <div className="h-1.5 flex-1 rounded-pill bg-surface-track">
        <div
          className="h-full rounded-pill bg-surface-filled transition-transform duration-base ease-standard"
          style={{ width: `${Math.max(fraction * 100, 2)}%` }}
        />
      </div>
      <span className="w-20 shrink-0 text-right text-xs text-text-secondary">
        {count} · {Math.round(averageWpm)} wpm
      </span>
    </div>
  );
}
