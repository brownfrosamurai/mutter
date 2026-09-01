import {
  AudioWaveform,
  BarChart3,
  History,
  Settings,
  Power,
} from "lucide-react";
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
 * The top wave icon is a decorative brand mark (confirmed with the user,
 * not a 4th nav destination) — not a button, no click handler. Always shown
 * in the same highlighted state a selected nav icon gets (2026-08-31,
 * user-directed) — a permanent mark, not a selection state. Its own
 * independent `.glass-panel`, frosted separately from the main content
 * card via the window's uniform native vibrancy underneath both. */
export function Sidebar({ active, onSelect, onQuit }: SidebarProps) {
  return (
    <GlassPanel className="flex w-10 flex-col items-center gap-1 self-center rounded-pill py-2">
      <div
        aria-hidden="true"
        className="mb-1 flex h-[26px] w-[26px] items-center justify-center rounded-full text-text-primary"
        style={{ backgroundColor: "var(--surface-active)" }}
      >
        <AudioWaveform size={16} strokeWidth={2} />
      </div>

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
