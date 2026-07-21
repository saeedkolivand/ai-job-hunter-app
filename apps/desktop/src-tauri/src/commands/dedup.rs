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
mod tests {
    use super::{clamp_split_request, MAX_DEDUP_KEY_BYTES, MAX_OTHER_KEYS};

    #[test]
    fn clamp_caps_other_keys_at_the_limit() {
        // A caller that bypassed the Zod `.max(32)` sends 100 keys → clamped to
        // 32, so the insert is bounded (CWE-770).
        let others: Vec<String> = (0..100).map(|i| format!("k{i}")).collect();
        let (member, clamped) = clamp_split_request("member", &others).expect("valid request");
        assert_eq!(member, "member");
        assert_eq!(
            clamped.len(),
            MAX_OTHER_KEYS,
            "other_keys must be capped at the server limit"
        );
    }

    #[test]
    fn clamp_trims_drops_blanks_and_self_pairs() {
        let mixed = vec![
            "  ".to_string(),     // blank → dropped
            "member".to_string(), // self-pair → dropped
            "  real  ".to_string(),
        ];
        let (_, clamped) = clamp_split_request("member", &mixed).expect("valid request");
        assert_eq!(
            clamped,
            vec!["real".to_string()],
            "trimmed, blank + self dropped"
        );
    }

    #[test]
    fn clamp_deduplicates_repeated_keys_before_the_cap() {
        // 33 entries where two are duplicates (k0 and k1 each appear twice) → 31
        // DISTINCT keys. De-dup runs BEFORE the 32-cap, so all 31 are kept
        // (first-seen order) and a repeated key never wastes a slot.
        let mut keys: Vec<String> = (0..31).map(|i| format!("k{i}")).collect();
        keys.push("k0".to_string()); // duplicate
        keys.push("k1".to_string()); // duplicate
        assert_eq!(keys.len(), 33);

        let (_, clamped) = clamp_split_request("member", &keys).expect("valid request");
        assert_eq!(
            clamped.len(),
            31,
            "the two duplicate keys must not consume slots — 31 distinct pairs"
        );
        // No duplicates survive, and first-seen order is preserved.
        let unique: std::collections::HashSet<&String> = clamped.iter().collect();
        assert_eq!(unique.len(), clamped.len(), "no duplicate key survives");
        assert_eq!(clamped.first().map(String::as_str), Some("k0"));
    }

    #[test]
    fn clamp_byte_caps_oversized_keys() {
        let big_member = "m".repeat(500);
        let big_other = "o".repeat(500);
        let (member, clamped) =
            clamp_split_request(&big_member, &[big_other]).expect("valid request");
        assert!(
            member.len() <= MAX_DEDUP_KEY_BYTES,
            "member is byte-clamped"
        );
        assert_eq!(clamped.len(), 1);
        assert!(
            clamped[0].len() <= MAX_DEDUP_KEY_BYTES,
            "other key is byte-clamped"
        );
    }

    #[test]
    fn clamp_byte_cap_cuts_on_a_utf8_char_boundary() {
        // A multi-byte (UTF-8) key well over the cap must be truncated on a char
        // boundary — never mid-codepoint (`String::truncate` would PANIC if the
        // char-boundary walk-back in `clamp_bytes` regressed). Mirrors the
        // salary-field clamp regression net (job_preferences/test.rs).
        let euros = "€".repeat(150); // 150 × 3 bytes = 450 bytes, over the 200 cap
                                     // member path
        let (member, _) = clamp_split_request(&euros, &["distinct".to_string()])
            .expect("a distinct other key keeps this a valid request");
        assert!(member.len() <= MAX_DEDUP_KEY_BYTES, "member byte-clamped");
        assert!(
            member.is_char_boundary(member.len()),
            "member clamp must cut on a char boundary (valid UTF-8)"
        );
        // other_keys path
        let (_, clamped) = clamp_split_request("member", &[euros]).expect("valid request");
        assert_eq!(clamped.len(), 1);
        assert!(
            clamped[0].len() <= MAX_DEDUP_KEY_BYTES,
            "other key byte-clamped"
        );
        assert!(
            clamped[0].is_char_boundary(clamped[0].len()),
            "other-key clamp must cut on a char boundary (valid UTF-8)"
        );
    }

    #[test]
    fn clamp_rejects_empty_member_or_no_usable_others() {
        // Empty/whitespace member → nothing to record.
        assert!(clamp_split_request("   ", &["k".to_string()]).is_none());
        // Others all blank / only a self-pair → nothing to record.
        assert!(clamp_split_request("member", &["  ".to_string()]).is_none());
        assert!(clamp_split_request("member", &["member".to_string()]).is_none());
    }
}
