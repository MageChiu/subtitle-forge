// ============================================================
// SubtitleForge — main.rs
// Tauri application entry point
// ============================================================

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    subtitle_forge_lib::run();
}
