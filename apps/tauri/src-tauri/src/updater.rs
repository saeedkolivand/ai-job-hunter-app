/// Auto-updater for the Tauri shell.
///
/// Mirrors the Electron updater (apps/desktop/src/main/updater.ts) but uses
/// tauri-plugin-updater instead of electron-updater.
///
/// ── UpdateStatus shapes (must match use-updater.ts) ──────────────────────────
///   { state: "idle" }
///   { state: "checking" }
///   { state: "available",     version, releaseNotes? }
///   { state: "not-available" }
///   { state: "downloading",   percent }
///   { state: "downloaded",    version }
///   { state: "error",         message }
///
/// ── Event channel ────────────────────────────────────────────────────────────
///   updater:status  — emitted by every state transition; the renderer's
///   TauriInvokeClient.updater.onStatus subscribes to it via listen().
///
/// ── Three-step flow ──────────────────────────────────────────────────────────
///   updater_check    → check, emit available / not-available
///   updater_download → download with progress, emit downloading/downloaded
///   updater_install  → install bytes and relaunch
///
/// ── Background polling ───────────────────────────────────────────────────────
///   setup_auto_check() — silent check 10 s after launch, then every 4 h.
///   Call from main.rs setup closure.
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Holds the downloaded bytes between updater_download and updater_install.
#[derive(Default)]
pub struct UpdaterState {
    /// Version string of the available update (set by updater_check).
    pub pending_version: Option<String>,
    /// Raw bytes from the last successful download (set by updater_download).
    pub downloaded_bytes: Option<Vec<u8>>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn emit_status(app: &AppHandle, status: Value) {
    let _ = app.emit("updater:status", status);
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Check for an available update.
/// Emits checking → available(version) | not-available | error.
#[tauri::command]
pub async fn updater_check(app: AppHandle) -> Value {
    emit_status(&app, json!({ "state": "checking" }));

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            let msg = e.to_string();
            emit_status(&app, json!({ "state": "error", "message": msg }));
            return json!({ "error": msg });
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let notes = update.body.clone();
            {
                let state = app.state::<Mutex<UpdaterState>>();
                let mut guard = state.lock().unwrap();
                guard.pending_version = Some(version.clone());
                guard.downloaded_bytes = None;
            }
            emit_status(
                &app,
                json!({ "state": "available", "version": version, "releaseNotes": notes }),
            );
            json!({ "available": true, "version": version })
        }
        Ok(None) => {
            emit_status(&app, json!({ "state": "not-available" }));
            json!({ "available": false })
        }
        Err(e) => {
            let msg = e.to_string();
            // Provide helpful error messages for common issues
            let user_msg = if msg.contains("missing field") && msg.contains("signature") {
                "Update check failed: Release is not properly signed. Please check UPDATER_SETUP.md for instructions.".to_string()
            } else if msg.contains("invalid encoding") || msg.contains("minisign") {
                "Update check failed: Signature file is corrupted or invalid. Please regenerate signatures.".to_string()
            } else if msg.contains("404") || msg.contains("not found") {
                "Update check failed: No releases found. Make sure latest.json exists in GitHub releases.".to_string()
            } else {
                format!("Update check failed: {}", msg)
            };
            emit_status(&app, json!({ "state": "error", "message": user_msg }));
            json!({ "error": user_msg })
        }
    }
}

/// Download the pending update with progress events.
/// Emits downloading(percent) → downloaded(version) | error.
#[tauri::command]
pub async fn updater_download(app: AppHandle) -> Value {
    let version = {
        let state = app.state::<Mutex<UpdaterState>>();
        let guard = state.lock().unwrap();
        guard.pending_version.clone()
    };

    let Some(version) = version else {
        return json!({ "error": "no pending update — call updater_check first" });
    };

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => return json!({ "error": e.to_string() }),
    };

    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => return json!({ "error": "update no longer available" }),
        Err(e) => return json!({ "error": e.to_string() }),
    };

    let app_clone = app.clone();
    let bytes = update
        .download(
            move |downloaded, total| {
                let percent = total
                    .map(|t| (downloaded as f64 / t as f64 * 100.0) as u32)
                    .unwrap_or(0);
                emit_status(&app_clone, json!({ "state": "downloading", "percent": percent }));
            },
            || {},
        )
        .await;

    match bytes {
        Ok(b) => {
            let state = app.state::<Mutex<UpdaterState>>();
            state.lock().unwrap().downloaded_bytes = Some(b);
            emit_status(&app, json!({ "state": "downloaded", "version": version }));
            json!({ "downloaded": true })
        }
        Err(e) => {
            let msg = e.to_string();
            let user_msg = if msg.contains("invalid encoding") || msg.contains("minisign") {
                "Download failed: Signature verification failed. The update file may be corrupted.".to_string()
            } else if msg.contains("404") || msg.contains("not found") {
                "Download failed: Update file not found in GitHub releases.".to_string()
            } else if msg.contains("timeout") || msg.contains("timed out") {
                "Download failed: Connection timed out. Please check your internet connection.".to_string()
            } else {
                format!("Download failed: {}", msg)
            };
            emit_status(&app, json!({ "state": "error", "message": user_msg }));
            json!({ "error": user_msg })
        }
    }
}

/// Install the downloaded update and relaunch.
#[tauri::command]
pub async fn updater_install(app: AppHandle) -> Value {
    let bytes = {
        let state = app.state::<Mutex<UpdaterState>>();
        let mut guard = state.lock().unwrap();
        guard.downloaded_bytes.take()
    };

    let Some(bytes) = bytes else {
        return json!({ "error": "no downloaded update — call updater_download first" });
    };

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => return json!({ "error": e.to_string() }),
    };

    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => return json!({ "error": "update no longer available" }),
        Err(e) => return json!({ "error": e.to_string() }),
    };

    match update.install(bytes) {
        Ok(()) => {
            app.restart(); // never returns
        }
        Err(e) => {
            let msg = e.to_string();
            emit_status(&app, json!({ "state": "error", "message": msg }));
            json!({ "error": msg })
        }
    }
}

// ── Background polling ────────────────────────────────────────────────────────

/// Silent check 10 s after launch, then every 4 h — mirrors Electron's
/// updater.ts schedule.
pub fn setup_auto_check(app: &AppHandle) {
    let app_10s = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        silent_check(&app_10s).await;

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(4 * 60 * 60));
        loop {
            interval.tick().await;
            silent_check(&app_10s).await;
        }
    });
}

async fn silent_check(app: &AppHandle) {
    if let Ok(updater) = app.updater() {
        if let Ok(Some(update)) = updater.check().await {
            let version = update.version.clone();
            let notes = update.body.clone();
            {
                let state = app.state::<Mutex<UpdaterState>>();
                let mut guard = state.lock().unwrap();
                guard.pending_version = Some(version.clone());
            }
            emit_status(
                app,
                json!({ "state": "available", "version": version, "releaseNotes": notes }),
            );
        }
    }
}
