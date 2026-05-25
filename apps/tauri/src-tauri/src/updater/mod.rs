/// Auto-updater for the Tauri shell.
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
///   updater:status  — emitted by every state transition.
///
/// ── Three-step flow ──────────────────────────────────────────────────────────
///   updater_check    → check once, store the Update object, emit available/not-available
///   updater_download → use stored Update to download with progress, store bytes
///   updater_install  → use stored Update + stored bytes to install, then relaunch
///
/// The Update object is stored across commands so the download URL and signature
/// are never re-fetched, avoiding race conditions and unnecessary network calls.
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Holds the pending Update and downloaded bytes between commands.
pub struct UpdaterState {
    /// The Update object returned by check(). Stored so download/install don't re-fetch.
    pub pending_update: Option<Arc<Update>>,
    /// Version string for UI display (mirrors pending_update.version).
    pub pending_version: Option<String>,
    /// Raw bytes from the last successful download.
    pub downloaded_bytes: Option<Vec<u8>>,
}

impl Default for UpdaterState {
    fn default() -> Self {
        Self {
            pending_update: None,
            pending_version: None,
            downloaded_bytes: None,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn emit_status(app: &AppHandle, status: Value) {
    let _ = app.emit("updater:status", status);
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Check for an available update.
/// Emits checking → available(version) | not-available | error.
/// Stores the Update object for use by updater_download.
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
                guard.pending_update = Some(Arc::new(update));
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
            let user_msg = if msg.contains("missing field") && msg.contains("signature") {
                "Update check failed: Release is not properly signed. Please check UPDATER_SETUP.md for instructions.".to_string()
            } else if msg.contains("invalid encoding") || msg.contains("minisign") {
                "Update check failed: Signature file is corrupted or invalid.".to_string()
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
/// Uses the Update object stored by updater_check — no re-fetch.
/// Emits downloading(percent) → downloaded(version) | error.
#[tauri::command]
pub async fn updater_download(app: AppHandle) -> Value {
    let (update, version) = {
        let state = app.state::<Mutex<UpdaterState>>();
        let guard = state.lock().unwrap();
        match (guard.pending_update.clone(), guard.pending_version.clone()) {
            (Some(u), Some(v)) => (u, v),
            _ => return json!({ "error": "no pending update — call updater_check first" }),
        }
    };

    let app_clone = app.clone();
    let bytes = update
        .download(
            move |downloaded, total| {
                let percent = total
                    .map(|t| (downloaded as f64 / t as f64 * 100.0) as u32)
                    .unwrap_or(0);
                emit_status(
                    &app_clone,
                    json!({
                        "state": "downloading",
                        "percent": percent,
                        "downloaded": downloaded,
                        "total": total.unwrap_or(0)
                    }),
                );
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
/// Uses the Update object and bytes stored by earlier commands — no re-fetch.
#[tauri::command]
pub async fn updater_install(app: AppHandle) -> Value {
    let (update, bytes) = {
        let state = app.state::<Mutex<UpdaterState>>();
        let mut guard = state.lock().unwrap();
        match (guard.pending_update.clone(), guard.downloaded_bytes.take()) {
            (Some(u), Some(b)) => (u, b),
            (None, _) => return json!({ "error": "no pending update — call updater_check first" }),
            (_, None) => return json!({ "error": "no downloaded update — call updater_download first" }),
        }
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

/// Silent check 10 s after launch, then every 4 h.
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
                guard.pending_update = Some(Arc::new(update));
                guard.downloaded_bytes = None;
            }
            emit_status(
                app,
                json!({ "state": "available", "version": version, "releaseNotes": notes }),
            );
        }
    }
}

#[cfg(test)]
mod test;
