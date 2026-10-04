use parking_lot::Mutex;
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
///   { state: "managed",       by: "msstore" | "snap" }   — packaged build, flavour-specific
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
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::events::{emit_event, UPDATER_STATUS};
use tauri_plugin_updater::{Update, UpdaterExt};

mod changelog;
mod pre_update_backup;
mod replies;

use replies::{managed_status, store_managed_refusal};
pub(crate) use replies::{status_reply, store_managed};

/// Holds the pending Update and downloaded bytes between commands.
#[derive(Default)]
pub struct UpdaterState {
    /// The Update object returned by check(). Stored so download/install don't re-fetch.
    pub pending_update: Option<Arc<Update>>,
    /// Version string for UI display (mirrors pending_update.version).
    pub pending_version: Option<String>,
    /// Raw bytes from the last successful download.
    pub downloaded_bytes: Option<Vec<u8>>,
    /// Set for the lifetime of one `updater_download` call (cleared by
    /// [`DownloadGuard`] on every exit path, including a panic). Lets
    /// `updater_check`/`updater_download` recognize "a transfer is already
    /// running" without polling the update plugin — the single re-entrancy
    /// flag both commands share.
    pub downloading: bool,
    /// Set once a network check (`updater_check` or the automatic
    /// `silent_check`) has actually COMPLETED — on either a found update or
    /// a confirmed "none available", never on an error, which leaves this
    /// untouched rather than claiming a fresh answer it doesn't have.
    /// [`status_reply`] reads this so a read-only caller can tell "checked,
    /// genuinely current" apart from "no check has ever run" / "the last
    /// one failed" — both of which stayed the exact same `{"available":
    /// false}` before this field existed (`B1-r2-ACLI-R6-2`).
    pub checked: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn emit_status(app: &AppHandle, status: Value) {
    emit_event(app, UPDATER_STATUS, status);
}

/// Whether a download for the pending update has already finished or is
/// still running — the single predicate both `updater_check` (don't discard
/// it) and `updater_download` (don't start a second one) key their guard on.
/// Split out so it is testable against a plain [`UpdaterState`], without a
/// live `AppHandle` (this crate has no `tauri::test` mock-app harness).
fn download_in_progress_or_done(state: &UpdaterState) -> bool {
    state.downloading || state.downloaded_bytes.is_some()
}

/// How long after launch the first status is pushed. The silent check has
/// always waited this out so it does not compete with startup; the packaged-
/// build announcement reuses it for a second reason — `updater:status`
/// events are not replayed, so anything emitted before the webview mounts
/// its listeners is simply lost.
const STARTUP_STATUS_DELAY: tokio::time::Duration = tokio::time::Duration::from_secs(10);

// ── Commands ──────────────────────────────────────────────────────────────────

/// The last-known update state — whatever `updater_check` or the automatic
/// `silent_check` (10s after launch, then every 4h) already found — with no
/// network call and no `updater:status` emission. Exists so a read-only
/// caller (the agent-cli `Read` tier) can answer "is an update available"
/// without triggering `updater_check`'s network probe/event, which selects
/// the install target the rest of the check→download→install flow acts on
/// (issue #1165's follow-up: `updater_check` itself stays `Effect::Reversible`
/// for exactly that reason — see its POLICY row comment).
///
/// Agent-tier only, by design (round-4 advisory T6, PR #1182): registered in
/// `generate_handler!` so it is reachable from the webview like any other
/// command, but no `ipc/contracts/` entry, `tauri-client/` binding, or
/// `services/` hook exists for it (AGENTS.md rule 14) — the renderer has no
/// caller for a bare-status read with no accompanying network probe/event,
/// so a webview-side contract half would exist for nobody.
#[tauri::command]
pub fn updater_status(app: AppHandle) -> Value {
    let state = app.state::<Mutex<UpdaterState>>();
    status_reply(&state.lock(), crate::platform::packaged_flavour())
}

/// Check for an available update.
/// Emits checking → available(version) | not-available | error.
/// Stores the Update object for use by updater_download.
#[tauri::command]
pub async fn updater_check(app: AppHandle) -> Value {
    // Before the network, before the state: a packaged (Store/Snap)
    // build never checks GitHub at all. Emitted as well as returned so every
    // mounted listener (banner, settings panel, menu) converges on the same
    // answer, exactly like the outcomes below.
    if let Some(flavour) = crate::platform::packaged_flavour() {
        emit_status(&app, managed_status(flavour));
        return store_managed(flavour);
    }

    // A finished or in-flight download must never be thrown away by a fresh
    // check. `check()` below unconditionally replaces `pending_update` and
    // resets `downloaded_bytes` to `None` on success — correct the FIRST
    // time, but a returning caller (a remounted route, or a stale "Check now"
    // button rendered over work that is already running) would otherwise
    // discard an already-downloaded release — forcing a full re-download —
    // or swap the `Update` object out from under a transfer still in flight.
    // Report the state that is already known instead of re-fetching: the
    // caller wants to re-attach, not restart.
    {
        let state = app.state::<Mutex<UpdaterState>>();
        let guard = state.lock();
        if let Some(version) = guard.pending_version.clone() {
            if download_in_progress_or_done(&guard) {
                return json!({
                    "available": true,
                    "version": version,
                    "downloaded": guard.downloaded_bytes.is_some(),
                    "downloading": guard.downloading,
                });
            }
        }
    }

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
                let mut guard = state.lock();
                guard.pending_version = Some(version.clone());
                guard.pending_update = Some(Arc::new(update));
                guard.downloaded_bytes = None;
                guard.checked = true;
            }
            emit_status(
                &app,
                json!({ "state": "available", "version": version, "releaseNotes": notes }),
            );
            json!({ "available": true, "version": version })
        }
        Ok(None) => {
            app.state::<Mutex<UpdaterState>>().lock().checked = true;
            emit_status(&app, json!({ "state": "not-available" }));
            json!({ "available": false })
        }
        Err(e) => {
            let msg = e.to_string();
            let user_msg = if msg.contains("missing field") && msg.contains("signature") {
                "Update check failed: Release is not properly signed. See docs/DEPLOYMENT.md (Updater signing keys).".to_string()
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

/// Clears [`UpdaterState::downloading`] when it drops — on the success
/// return, the error return, OR a panic unwind mid-transfer — so a download
/// that dies partway through can never leave the flag stuck `true` and
/// permanently refuse every future `updater_download` call.
struct DownloadGuard(AppHandle);

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        if let Some(state) = self.0.try_state::<Mutex<UpdaterState>>() {
            state.lock().downloading = false;
        }
    }
}

/// Download the pending update with progress events.
/// Uses the Update object stored by updater_check — no re-fetch.
/// Emits downloading(percent) → downloaded(version) | error.
///
/// Not re-entrant: a second call while one is already in flight (or one
/// already finished) refuses rather than starting a second transfer of the
/// same release — progress/completion is broadcast on `updater:status` to
/// every listener regardless of which call started the download, so a
/// second caller has nothing useful to do but wait.
#[tauri::command]
pub async fn updater_download(app: AppHandle) -> Value {
    if let Some(flavour) = crate::platform::packaged_flavour() {
        return store_managed_refusal(flavour);
    }
    let (update, version) = {
        let state = app.state::<Mutex<UpdaterState>>();
        let mut guard = state.lock();
        match (guard.pending_update.clone(), guard.pending_version.clone()) {
            (Some(u), Some(v)) => {
                // Checked and claimed under the SAME lock as the reads above,
                // so two concurrent calls can't both observe "not downloading"
                // before either sets the flag — the check-then-act race
                // `job_start_exclusive` closes for jobs; this is the
                // updater's version of the same fix, over a plain bool
                // instead of the job tracker.
                if download_in_progress_or_done(&guard) {
                    return json!({
                        "version": v,
                        "downloaded": guard.downloaded_bytes.is_some(),
                        "downloading": guard.downloading,
                    });
                }
                guard.downloading = true;
                (u, v)
            }
            _ => return json!({ "error": "no pending update — call updater_check first" }),
        }
    };
    let _guard = DownloadGuard(app.clone());

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
            state.lock().downloaded_bytes = Some(b);
            emit_status(&app, json!({ "state": "downloaded", "version": version }));
            json!({ "downloaded": true })
        }
        Err(e) => {
            let msg = e.to_string();
            let user_msg = if msg.contains("invalid encoding") || msg.contains("minisign") {
                "Download failed: Signature verification failed. The update file may be corrupted."
                    .to_string()
            } else if msg.contains("404") || msg.contains("not found") {
                "Download failed: Update file not found in GitHub releases.".to_string()
            } else if msg.contains("timeout") || msg.contains("timed out") {
                "Download failed: Connection timed out. Please check your internet connection."
                    .to_string()
            } else {
                format!("Download failed: {}", msg)
            };
            emit_status(&app, json!({ "state": "error", "message": user_msg }));
            json!({ "error": user_msg })
        }
    }
}

/// Copy the user's data to `backups/` before the new version can touch it
/// (#1278). Never blocks the update: a failure is logged and the install goes
/// ahead, because holding back an update also holds back its security fixes.
/// Only logged, not shown: the app restarts right after, so nobody would see it.
async fn back_up_before_install(app: &AppHandle) {
    let app = app.clone();
    // Reading every store can take a while (one has been seen at 41 MB), so it
    // runs off the async runtime.
    let result = tokio::task::spawn_blocking(move || {
        let bundle = crate::commands::data::build_bundle(&app);
        pre_update_backup::write_pre_update_backup(
            &crate::platform::config::data_dir(),
            env!("CARGO_PKG_VERSION"),
            &crate::commands::data::date_stamp(),
            &bundle,
        )
    })
    .await;
    match result {
        Ok(Ok(path)) => log::info!(
            "[updater] pre-update backup written: {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ),
        Ok(Err(e)) => log::warn!(
            "[updater] pre-update backup failed, installing anyway: {}",
            crate::observability::sanitize_reason(&e.to_string())
        ),
        Err(e) => log::warn!(
            "[updater] pre-update backup task failed, installing anyway: {}",
            crate::observability::sanitize_reason(&e.to_string())
        ),
    }
}

/// Install the downloaded update and relaunch.
/// Uses the Update object and bytes stored by earlier commands — no re-fetch.
#[tauri::command]
pub async fn updater_install(app: AppHandle) -> Value {
    if let Some(flavour) = crate::platform::packaged_flavour() {
        return store_managed_refusal(flavour);
    }
    let (update, bytes) = {
        let state = app.state::<Mutex<UpdaterState>>();
        let mut guard = state.lock();
        match (guard.pending_update.clone(), guard.downloaded_bytes.take()) {
            (Some(u), Some(b)) => (u, b),
            (None, _) => return json!({ "error": "no pending update — call updater_check first" }),
            (_, None) => {
                return json!({ "error": "no downloaded update — call updater_download first" })
            }
        }
    };

    back_up_before_install(&app).await;

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

/// Recent release history (newest first) for the in-app changelog, parsed from
/// the bundled [`changelog::CHANGELOG_MD`] — no network call. Returns `{ releases: [...] }`
/// or `{ error }` — never panics, so the UI can render a friendly empty/error
/// state.
#[tauri::command]
pub fn updater_changelog() -> Value {
    changelog::changelog_response(changelog::CHANGELOG_MD)
}

// ── Background polling ────────────────────────────────────────────────────────

/// Silent check 10 s after launch, then every 4 h.
///
/// A packaged (Store/Snap) build gets neither: no first check, no
/// interval, no network. It announces once (so the settings panel can say
/// where updates come from without the user pressing anything) and stops
/// there.
pub fn setup_auto_check(app: &AppHandle) {
    if let Some(flavour) = crate::platform::packaged_flavour() {
        let app_announce = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(STARTUP_STATUS_DELAY).await;
            emit_status(&app_announce, managed_status(flavour));
        });
        return;
    }

    let app_10s = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_STATUS_DELAY).await;
        silent_check(&app_10s).await;

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(4 * 60 * 60));
        loop {
            interval.tick().await;
            silent_check(&app_10s).await;
        }
    });
}

async fn silent_check(app: &AppHandle) {
    let Ok(updater) = app.updater() else { return };
    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let notes = update.body.clone();
            {
                let state = app.state::<Mutex<UpdaterState>>();
                let mut guard = state.lock();
                guard.pending_version = Some(version.clone());
                guard.pending_update = Some(Arc::new(update));
                guard.downloaded_bytes = None;
                guard.checked = true;
            }
            emit_status(
                app,
                json!({ "state": "available", "version": version, "releaseNotes": notes }),
            );
        }
        // A confirmed "nothing newer" still counts as a completed check for
        // `status_reply` — before this arm, a silent check that found
        // nothing left `checked` exactly as unset as one that never ran at
        // all, the same collapse `updater_check`'s own `Ok(None)` arm fixes.
        Ok(None) => app.state::<Mutex<UpdaterState>>().lock().checked = true,
        // Swallowed on purpose (this check is silent) — but never marked
        // `checked`, so a caller reading `updater_status` after a failed
        // background probe sees "unknown", not a confident "current".
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests;
