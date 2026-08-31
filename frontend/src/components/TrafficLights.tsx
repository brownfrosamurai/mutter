import { getCurrentWindow } from "@tauri-apps/api/window";

/** Custom titlebar traffic lights — real macOS red/yellow/green (2026-08-31,
 * user-directed reversal of the earlier monochrome pass; DESIGN.md's own
 * "icon chrome matching a real, universally-recognized macOS convention"
 * reasoning applies again). Real window actions, not decorative: the
 * dashboard has no native decorations (`decorations: false`), so these are
 * the only way to close/minimize/maximize it. */
export function TrafficLights() {
  const appWindow = getCurrentWindow();

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        aria-label="Close"
        onClick={() => appWindow.hide()}
        className="h-2.5 w-2.5 rounded-full transition-opacity duration-fast hover:opacity-80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
        style={{ backgroundColor: "#ff5f57" }}
      />
      <button
        type="button"
        aria-label="Minimize"
        onClick={() => appWindow.minimize()}
        className="h-2.5 w-2.5 rounded-full transition-opacity duration-fast hover:opacity-80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
        style={{ backgroundColor: "#febc2e" }}
      />
      <button
        type="button"
        aria-label="Maximize"
        onClick={() => appWindow.toggleMaximize()}
        className="h-2.5 w-2.5 rounded-full transition-opacity duration-fast hover:opacity-80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus-ring"
        style={{ backgroundColor: "#28c840" }}
      />
    </div>
  );
}
