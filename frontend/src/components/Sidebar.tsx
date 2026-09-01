import { BarChart3, History, Settings, Power } from "lucide-react";
import { GlassPanel } from "./GlassPanel";

export type PanelId = "stats" | "history" | "settings";

interface SidebarProps {
  active: PanelId;
  onSelect: (panel: PanelId) => void;
  onQuit: () => void;
}

const NAV_ITEMS: { id: PanelId; label: string; icon: typeof BarChart3 }[] = [
  { id: "stats", label: "Stats", icon: BarChart3 },
  { id: "history", label: "History", icon: History },
  { id: "settings", label: "Settings", icon: Settings },
];

/** Floating icon nav rail — content-sized pill capsule, vertically centered
 * (DESIGN.md's "widget, not a huge desktop application" layout principle).
 * The decorative wave-mark brand icon that used to sit above the nav items
 * is gone (2026-08-31, user-directed removal) — the preview's sidebar is
 * nav icons only, no separate static brand mark. */
export function Sidebar({ active, onSelect, onQuit }: SidebarProps) {
  return (
    <GlassPanel className="flex w-10 flex-col items-center gap-1 self-center rounded-pill py-2">
      {NAV_ITEMS.map(({ id, label, icon: Icon }) => (
        <button
          key={id}
          type="button"
          aria-label={label}
          aria-current={active === id}
          onClick={() => onSelect(id)}
          className="flex h-[26px] w-[26px] items-center justify-center rounded-full text-text-primary transition-colors duration-base ease-standard focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
          style={{
            backgroundColor:
              active === id ? "var(--surface-active)" : "transparent",
          }}
        >
          <Icon size={16} strokeWidth={2} />
        </button>
      ))}

      <div className="my-1 h-px w-4 bg-glass-border" />

      <button
        type="button"
        aria-label="Quit Mutter"
        onClick={onQuit}
        className="flex h-[26px] w-[26px] items-center justify-center rounded-full text-danger transition-colors duration-base ease-standard hover:opacity-80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
      >
        <Power size={16} strokeWidth={2} />
      </button>
    </GlassPanel>
  );
}
