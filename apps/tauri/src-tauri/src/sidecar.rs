/// Scraper sidecar lifecycle management.
///
/// The scraper-runtime binary is launched as a Tauri sidecar. On startup it
/// writes one JSON line to stdout:
///   {"port":<N>,"status":"ready"}
///
/// We read that line, parse the port, store it in ScraperSidecarState, then
/// push all OS-keychain credentials to the sidecar via set.credentials so the
/// sidecar can authenticate scrapers without its own secure storage.
///
/// ── Rollback ──────────────────────────────────────────────────────────────
/// If the sidecar binary is absent (development workflow), try_start returns
/// Ok and leaves ScraperSidecarState::port as None. All scrape commands then
/// return a "sidecar not available" stub rather than crashing.
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::credentials::CredentialStore;

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

    // In dev mode: launch via `node dist/index.js` from apps/scraper-runtime.
    // In production: the binary would be bundled via externalBin.
    let shell = app.shell();
    let sidecar_dist = std::env::var("AJH_SIDECAR_DIST").unwrap_or_else(|_| {
        // Default dev path relative to the Tauri src-tauri directory.
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        format!("{manifest_dir}/../../scraper-runtime/dist/index.js")
    });

    let (mut rx, _child) = match shell.command("node").args([&sidecar_dist]).spawn() {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("[sidecar] failed to spawn scraper-runtime (node {sidecar_dist}): {e}");
            return Ok(());
        }
    };

    {
        let mut guard = state.lock().unwrap();
        guard.running = true;
    }

    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let text = String::from_utf8_lossy(&line);
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                        if let Some(port) = v.get("port").and_then(|p| p.as_u64()) {
                            let port = port as u16;
                            {
                                let sidecar_state = app_handle.state::<Mutex<ScraperSidecarState>>();
                                let mut guard = sidecar_state.lock().unwrap();
                                guard.port = Some(port);
                            }
                            eprintln!("[sidecar] ready on port {port}");
                            push_credentials(&app_handle, port).await;
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

/// Push all OS-keychain credentials to the sidecar's set.credentials endpoint.
///
/// Called once after the sidecar announces its port. The sidecar holds them
/// in memory for the session — it has no secure storage of its own.
async fn push_credentials(app: &AppHandle, port: u16) {
    let creds = {
        let store = app.state::<Mutex<CredentialStore>>();
        let guard = store.lock().unwrap();
        guard.get_all_decrypted()
    };

    if creds.is_empty() {
        return;
    }

    let client = reqwest::Client::new();
    for (board_id, username, password) in creds {
        let cmd = serde_json::json!({
            "kind": "set.credentials",
            "boardId": board_id,
            "username": username,
            "password": password,
        });
        if let Err(e) = client
            .post(format!("http://127.0.0.1:{port}/command"))
            .json(&cmd)
            .send()
            .await
        {
            eprintln!("[sidecar] failed to push credentials for {board_id}: {e}");
        } else {
            eprintln!("[sidecar] pushed credentials for {board_id}");
        }
    }
}
