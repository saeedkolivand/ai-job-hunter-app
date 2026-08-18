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

// ── Commands ──────────────────────────────────────────────────────────────────

/// Check for an available update.
/// Emits checking → available(version) | not-available | error.
/// Stores the Update object for use by updater_download.
#[tauri::command]
pub async fn updater_check(app: AppHandle) -> Value {
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

/// Install the downloaded update and relaunch.
/// Uses the Update object and bytes stored by earlier commands — no re-fetch.
#[tauri::command]
pub async fn updater_install(app: AppHandle) -> Value {
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

// ── Changelog (release history) ────────────────────────────────────────────────

/// Most recent releases to surface in the in-app changelog.
const CHANGELOG_LIMIT: usize = 15;

/// The repo's own `CHANGELOG.md`, bundled into the binary at compile time — the
/// changelog now works fully offline and makes no GitHub request (removes an
/// egress path the old per-release API fetch had). `include_str!` makes rustc
/// track the file for rebuilds like any other source dependency, no `build.rs`
/// needed. `@semantic-release/changelog` (`release.config.mjs`) regenerates this
/// file as part of the release commit, before the Tauri build step packages this
/// binary — see the release workflow for the ordering.
const CHANGELOG_MD: &str = include_str!("../../../../../CHANGELOG.md");

/// One version section parsed out of [`CHANGELOG_MD`].
struct ChangelogEntry {
    version: String,
    date: Option<String>,
    body: String,
}

/// Splits a changelog document into per-version entries, in file order (the
/// generator writes newest-first). `@semantic-release/changelog` writes headings
/// shaped `## [x.y.z](compare-url) (YYYY-MM-DD)`; any other line — including a
/// malformed or empty document — simply yields no entry for that line rather
/// than erroring, so a reformatted/corrupted changelog degrades to fewer
/// releases instead of panicking.
fn parse_changelog(raw: &str) -> Vec<ChangelogEntry> {
    let mut entries = Vec::new();
    let mut current: Option<(String, Option<String>, usize)> = None;
    let mut offset = 0usize;

    for line in raw.split_inclusive('\n') {
        if let Some((version, date)) = parse_heading(line) {
            if let Some((v, d, body_start)) = current.take() {
                entries.push(ChangelogEntry {
                    version: v,
                    date: d,
                    body: raw[body_start..offset].trim().to_string(),
                });
            }
            current = Some((version, date, offset + line.len()));
        }
        offset += line.len();
    }
    if let Some((v, d, body_start)) = current {
        entries.push(ChangelogEntry {
            version: v,
            date: d,
            body: raw[body_start..].trim().to_string(),
        });
    }
    entries
}

/// Parses one `## [x.y.z](...) (YYYY-MM-DD)` version heading line. `None` for any
/// other line (subsection headings like `### Features`, prose, blank lines). This
/// also deliberately excludes `@semantic-release/changelog`'s first-ever-release
/// heading shape `## x.y.z` (no `[...]` link, since there's no prior tag to
/// compare against) — harmless in practice, since `CHANGELOG_LIMIT` means the
/// capped, newest-first list never reaches that far back.
fn parse_heading(line: &str) -> Option<(String, Option<String>)> {
    let rest = line.trim_end().strip_prefix("## [")?;
    let (version, rest) = rest.split_once(']')?;
    if !version.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    // Shape is `(compare-url) (YYYY-MM-DD)` — the date is the last `(...)`.
    let date = rest
        .rsplit_once('(')
        .and_then(|(_, tail)| tail.strip_suffix(')'))
        .filter(|d| d.len() == 10)
        .map(str::to_string);
    Some((version.to_string(), date))
}

/// Builds the `updater_changelog` reply from raw changelog text — split out from
/// the command so tests can exercise the malformed/empty path without needing a
/// separate on-disk fixture.
fn changelog_response(raw: &str) -> Value {
    let entries = parse_changelog(raw);
    if entries.is_empty() {
        return json!({ "error": "Changelog unavailable (bundled CHANGELOG.md has no releases)." });
    }

    let items: Vec<Value> = entries
        .into_iter()
        .take(CHANGELOG_LIMIT)
        .map(|e| {
            let url = format!(
                "https://github.com/saeedkolivand/ai-job-hunter-app/releases/tag/v{}",
                e.version
            );
            json!({
                "version": e.version,
                "name": null,
                "body": e.body,
                "publishedAt": e.date,
                "url": url,
                "prerelease": e.version.contains('-'),
            })
        })
        .collect();
    json!({ "releases": items })
}

/// Recent release history (newest first) for the in-app changelog, parsed from
/// the bundled [`CHANGELOG_MD`] — no network call. Returns `{ releases: [...] }`
/// or `{ error }` — never panics, so the UI can render a friendly empty/error
/// state.
#[tauri::command]
pub fn updater_changelog() -> Value {
    changelog_response(CHANGELOG_MD)
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
                let mut guard = state.lock();
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
