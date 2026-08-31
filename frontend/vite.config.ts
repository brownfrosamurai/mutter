import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

// Tauri's own recommended Vite setup for a multi-window app: one HTML entry
// per window (dashboard/pill/recovery), a fixed dev-server port matching
// tauri.conf.json's `build.devUrl`, and HMR tuned for the Tauri host
// (ignoring src-tauri/ so a Rust rebuild doesn't also trigger a frontend
// reload) — see https://tauri.app/develop/#vite.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
    },
  },
  build: {
    rollupOptions: {
      input: {
        dashboard: resolve(__dirname, "dashboard.html"),
        pill: resolve(__dirname, "pill.html"),
        recovery: resolve(__dirname, "recovery.html"),
      },
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
