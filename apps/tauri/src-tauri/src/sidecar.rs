/// Scraper sidecar lifecycle management.
///
/// The scraper-runtime binary (a bundled Node.js executable) is launched as a
/// Tauri sidecar. It communicates over stdio using the ScraperCommand /
/// ScraperEvent protocol defined in packages/data.
///
/// In this spike the sidecar is configured but not yet launched automatically.
/// Call `try_start` once the app is ready; it logs a warning and returns Ok
/// if the binary is absent (development workflow where the sidecar is not
/// bundled yet).
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

pub struct ScraperSidecarState {
    pub running: bool,
}

impl Default for ScraperSidecarState {
    fn default() -> Self {
        Self { running: false }
    }
}

/// Attempt to launch the scraper sidecar. Logs and returns Ok if the binary
/// is not bundled — the UI degrades gracefully to "scraper unavailable".
pub fn try_start(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<Mutex<ScraperSidecarState>>();
    let mut guard = state.lock().unwrap();

    if guard.running {
        return Ok(());
    }

    let shell = app.shell();
    match shell.sidecar("scraper-runtime") {
        Ok(cmd) => {
            let (mut _rx, _child) = cmd
                .args(["--mode", "sidecar"])
                .spawn()
                .map_err(|e| {
                    eprintln!("[sidecar] failed to spawn scraper-runtime: {e}");
                    e
                })?;
            guard.running = true;
            println!("[sidecar] scraper-runtime started");
        }
        Err(e) => {
            // Binary absent during development — not a fatal error for the spike.
            eprintln!("[sidecar] scraper-runtime binary not found, continuing without it: {e}");
        }
    }

    Ok(())
}
