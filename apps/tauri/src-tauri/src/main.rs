// Prevents a terminal window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Thin binary launcher — the entire application lives in the library crate
// (`ajh_tauri`) so it is reachable from integration tests and `benches/`.
// See `lib.rs::run`. This is the canonical Tauri 2 lib + bin split.
fn main() {
    // Native-messaging short-circuit: when the browser launches this exe to spawn
    // the stdio relay host, run the relay and exit — Tauri + single-instance must
    // never boot (single-instance would forward argv to the running app and kill
    // the stdio Port). Detected purely from argv inside the library.
    if ajh_tauri::run_native_host_if_invoked() {
        return;
    }
    ajh_tauri::run();
}
