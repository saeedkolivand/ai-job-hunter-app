/// Scraper sidecar lifecycle management.
///
/// The scraper-runtime binary is launched as a Tauri sidecar. On startup it
/// writes one JSON line to stdout:
///   {"port":<N>,"status":"ready"}
///
/// We read that line, parse the port, and store it in ScraperSidecarState so
/// Tauri commands can proxy scraping requests to the sidecar over HTTP.
///
/// ── Rollback ──────────────────────────────────────────────────────────────
/// If the sidecar binary is absent (development workflow), try_start returns
/// Ok and leaves ScraperSidecarState::port as None. All scrape commands then
/// return a "sidecar not available" stub rather than crashing.
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Default)]
pub struct ScraperSidecarState {
    /// Set to true once the sidecar process has been launched.
    pub running: bool,
    /// HTTP port announced by the sidecar on stdout. None until announcement
    /// is received or if the sidecar is absent.
    pub port: Option<u16>,
}

/// Attempt to launch the scraper sidecar and wait for its port announcement.
///
/// Non-fatal: if the binary is absent the app opens with sidecar stubs.
pub fn try_start(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<Mutex<ScraperSidecarState>>();
    {
        let guard = state.lock().unwrap();
        if guard.running {
            return Ok(());
        }
    }

    let shell = app.shell();
    let sidecar_cmd = match shell.sidecar("scraper-runtime") {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("[sidecar] scraper-runtime binary not found — sidecar unavailable: {e}");
            return Ok(());
        }
    };

    let (mut rx, _child) = match sidecar_cmd.spawn() {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("[sidecar] failed to spawn scraper-runtime: {e}");
            return Ok(());
        }
    };

    {
        let mut guard = state.lock().unwrap();
        guard.running = true;
    }

    // Spawn a task to read stdout and capture the port announcement.
    let state_arc: Arc<Mutex<ScraperSidecarState>> =
        Arc::clone(&*app.state::<Mutex<ScraperSidecarState>>());
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let text = String::from_utf8_lossy(&line);
                    // Parse {"port":<N>,"status":"ready"}
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                        if let Some(port) = v.get("port").and_then(|p| p.as_u64()) {
                            let mut guard = state_arc.lock().unwrap();
                            guard.port = Some(port as u16);
                            eprintln!("[sidecar] ready on port {port}");
                        }
                    }
                }
                CommandEvent::Stderr(line) => {
                    eprintln!("[sidecar stderr] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Error(msg) => {
                    eprintln!("[sidecar error] {msg}");
                    break;
                }
                CommandEvent::Terminated(status) => {
                    eprintln!("[sidecar] process exited: {status:?}");
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(())
}
