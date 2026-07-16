//! "Have I already applied?" (`applied.check` → `applied.result`) — a pure,
//! read-only lookup against the local `ApplicationStore`. Split out of
//! `mod.rs` to keep that module under the R8 hard LOC cap
//! (`tests/architecture.rs`); mirrors `status_update`'s pure/impure split —
//! `resolve_applied_check` takes no `AppHandle` so it stays directly
//! unit-testable (see `import_tests.rs`), while `handle_applied_check` does
//! the app-stateful store lookup. No consent gate (unlike `profile.get`) —
//! this is the user's own metadata, device-local, loopback only.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::msg;
use crate::applications::{normalize_job_url, ApplicationStore};
use crate::error::{AppError, AppResult};

/// The `applied.check` outcome — see [`msg::APPLIED_CHECK`] docs. Read-only:
/// this only reads the existing [`ApplicationStore`] row for the normalized
/// url; it never fetches, scrapes, or writes anything.
#[derive(Debug)]
pub(super) struct AppliedCheckOk {
    pub(super) found: bool,
    pub(super) application_id: Option<String>,
    pub(super) status: Option<String>,
    pub(super) title: Option<String>,
    pub(super) applied_at: Option<u64>,
}

/// Build the `applied.result` envelope (success or error). Mirrors
/// `super::profile_result_reply`/`super::import_flow::result_reply` for the
/// sibling verbs.
pub(super) fn applied_result_reply(req_id: &str, outcome: AppResult<AppliedCheckOk>) -> String {
    let payload = match outcome {
        Ok(ok) => {
            let mut obj = serde_json::Map::new();
            obj.insert("found".to_string(), json!(ok.found));
            if let Some(v) = ok.application_id {
                obj.insert("applicationId".to_string(), json!(v));
            }
            if let Some(v) = ok.status {
                obj.insert("status".to_string(), json!(v));
            }
            if let Some(v) = ok.title {
                obj.insert("title".to_string(), json!(v));
            }
            if let Some(v) = ok.applied_at {
                obj.insert("appliedAt".to_string(), json!(v));
            }
            Value::Object(obj)
        }
        // Wire-error discipline: this must stay fixed sentinel text (no dynamic/path/PII
        // content) — detailed context belongs in the desktop log, not on the wire.
        Err(e) => json!({ "found": false, "error": e.to_string() }),
    };
    json!({
        "type": msg::APPLIED_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Core `applied.check`: normalize the `url` field the SAME way `handle_import`
/// does (the canonical SPA/list-view rewrite, then [`normalize_job_url`]) so a
/// check against the active tab's URL resolves to the exact identity an import
/// would have used — then looks up any existing Application for it. Pure
/// read-only store lookup: no fetch, no SSRF host gate (there is nothing to
/// fetch), and it never creates, merges, or advances a row.
pub(super) fn resolve_applied_check(
    store: &ApplicationStore,
    payload: &Value,
) -> AppResult<AppliedCheckOk> {
    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }

    let canonical = crate::scraping::scrape_url::canonical_job_url(&url);
    let effective_url = canonical.as_deref().unwrap_or(url.as_str());
    let normalized = normalize_job_url(effective_url);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }

    Ok(match store.find_by_job_url(&normalized) {
        Some(app) => AppliedCheckOk {
            found: true,
            application_id: Some(app.id),
            status: Some(app.status.as_id().to_string()),
            title: (!app.title.trim().is_empty()).then_some(app.title),
            applied_at: app.applied_at,
        },
        None => AppliedCheckOk {
            found: false,
            application_id: None,
            status: None,
            title: None,
            applied_at: None,
        },
    })
}

/// Answer an authenticated `applied.check`: resolve against the local
/// `ApplicationStore` and return a ready-to-send `applied.result` reply. No
/// consent gate (unlike `profile.get`) — this is the user's own metadata,
/// device-local, loopback only.
pub(super) fn handle_applied_check(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let outcome = app
        .try_state::<ApplicationStore>()
        .ok_or_else(|| AppError::Config("applications store unavailable".to_string()))
        .and_then(|store| resolve_applied_check(store.inner(), payload));
    applied_result_reply(req_id, outcome)
}
