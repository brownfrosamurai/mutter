// Prevents an additional console window on Windows in release builds —
// irrelevant for this macOS-only v1, kept for parity with Tauri's default
// scaffold in case that ever changes.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mutter_lib::run();
}
