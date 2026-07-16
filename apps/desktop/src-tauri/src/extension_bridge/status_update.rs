//! "Mark as applied" (`status.update` → `status.result`) — the narrowest
//! possible WRITE the bridge supports: `saved → applied` on an EXACT
//! normalized-url match, and nothing else. Split out of `mod.rs` to keep that
//! module under the R8 hard LOC cap (`tests/architecture.rs`); mirrors
//! `resolve_applied_check`/`handle_applied_check`'s pure/impure split in the
//! parent module — `resolve_status_update` takes no `AppHandle` so it stays
//! directly unit-testable (see `import_tests.rs`), while `handle_status_update`
//! does the app-stateful notify/emit tail.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::{msg, BridgeState};
use crate::applications::{normalize_job_url, ApplicationStatus, ApplicationStore};
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, APPLICATIONS_CHANGED};

/// Refusal text when an AUTOMATED (`auto: true`) `status.update` arrives while
/// the auto-track opt-in is off — an actionable, fixed sentinel (no dynamic /
/// path / PII content on the wire). The extension folds this into a silent
/// no-op (an auto write is a passive background action, not a user click), but
/// the desktop still returns a clear reason for its own log.
const AUTOTRACK_OFF_MESSAGE: &str =
    "Auto-track is off. Turn it on in AI Job Hunter → Settings → Accounts → Browser extension.";

/// The `auto` flag on a `status.update` payload (default false when absent) —
/// true marks the AUTOMATED Task-#22 write from the gesture submit-watcher, as
/// opposed to a deliberate popup "Mark as applied" click.
pub(super) fn is_auto_status_update(payload: &Value) -> bool {
    payload
        .get("auto")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Whether an AUTO `status.update` must be REFUSED: an auto-flagged write is
/// honored only while the auto-track opt-in is on. A non-auto (deliberate popup
/// click) write is never refused here — it keeps its existing ungated behavior.
/// Defense-in-depth: the extension also gates ARMING client-side, but a
/// compromised extension must not auto-write `applied` without the user's opt-in.
pub(super) fn auto_write_refused(payload: &Value, autotrack_enabled: bool) -> bool {
    is_auto_status_update(payload) && !autotrack_enabled
}

/// The `status.update` outcome: the transitioned Application's id + new status
/// (always `"applied"`), plus a title/company snapshot used ONLY to name the
/// Notification Center card in [`handle_status_update`] — NOT sent over the
/// wire (see [`status_result_reply`], which sends `{ applicationId, status }`
/// alone). The snapshot is already-stored, previously-trusted data (the same
/// title/company already shown on the Applications page), not fresh page
/// content.
#[derive(Debug)]
pub(super) struct StatusUpdateOk {
    pub(super) application_id: String,
    pub(super) status: String,
    pub(super) title: String,
    pub(super) company: String,
}

/// Build the `status.update` reply. UNLIKE `applied_result_reply`, this verb's
/// errors ARE user-facing (a `status.update` answers a deliberate click, not a
/// passive background check) — the popup must render the `error` text, never
/// fold it into a silent no-op.
pub(super) fn status_result_reply(req_id: &str, outcome: AppResult<StatusUpdateOk>) -> String {
    let payload = match outcome {
        Ok(ok) => json!({
            "ok": true,
            "applicationId": ok.application_id,
            "status": ok.status,
        }),
        // Wire-error discipline: this must stay fixed sentinel text (no dynamic/path/PII
        // content) — detailed context belongs in the desktop log, not on the wire.
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    };
    json!({
        "type": msg::STATUS_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Core `status.update`: normalize the `url` field the SAME way
/// `resolve_applied_check`/`handle_import` do, then perform the ONE transition
/// this verb can ever perform — `saved → applied` on an EXACT normalized-url
/// match. Every other combination is a refusal, never a write:
///   - `to` must literal-match `"applied"` — re-validated here independently
///     of the TS zod literal (TS is spec, Rust is the boundary).
///   - the row must exist for the exact normalized url (no fuzzy match, no
///     create-on-miss).
///   - the row's CURRENT status must be exactly `saved` (no transition out of
///     any other stage, no re-applying an already-applied row).
///
/// The actual guard is [`ApplicationStore::transition_status_if`]'s atomic
/// compare-and-set (`UPDATE ... WHERE id=? AND status=?` in one
/// lock/transaction) — the `find_by_job_url` + status-check below is
/// presentation only (it distinguishes "no match" from "not saved" for the
/// user-facing message), so a status change racing between that read and the
/// write can never be silently overwritten. The CAS appends the status event
/// with a short fixed note ("via extension" — no page-derived text, per the
/// untrusted-text discipline) and sets `applied_at` in the SAME transaction.
pub(super) fn resolve_status_update(
    store: &ApplicationStore,
    payload: &Value,
) -> AppResult<StatusUpdateOk> {
    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }
    let to = payload.get("to").and_then(|v| v.as_str()).unwrap_or("");
    if to != "applied" {
        return Err(AppError::Validation(
            "unsupported status transition".to_string(),
        ));
    }

    let canonical = crate::scraping::scrape_url::canonical_job_url(&url);
    let effective_url = canonical.as_deref().unwrap_or(url.as_str());
    let normalized = normalize_job_url(effective_url);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }

    let app = store.find_by_job_url(&normalized).ok_or_else(|| {
        AppError::Validation("couldn't find a saved job for this page".to_string())
    })?;
    if app.status != ApplicationStatus::Saved {
        return Err(AppError::Validation(
            "this job is no longer saved — its status already moved on".to_string(),
        ));
    }

    // Compare-and-set: re-checks the current status atomically (one
    // lock/transaction) instead of trusting the read above, which could be
    // stale by the time we write. `Ok(false)` means the guard lost the race
    // (status moved on between the read and here) — refuse with the same
    // user-facing sentinel as the pre-check above, never a partial write.
    let transitioned = store
        .transition_status_if(
            &app.id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .map_err(|e| {
            // Wire-error discipline: never let a raw store error (path/SQL
            // detail) reach `status_result_reply` — log it, reply a fixed
            // sentinel.
            log::warn!("[extension_bridge] status.update store error: {e}");
            AppError::Storage("could not update this application".to_string())
        })?;
    if !transitioned {
        return Err(AppError::Validation(
            "this job is no longer saved — its status already moved on".to_string(),
        ));
    }

    Ok(StatusUpdateOk {
        application_id: app.id,
        status: ApplicationStatus::Applied.as_id().to_string(),
        title: app.title,
        company: app.company,
    })
}

/// Answer an authenticated `status.update`: resolve the `saved → applied`
/// transition against the local `ApplicationStore`, and on success also drop a
/// Notification Center record + refresh the Applications view (mirrors
/// `handle_import`'s notify tail) so the change is visible without reopening
/// the app. No consent gate — a deliberate click writing to the user's own
/// local store on an exact match, strictly narrower than the already-ungated
/// `import.request{applied:true}` (see the doc note on [`msg::STATUS_UPDATE`]).
pub(super) fn handle_status_update(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    // Defense-in-depth (Task #22): an AUTOMATED write (`auto: true`) is honored
    // ONLY when the auto-track opt-in is on. A deliberate popup click (no `auto`
    // flag) stays ungated exactly as before — this refuses nothing for it.
    let autotrack_enabled = app
        .try_state::<BridgeState>()
        .map(|s| s.autotrack_enabled())
        .unwrap_or(false);
    if auto_write_refused(payload, autotrack_enabled) {
        return status_result_reply(
            req_id,
            Err(AppError::Validation(AUTOTRACK_OFF_MESSAGE.to_string())),
        );
    }
    let auto = is_auto_status_update(payload);

    let outcome = app
        .try_state::<ApplicationStore>()
        .ok_or_else(|| AppError::Config("applications store unavailable".to_string()))
        .and_then(|store| resolve_status_update(store.inner(), payload));

    if let Ok(ok) = &outcome {
        let display_name = if ok.title.trim().is_empty() {
            ok.company.clone()
        } else {
            ok.title.clone()
        };
        emit_event(
            app,
            APPLICATIONS_CHANGED,
            json!({
                "applicationId": ok.application_id.clone(),
                "status": ok.status.clone(),
            }),
        );
        let mut search = serde_json::Map::new();
        search.insert(
            "highlight".to_string(),
            Value::String(ok.application_id.clone()),
        );
        crate::commands::notifications::push_and_notify(
            app,
            crate::notifications::NewNotification {
                kind: "status.update".to_string(),
                title: format!("Marked \"{display_name}\" as applied"),
                body: if auto {
                    "auto-tracked on a detected form submit".to_string()
                } else {
                    "via the browser extension".to_string()
                },
                route: Some(crate::notifications::NotificationRoute {
                    to: "/applications".to_string(),
                    search: Some(search),
                }),
            },
            crate::commands::notifications::OsBanner::WhenUnfocused,
        );
    }

    status_result_reply(req_id, outcome)
}
