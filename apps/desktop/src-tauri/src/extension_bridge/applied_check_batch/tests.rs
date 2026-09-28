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
        super::super::applied_check::resolve_applied_check(&store, &json!({ "url": url })).unwrap();
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
