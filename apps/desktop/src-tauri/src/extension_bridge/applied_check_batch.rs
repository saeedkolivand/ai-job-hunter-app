//! `applied.check.batch` → `applied.batch.result` — PR3 (Check-fit on the page, results-page
//! stamps): the batch form of `applied.check` a results-listing page needs to stamp every visible
//! card in one round trip. New module (own file — every new verb gets its own, per `mod.rs`'s R8
//! doc), mirrors `document_export.rs`'s shape (its own throttle instance on `BridgeState`, its
//! own fixed-sentinel error reply).
//!
//! ## Caller + gate
//! Same trust class as `applied.check` — the user's own device-local metadata, no fetch, no
//! network, no consent gate — so the dispatch arm in `caller_gate::advance_authenticated` stays
//! UNCONDITIONAL, exactly like `msg::APPLIED_CHECK`'s own arm (both CLI and extension reach it).
//! Chosen over gating to a caller class because nothing about this verb's trust profile differs
//! from the single-URL form it batches — it only amplifies the SAME already-ungated read N-fold,
//! which is what the dedicated throttle below exists to bound (unlike `applied.check`, which has
//! none).
//!
//! ## Resolution
//! Reuses [`super::applied_check::resolve_applied_check_url`] — the exact per-URL
//! canonicalize-then-normalize-then-lookup chain `applied.check` itself uses — once per input url,
//! against ONE [`crate::applications::ApplicationStore`] handle for the whole batch (not one
//! lookup context per url). Preserves input order and returns exactly one entry per input url
//! (duplicates included, never deduplicated or merged) — each entry echoes the caller's OWN url
//! string verbatim so the client can map results back onto its DOM without re-normalizing.  Only
//! `status` rides along (no `applicationId`/`title`/`appliedAt`): a results-page stamp needs
//! "saved" or "applied", nothing else.
//!
//! ## Cap
//! [`MAX_BATCH_URLS`] (50) — a single results-listing viewport rarely shows more visible job cards
//! than this, and 50 sequential indexed SQLite lookups is still sub-millisecond local work. Over
//! the cap refuses with the fixed sentinel [`ERR_TOO_MANY_URLS`], never silently truncates — the
//! extension's own results-page collector must cap its candidate list to this SAME constant (PR3
//! spec §B.4) so it never sends an over-cap request in the first place.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::applied_check::resolve_applied_check_url;
use super::msg;
use crate::applications::ApplicationStore;

/// Hard cap on `urls` in one `applied.check.batch` request — see the module doc. The paired
/// extension's results-page collector caps to this SAME value.
pub(super) const MAX_BATCH_URLS: usize = 50;

/// The request carried more than [`MAX_BATCH_URLS`] urls — refused, never truncated.
const ERR_TOO_MANY_URLS: &str = "too_many_urls";
/// `urls` missing, not an array, or containing a non-string entry.
const ERR_INVALID_REQUEST: &str = "invalid_batch_request";
/// The `ApplicationStore` isn't managed (a start-up failure) — mirrors
/// `applied_check::handle_applied_check`'s own `AppError::Config` text verbatim.
const ERR_STORE_UNAVAILABLE: &str = "applications store unavailable";

/// One `applied.check.batch` result entry — see the module doc for why only
/// `url`/`found`/`status` ride along.
struct BatchEntry {
    url: String,
    found: bool,
    status: Option<String>,
}

fn entry_json(e: &BatchEntry) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("url".to_string(), json!(e.url));
    obj.insert("found".to_string(), json!(e.found));
    if let Some(status) = &e.status {
        obj.insert("status".to_string(), json!(status));
    }
    Value::Object(obj)
}

fn error_reply(req_id: &str, error: &str, retry_after_ms: Option<u64>) -> String {
    let mut payload = json!({ "ok": false, "error": error });
    if let Some(ms) = retry_after_ms {
        payload["retryAfterMs"] = json!(ms);
    }
    json!({
        "type": msg::APPLIED_BATCH_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// `applied.check.batch` refused by [`AppliedCheckBatchThrottle`] — `retry_after_ms` comes from
/// [`super::BridgeState::applied_check_batch_retry_after_ms`], read by the ONE caller right after
/// a failed `try_acquire_applied_check_batch` (same discipline as `document_export`'s throttle
/// reply).
pub(super) fn throttled_reply(req_id: &str, retry_after_ms: u64) -> String {
    error_reply(
        req_id,
        super::agent_call::ERR_RATE_LIMITED,
        Some(retry_after_ms),
    )
}

/// Pure parse + validate of the wire request (`{ urls: string[] }`) — no `AppHandle`, directly
/// unit-testable. Distinguishes the over-cap sentinel from every other malformed shape.
fn parse_urls(payload: &Value) -> Result<Vec<String>, &'static str> {
    let arr = payload
        .get("urls")
        .and_then(Value::as_array)
        .ok_or(ERR_INVALID_REQUEST)?;
    if arr.len() > MAX_BATCH_URLS {
        return Err(ERR_TOO_MANY_URLS);
    }
    arr.iter()
        .map(|v| v.as_str().map(str::to_string).ok_or(ERR_INVALID_REQUEST))
        .collect()
}

/// Pure per-batch resolution against `store` — ONE handle for the whole batch (see module doc).
/// Preserves input order; one entry per input url, duplicates included verbatim (never deduped).
/// A per-url resolution failure (an empty/non-http string slipped past `parse_urls`, which only
/// checks "is a string") degrades to `found: false` with no status, mirroring
/// `resolve_applied_check`'s own "never an error for a single lookup" posture — a batch caller
/// gets a result row per url, never a partial failure for the whole request.
fn resolve_applied_check_batch(store: &ApplicationStore, urls: &[String]) -> Vec<BatchEntry> {
    urls.iter()
        .map(|url| match resolve_applied_check_url(store, url) {
            Ok(ok) => BatchEntry {
                url: url.clone(),
                found: ok.found,
                status: ok.status,
            },
            Err(_) => BatchEntry {
                url: url.clone(),
                found: false,
                status: None,
            },
        })
        .collect()
}

fn applied_batch_result_reply(req_id: &str, entries: &[BatchEntry]) -> String {
    let results: Vec<Value> = entries.iter().map(entry_json).collect();
    json!({
        "type": msg::APPLIED_BATCH_RESULT,
        "reqId": req_id,
        "payload": { "ok": true, "results": results },
    })
    .to_string()
}

/// Answer an authenticated, throttle-admitted `applied.check.batch`: parse + validate (own
/// sentinels), then resolve every url against ONE `ApplicationStore` handle and reply
/// `applied.batch.result`.
pub(super) fn handle_applied_check_batch(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let urls = match parse_urls(payload) {
        Ok(u) => u,
        Err(sentinel) => return error_reply(req_id, sentinel, None),
    };
    let Some(store) = app.try_state::<ApplicationStore>() else {
        return error_reply(req_id, ERR_STORE_UNAVAILABLE, None);
    };
    let entries = resolve_applied_check_batch(store.inner(), &urls);
    applied_batch_result_reply(req_id, &entries)
}

// ── Throttle (own instance on `BridgeState`, per pairing — see module doc) ─────────

/// Burst 5, refilling one token every 5s — a "stamp results page" gesture-bound action:
/// occasional re-runs (scroll + re-stamp, a retry) shouldn't throttle immediately, but each call
/// can amplify up to [`MAX_BATCH_URLS`] lookups, so this stays a small dedicated bucket rather
/// than sharing `agent_read::AgentQueryThrottle`'s cheap single-read bucket.
const APPLIED_CHECK_BATCH_BURST: f64 = 5.0;
const APPLIED_CHECK_BATCH_REFILL_SECS: f64 = 5.0;

/// Minimal token bucket — the exact math `document_export::DocumentExportThrottle`/
/// `match_live::MatchLiveThrottle` use; its own instance rather than sharing either (per-verb cost
/// profiles differ — see the module doc).
pub(super) struct AppliedCheckBatchThrottle {
    tokens: f64,
    last: std::time::Instant,
}

impl AppliedCheckBatchThrottle {
    pub(super) fn new() -> Self {
        Self {
            tokens: APPLIED_CHECK_BATCH_BURST,
            last: std::time::Instant::now(),
        }
    }

    fn try_acquire_at(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed / APPLIED_CHECK_BATCH_REFILL_SECS)
            .min(APPLIED_CHECK_BATCH_BURST);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub(super) fn try_acquire(&mut self) -> bool {
        self.try_acquire_at(std::time::Instant::now())
    }

    /// Milliseconds until this bucket would hold one full token — only meaningful called right
    /// after a failed [`Self::try_acquire`] in the same tick (mirrors
    /// `document_export::DocumentExportThrottle::retry_after_ms`).
    pub(super) fn retry_after_ms(&self) -> u64 {
        if self.tokens >= 1.0 {
            return 0;
        }
        let needed_secs = (1.0 - self.tokens) * APPLIED_CHECK_BATCH_REFILL_SECS;
        (needed_secs * 1000.0).ceil() as u64
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::applications::{ApplicationMeta, ApplicationOrigin};

    fn open_store() -> (TempDir, ApplicationStore) {
        let dir = TempDir::new().unwrap();
        let store = ApplicationStore::open(dir.path()).unwrap();
        (dir, store)
    }

    fn app_meta(company: &str, title: &str) -> ApplicationMeta {
        ApplicationMeta {
            company: company.into(),
            title: title.into(),
            candidate: "Test User".into(),
            brief: String::new(),
            job_description: String::new(),
            answers: vec![],
            job_summary: String::new(),
            salary_min: None,
            salary_max: None,
            salary_currency: None,
        }
    }

    // ── parse_urls ────────────────────────────────────────────────────────────

    #[test]
    fn parse_urls_ok_for_a_plain_array() {
        let payload = json!({ "urls": ["https://a.example/1", "https://b.example/2"] });
        let urls = parse_urls(&payload).unwrap();
        assert_eq!(urls, vec!["https://a.example/1", "https://b.example/2"]);
    }

    #[test]
    fn parse_urls_rejects_missing_urls_field() {
        assert_eq!(parse_urls(&json!({})).unwrap_err(), ERR_INVALID_REQUEST);
    }

    #[test]
    fn parse_urls_rejects_non_array_urls() {
        assert_eq!(
            parse_urls(&json!({ "urls": "not-an-array" })).unwrap_err(),
            ERR_INVALID_REQUEST
        );
    }

    #[test]
    fn parse_urls_rejects_a_non_string_entry() {
        assert_eq!(
            parse_urls(&json!({ "urls": ["https://a.example/1", 42] })).unwrap_err(),
            ERR_INVALID_REQUEST
        );
    }

    #[test]
    fn parse_urls_refuses_over_the_cap_without_truncating() {
        let urls: Vec<String> = (0..(MAX_BATCH_URLS + 1))
            .map(|i| format!("https://example.com/{i}"))
            .collect();
        let err = parse_urls(&json!({ "urls": urls })).unwrap_err();
        assert_eq!(err, ERR_TOO_MANY_URLS);
    }

    #[test]
    fn parse_urls_accepts_exactly_the_cap() {
        let urls: Vec<String> = (0..MAX_BATCH_URLS)
            .map(|i| format!("https://example.com/{i}"))
            .collect();
        assert_eq!(
            parse_urls(&json!({ "urls": urls })).unwrap().len(),
            MAX_BATCH_URLS
        );
    }

    // ── resolve_applied_check_batch — order, 1:1 mapping, duplicates, unknowns ──

    #[test]
    fn resolves_each_url_preserving_order_and_1to1_mapping() {
        let (_dir, store) = open_store();
        let saved = "https://jobs.example.com/posting/saved-1";
        let applied = "https://jobs.example.com/posting/applied-1";
        let unknown = "https://jobs.example.com/posting/unknown";
        store
            .upsert_for_origin(
                saved,
                "linkedin",
                &app_meta("Acme", "Engineer"),
                ApplicationOrigin::Saved,
                None,
            )
            .unwrap();
        store
            .upsert_for_origin(
                applied,
                "linkedin",
                &app_meta("Acme", "Staff Engineer"),
                ApplicationOrigin::Saved,
                Some(true),
            )
            .unwrap();

        let urls = vec![unknown.to_string(), saved.to_string(), applied.to_string()];
        let entries = resolve_applied_check_batch(&store, &urls);

        assert_eq!(entries.len(), 3, "one entry per input url");
        assert_eq!(entries[0].url, unknown);
        assert!(!entries[0].found);
        assert_eq!(entries[1].url, saved);
        assert!(entries[1].found);
        assert_eq!(entries[1].status.as_deref(), Some("saved"));
        assert_eq!(entries[2].url, applied);
        assert!(entries[2].found);
        assert_eq!(entries[2].status.as_deref(), Some("applied"));
    }

    #[test]
    fn duplicate_urls_each_get_their_own_result_entry() {
        let (_dir, store) = open_store();
        let url = "https://jobs.example.com/posting/dup-1";
        store
            .upsert_for_origin(
                url,
                "linkedin",
                &app_meta("Acme", "Engineer"),
                ApplicationOrigin::Saved,
                None,
            )
            .unwrap();

        let urls = vec![url.to_string(), url.to_string()];
        let entries = resolve_applied_check_batch(&store, &urls);

        assert_eq!(entries.len(), 2, "duplicates are never deduped");
        assert!(entries[0].found && entries[1].found);
    }

    #[test]
    fn a_malformed_per_url_entry_degrades_to_not_found_never_fails_the_batch() {
        let (_dir, store) = open_store();
        let entries = resolve_applied_check_batch(&store, &["".to_string()]);
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].found);
        assert!(entries[0].status.is_none());
    }

    // ── applied_batch_result_reply — wire shape (status only, no applicationId/title/appliedAt) ──

    #[test]
    fn reply_carries_url_found_status_only() {
        let entries = vec![
            BatchEntry {
                url: "https://a.example/1".to_string(),
                found: true,
                status: Some("applied".to_string()),
            },
            BatchEntry {
                url: "https://b.example/2".to_string(),
                found: false,
                status: None,
            },
        ];
        let reply = applied_batch_result_reply("req-1", &entries);
        let v: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(v["type"], msg::APPLIED_BATCH_RESULT);
        assert_eq!(v["reqId"], "req-1");
        assert_eq!(v["payload"]["ok"], true);
        assert_eq!(v["payload"]["results"][0]["url"], "https://a.example/1");
        assert_eq!(v["payload"]["results"][0]["found"], true);
        assert_eq!(v["payload"]["results"][0]["status"], "applied");
        assert_eq!(v["payload"]["results"][1]["found"], false);
        assert!(
            v["payload"]["results"][1].get("status").is_none(),
            "status must be absent (not null) when not found"
        );
        assert!(
            v["payload"]["results"][0].get("applicationId").is_none()
                && v["payload"]["results"][0].get("title").is_none()
                && v["payload"]["results"][0].get("appliedAt").is_none(),
            "a batch entry must carry ONLY url/found/status"
        );
    }

    // ── handle_applied_check_batch — cap refusal + malformed payload, end to end ──

    #[test]
    fn handle_refuses_over_cap_without_touching_the_store() {
        let (_dir, store) = open_store();
        let _ = store; // store present but must never be reached for an over-cap request
        let urls: Vec<String> = (0..(MAX_BATCH_URLS + 1))
            .map(|i| format!("https://example.com/{i}"))
            .collect();
        let payload = json!({ "urls": urls });
        // parse_urls alone proves the refusal path — handle_applied_check_batch needs a real
        // AppHandle, so this exercises the exact boundary it calls before ever reaching one.
        assert_eq!(parse_urls(&payload).unwrap_err(), ERR_TOO_MANY_URLS);
    }

    #[test]
    fn handle_rejects_malformed_payload_shape() {
        assert_eq!(
            parse_urls(&json!({ "urls": null })).unwrap_err(),
            ERR_INVALID_REQUEST
        );
    }

    // ── equivalence with applied.check for a single url ──────────────────────

    #[test]
    fn single_url_batch_matches_applied_check_exactly() {
        let (_dir, store) = open_store();
        let url = "https://jobs.example.com/posting/parity-1";
        store
            .upsert_for_origin(
                url,
                "linkedin",
                &app_meta("Acme", "Engineer"),
                ApplicationOrigin::Saved,
                Some(true),
            )
            .unwrap();

        let single =
            super::super::applied_check::resolve_applied_check(&store, &json!({ "url": url }))
                .unwrap();
        let batch = resolve_applied_check_batch(&store, &[url.to_string()]);

        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].found, single.found);
        assert_eq!(batch[0].status, single.status);
        assert_eq!(batch[0].url, url);
    }

    // ── AppliedCheckBatchThrottle ─────────────────────────────────────────────

    #[test]
    fn throttle_allows_a_burst_then_refuses() {
        let mut t = AppliedCheckBatchThrottle::new();
        let now = std::time::Instant::now();
        for i in 0..(APPLIED_CHECK_BATCH_BURST as u32) {
            assert!(t.try_acquire_at(now), "request {i} within the burst");
        }
        assert!(!t.try_acquire_at(now), "burst exhausted");
    }

    #[test]
    fn throttle_refills_one_token_per_interval() {
        let mut t = AppliedCheckBatchThrottle::new();
        let t0 = std::time::Instant::now();
        for _ in 0..(APPLIED_CHECK_BATCH_BURST as u32) {
            assert!(t.try_acquire_at(t0));
        }
        assert!(!t.try_acquire_at(t0));
        let t1 = t0 + std::time::Duration::from_secs_f64(APPLIED_CHECK_BATCH_REFILL_SECS);
        assert!(
            t.try_acquire_at(t1),
            "one interval later, one token refilled"
        );
        assert!(!t.try_acquire_at(t1), "only ONE token refilled");
    }

    #[test]
    fn throttled_reply_carries_rate_limited_sentinel_and_retry_after() {
        let reply = throttled_reply("req-5", 1234);
        let v: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(v["type"], msg::APPLIED_BATCH_RESULT);
        assert_eq!(v["payload"]["ok"], false);
        assert_eq!(
            v["payload"]["error"],
            super::super::agent_call::ERR_RATE_LIMITED
        );
        assert_eq!(v["payload"]["retryAfterMs"], 1234);
    }
}
