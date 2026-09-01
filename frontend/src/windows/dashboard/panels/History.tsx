import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Search, Copy, Check } from "lucide-react";
import { commands, type HistoryEntryDto } from "@/lib/bindings";

const HISTORY_PAGE_SIZE = 50;

function formatDuration(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

function formatRelativeTime(unixSeconds: number): string {
  const diffMs = Date.now() - unixSeconds * 1000;
  const diffMin = Math.round(diffMs / 60_000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHours = Math.round(diffMin / 60);
  if (diffHours < 24) return `${diffHours}h ago`;
  return `${Math.round(diffHours / 24)}d ago`;
}

function languageName(code: string): string {
  if (code === "unknown") return "Unknown";
  try {
    return new Intl.DisplayNames(["en"], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

function HistoryRow({ entry }: { entry: HistoryEntryDto }) {
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    const res = await commands.copyHistoryText(entry.text);
    if (res.status === "ok") {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }
  }

  return (
    <div className="flex items-start gap-2 border-b border-glass-border py-3 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p dir="auto" className="truncate text-sm text-text-primary">
          {entry.text}
        </p>
        <div className="mt-1 flex items-center gap-2 text-xs text-text-secondary">
          <span>{formatRelativeTime(entry.timestamp)}</span>
          <span aria-hidden="true">·</span>
          <span>{languageName(entry.language)}</span>
          <span aria-hidden="true">·</span>
          <span>{formatDuration(entry.duration_secs)}</span>
          <span aria-hidden="true">·</span>
          <span>{entry.text.split(/\s+/).filter(Boolean).length} words</span>
        </div>
      </div>
      <button
        type="button"
        aria-label={copied ? "Copied" : "Copy transcript"}
        onClick={() => void handleCopy()}
        className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-[6px] bg-surface-inset text-text-secondary transition-colors duration-fast hover:bg-surface-hover hover:text-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
      >
        {copied ? <Check size={13} /> : <Copy size={13} />}
      </button>
    </div>
  );
}

export function HistoryPanel() {
  const [query, setQuery] = useState("");

  const history = useQuery({
    queryKey: ["history-page", 0, HISTORY_PAGE_SIZE],
    queryFn: async () => {
      const res = await commands.getHistoryPage(0, HISTORY_PAGE_SIZE);
      if (res.status === "error") throw new Error(res.error);
      return res.data;
    },
  });

  const filtered = useMemo(() => {
    if (!history.data) return [];
    const q = query.trim().toLowerCase();
    if (!q) return history.data;
    return history.data.filter((e) => e.text.toLowerCase().includes(q));
  }, [history.data, query]);

  return (
    <div className="flex flex-col gap-4">
      <div className="relative">
        <Search
          size={14}
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary"
        />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search recent transcripts…"
          className="w-full rounded-small bg-surface-inset py-2 pl-8 pr-3 text-sm text-text-primary placeholder:text-text-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
        />
      </div>

      {filtered.length === 0 ? (
        <p className="text-sm text-text-secondary">
          {history.data?.length ? "No matches." : "No dictations yet."}
        </p>
      ) : (
        <div>
          {filtered.map((entry) => (
            <HistoryRow key={entry.timestamp} entry={entry} />
          ))}
        </div>
      )}
    </div>
  );
}
