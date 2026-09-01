import { useState } from "react";
import { commands } from "@/lib/bindings";
import { Sidebar, type PanelId } from "@/components/Sidebar";
import { GlassPanel } from "@/components/GlassPanel";
import { TrafficLights } from "@/components/TrafficLights";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { StatsPanel } from "./panels/Stats";
import { HistoryPanel } from "./panels/History";
import { SettingsPanel } from "./panels/Settings";

const PANEL_TITLE: Record<PanelId, string> = {
  stats: "Metrics",
  history: "History",
  settings: "Settings",
};

/** Dashboard shell: floating sidebar + a single content card, each its own
 * independent `.glass-panel` (2026-08-31, user-directed — after native
 * vibrancy masking proved unreliable trying to constrain one window-wide
 * vibrancy layer to two shapes, the window itself now applies vibrancy
 * uniformly, same proven mechanism as pill/recovery — see lib.rs's `run()`
 * setup() comment for the full history). The gap around the floating
 * sidebar is also vibrant now, not real unblurred desktop — a deliberate
 * trade for reliability over the masking approach's edge-bleed risk.
 *
 * **Shell corners, 2026-09-01 — attempted, reverted, not shipped.** A
 * single-shape native mask (reusing the pill's proven `mask_to_shape`) was
 * tried here to round the window's outer corners, user-directed. Live
 * screenshots and native NSLog diagnostics confirmed the reported rect
 * exactly matched the real vibrancy view's own bounds, yet the rendered
 * result stayed square at the top regardless — even a deliberately wrong,
 * drastically inset test rect produced no visible change at all. See
 * `lib.rs`'s `reveal_dashboard` doc comment for the full writeup. Root
 * cause not found; this is the same fragility class already documented for
 * the dashboard's vibrancy in the paragraph above. */
export function App() {
  const [active, setActive] = useState<PanelId>("stats");

  return (
    <div className="flex h-screen gap-3 p-5 ">
      <Sidebar
        active={active}
        onSelect={setActive}
        onQuit={() => {
          void commands.quitApp();
        }}
      />

      <GlassPanel
        thick
        className="relative flex-1 overflow-hidden rounded-panel"
      >
        <div
          data-tauri-drag-region
          className="absolute inset-x-0 top-0 z-10 flex h-11 items-center justify-between px-4"
        >
          <TrafficLights />
          <span className="pointer-events-none absolute left-1/2 -translate-x-1/2 text-lg uppercase tracking-wide text-text-primary">
            {PANEL_TITLE[active]}
          </span>
          <span className="w-[52px]" aria-hidden="true" />
        </div>

        <div className="h-full select-none overflow-y-auto px-4 pb-4 pt-12">
          <ErrorBoundary key={active}>
            {active === "stats" && <StatsPanel />}
            {active === "history" && <HistoryPanel />}
            {active === "settings" && <SettingsPanel />}
          </ErrorBoundary>
        </div>
      </GlassPanel>
    </div>
  );
}
