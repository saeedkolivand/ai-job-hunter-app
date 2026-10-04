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

/// Max `otherKeys` accepted per split, mirroring the Zod `.max(32)` bound at the
/// SERVER trust boundary (defense-in-depth against a caller that bypasses the
/// renderer's validation — CWE-770 unbounded insert).
const MAX_OTHER_KEYS: usize = 32;
/// Per-key byte cap, same ~200-byte convention as `job_preferences`'s
/// `clamp_agency_list`. A real `canonical_job_key` (a normalized URL) sits well
/// under this; a pathological over-cap key is truncated, not inserted whole.
const MAX_DEDUP_KEY_BYTES: usize = 200;

/// Clamp `s` to at most `max` bytes, cutting on a UTF-8 char boundary — the same
/// discipline as `job_preferences::clamp_bytes`.
fn clamp_bytes(mut s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}

/// Normalize an untrusted split request at the command boundary (CWE-770):
/// trim + byte-clamp `member_key`, then trim / drop-blank / byte-clamp each
/// `other_key`, drop self-pairs, DE-DUPLICATE (first-seen order preserved) so a
/// repeated key can't waste a slot, and cap the count at [`MAX_OTHER_KEYS`].
/// Returns `None` when there is nothing usable to record (empty member, or no
/// usable others) so the command no-ops instead of doing an unbounded/junk
/// insert. Pure (no `AppHandle`) so it is unit-tested directly.
fn clamp_split_request(member_key: &str, other_keys: &[String]) -> Option<(String, Vec<String>)> {
    let member = clamp_bytes(member_key.trim().to_string(), MAX_DEDUP_KEY_BYTES);
    if member.is_empty() {
        return None;
    }
    // De-dup BEFORE the count cap: an insert is idempotent (`INSERT OR IGNORE`),
    // so a repeated key would otherwise consume one of the 32 slots for nothing.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let others: Vec<String> = other_keys
        .iter()
        .map(|k| clamp_bytes(k.trim().to_string(), MAX_DEDUP_KEY_BYTES))
        .filter(|k| !k.is_empty() && *k != member)
        .filter(|k| seen.insert(k.clone()))
        .take(MAX_OTHER_KEYS)
        .collect();
    if others.is_empty() {
        return None;
    }
    Some((member, others))
}

/// Record a "not a duplicate" verdict: insert pair tombstones between
/// `memberKey` and each of `otherKeys`, then re-cluster the live postings cache
/// and — when `autopilotId` is present — that autopilot record's found-jobs, so
/// the split is reflected everywhere it's shown.
#[tauri::command]
pub async fn dedup_mark_not_duplicate(app: AppHandle, req: DedupMarkNotDuplicateRequest) -> Value {
    let Some(store) = app.try_state::<crate::dedup::DedupStore>() else {
        return json!({ "error": "dedup store unavailable" });
    };

    // Server-side clamp (never trust the renderer's Zod bounds alone): a caller
    // that bypasses validation can't drive an unbounded insert. A request with
    // nothing usable after clamping is a no-op success (idempotent).
    let Some((member_key, other_keys)) = clamp_split_request(&req.member_key, &req.other_keys)
    else {
        return json!({ "success": true });
    };

    // member × others pairs. The store additionally canonicalizes ordering,
    // de-dups, and skips a self-pair, so we can hand them straight over.
    let pairs: Vec<(String, String)> = other_keys
        .iter()
        .map(|other| (member_key.clone(), other.clone()))
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

#[cfg(test)]
mod tests;
