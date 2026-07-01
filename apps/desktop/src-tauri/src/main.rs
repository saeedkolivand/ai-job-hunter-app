// Prevents a terminal window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Thin binary launcher — the entire application lives in the library crate
// (`ajh_tauri`) so it is reachable from integration tests and `benches/`.
// See `lib.rs::run`. This is the canonical Tauri 2 lib + bin split.
fn main() {
    // Work around the Steam Deck / Mesa-Wayland AppImage blank-screen abort
    // (`EGL_BAD_PARAMETER`): set the WebKit DMABUF/compositing env vars and, on a
    // Wayland AppImage launch, LD_PRELOAD the host libwayland and re-exec so the
    // bundled copy is shadowed. MUST be the FIRST statement in `main()` — before
    // any WebKitGTK/GTK init AND before the native-host short-circuit — because
    // the env vars are read at webview init and the re-exec must precede dynamic
    // linking. On a successful preload re-exec this never returns; otherwise it is
    // a no-op (non-Linux, non-AppImage, X11, no host lib, or already re-exec'd).
    // Centralized in `platform` so the launcher never touches `std::env` (R4).
    ajh_tauri::platform::linux_appimage::apply_wayland_appimage_safeguard();

    // Native-messaging short-circuit: when the browser launches this exe to spawn
    // the stdio relay host, run the relay and exit — Tauri + single-instance must
    // never boot (single-instance would forward argv to the running app and kill
    // the stdio Port). Detected purely from argv inside the library.
    if ajh_tauri::run_native_host_if_invoked() {
        return;
    }
    ajh_tauri::run();
}
