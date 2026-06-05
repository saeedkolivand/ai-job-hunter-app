// Prevents a terminal window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Thin binary launcher — the entire application lives in the library crate
// (`ajh_tauri`) so it is reachable from integration tests and `benches/`.
// See `lib.rs::run`. This is the canonical Tauri 2 lib + bin split.
fn main() {
    ajh_tauri::run();
}
