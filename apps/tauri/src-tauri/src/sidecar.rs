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

    let shell = app.shell();

    // In production: launch the platform-specific binary bundled as a resource.
    // In dev (debug builds): run TypeScript directly with tsx — no build step needed,
    // changes take effect on Tauri restart.
    #[cfg(not(debug_assertions))]
    let spawn_result = {
        use tauri::Manager;
        let target = env!("TAURI_ENV_TARGET_TRIPLE");
        let bin_name = format!("scraper-runtime-{}", target);
        let bin_path = app
            .path()
            .resource_dir()
            .map(|d| d.join(&bin_name))
            .unwrap_or_else(|_| std::path::PathBuf::from(&bin_name));
        shell.command(bin_path.to_string_lossy().as_ref()).spawn()
    };

    #[cfg(debug_assertions)]
    let spawn_result = {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let src = format!("{manifest_dir}/../../scraper-runtime/src/index.ts");
        // Use npx so tsx doesn't need to be globally installed.
        // On Windows the binary is npx.cmd; on Unix it's npx.
        #[cfg(target_os = "windows")]
        let npx = "npx.cmd";
        #[cfg(not(target_os = "windows"))]
        let npx = "npx";
        shell.command(npx).args(["tsx", &src]).spawn()
    };

    let (mut rx, _child) = match spawn_result {
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
