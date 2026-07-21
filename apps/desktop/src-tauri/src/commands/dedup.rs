//! Cross-board dedup IPC surface (ADR-029 §h): the single "split" command.
//!
//! `dedup_mark_not_duplicate` records a user "not a duplicate" verdict between a
//! member and one-or-more other cluster members, then recomputes the affected
//! surfaces so the split takes effect immediately. Because clustering is
//! recomputed at every ingest and the veto reads the persisted pair tombstones,
//! the split survives every future re-scrape.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

// Generated from `DedupMarkNotDuplicateRequestSchema` by `pnpm gen:ipc`.
pub use crate::ipc_contracts::dedup::DedupMarkNotDuplicateRequest;

/// Record a "not a duplicate" verdict: insert pair tombstones between
/// `memberKey` and each of `otherKeys`, then re-cluster the live postings cache
/// and — when `autopilotId` is present — that autopilot record's found-jobs, so
/// the split is reflected everywhere it's shown.
#[tauri::command]
pub async fn dedup_mark_not_duplicate(app: AppHandle, req: DedupMarkNotDuplicateRequest) -> Value {
    let Some(store) = app.try_state::<crate::dedup::DedupStore>() else {
        return json!({ "error": "dedup store unavailable" });
    };

    // member × others pairs. The store canonicalizes ordering, de-dups, and
    // skips a self-pair, so we can hand them straight over.
    let pairs: Vec<(String, String)> = req
        .other_keys
        .iter()
        .map(|other| (req.member_key.clone(), other.clone()))
        .collect();
    if let Err(e) = store.insert_pairs(&pairs) {
        return json!({ "error": e.to_string() });
    }

    // Recompute the live postings cache so a manual-scrape split shows at once.
    crate::commands::scrape::recluster_postings_cache(&app);

    // If the split originated from an autopilot found-jobs view, recompute +
    // persist that record's cluster annotations too.
    if let Some(autopilot_id) = req.autopilot_id.as_deref() {
        crate::commands::autopilot::recluster_autopilot_record(&app, autopilot_id);
    }

    json!({ "success": true })
}
