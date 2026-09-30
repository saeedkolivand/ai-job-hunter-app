use super::super::super::test_support::*;
use super::super::*;
use super::fixture_mapping::FIXTURE_JSON;

// ── per-row deserialization resilience ───────────────────────────────────────

#[test]
fn test_rows_to_jobs_skips_malformed_row_keeps_good_ones() {
    // One row has `id` as a STRING (schema drift on a single row); the other
    // two are well-formed. Without per-row resilience, `Vec<Job>`'s atomic
    // deserialize would zero the whole batch on that one bad row.
    let values: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"id": 1, "url": "https://jobicy.com/jobs/1-x", "jobTitle": "Good Job A"},
            {"id": "not-a-number", "url": "https://jobicy.com/jobs/2-x", "jobTitle": "Bad Row"},
            {"id": 3, "url": "https://jobicy.com/jobs/3-x", "jobTitle": "Good Job B"}
        ]"#,
    )
    .unwrap();

    let jobs = rows_to_jobs(values);
    assert_eq!(
        jobs.len(),
        2,
        "the malformed row must be skipped; both good rows must survive"
    );
    assert_eq!(jobs[0].job_title.as_deref(), Some("Good Job A"));
    assert_eq!(jobs[1].job_title.as_deref(), Some("Good Job B"));
}

#[test]
fn test_rows_to_jobs_empty_input_returns_empty() {
    assert!(rows_to_jobs(Vec::new()).is_empty());
}

// ── keyless/empty response ───────────────────────────────────────────────────

#[test]
fn test_empty_jobs_array_parses_to_empty_vec() {
    let resp: Resp = serde_json::from_str(r#"{"jobs": []}"#).unwrap();
    assert!(resp.jobs.is_empty());
}

#[test]
fn test_missing_jobs_field_defaults_to_empty_vec() {
    // Defensive: an unexpected/absent `jobs` key must not fail deserialization
    // (`#[serde(default)]`) — a keyless-empty response is `Ok(vec![])`, never a
    // parse error or a fabricated result.
    let resp: Resp = serde_json::from_str(r#"{"jobCount": 0}"#).unwrap();
    assert!(resp.jobs.is_empty());
}

// ── parse_response (hermetic — the 404-vs-other-status contract) ────────────
//
// `search()` delegates its "is this a real failure or a 404-but-valid-empty-
// result" decision to `parse_response`, a pure function with no network call,
// so this load-bearing contract no longer relies solely on the `#[ignore]`d
// live-network tests below.

#[test]
fn test_parse_response_500_is_err() {
    let result = parse_response(500, "Internal Server Error", "jobicy", 0);
    assert!(result.is_err(), "a real 5xx outage must never be Ok");
}

#[test]
fn test_parse_response_403_is_err() {
    let result = parse_response(403, "Forbidden", "jobicy", 0);
    assert!(result.is_err(), "a 403 must never be misread as empty");
}

#[test]
fn test_parse_response_404_with_empty_jobs_json_is_ok_empty() {
    let body = r#"{"jobs":[],"success":false,"message":"Nothing found..."}"#;
    let result = parse_response(404, body, "jobicy", 0);
    assert!(
        result.is_ok(),
        "a 404 with a valid empty-jobs JSON body is a genuine zero-match search"
    );
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_parse_response_200_with_jobs_is_ok_with_postings() {
    let result = parse_response(200, FIXTURE_JSON, "jobicy", 0);
    assert!(result.is_ok(), "a 200 with a well-formed body must parse");
    // job3 has no id and is dropped; job1+job2 survive (see FIXTURE_JSON doc).
    assert_eq!(result.unwrap().len(), 2);
}

#[test]
fn test_parse_response_404_with_html_body_is_err() {
    // A real routing 404 (e.g. a CDN/edge error page) returns HTML, not the
    // `{"jobs":[...]}` shape — JSON parsing must fail and propagate as `Err`,
    // never a silently-empty `Ok`.
    let body = "<html><body>404 Not Found</body></html>";
    let result = parse_response(404, body, "jobicy", 0);
    assert!(
        result.is_err(),
        "a 404 with an HTML (non-JSON) body is a real outage, not an empty result"
    );
}

// ── live network (ignored in CI) ─────────────────────────────────────────────

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = JobicyScraper;
    let input = BoardSearchInput {
        query: "engineer".to_string(),
        ..default_search_input()
    };
    let ctx = default_ctx();
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        scraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    assert!(
        first.description.is_some(),
        "jobicy always returns a full description — feed changed?"
    );
    println!("jobicy: {} results", postings.len());
    println!("first: {:?}", first.title);
}

#[tokio::test]
#[ignore = "live network"]
async fn live_garbage_tag_returns_ok_empty_not_error() {
    // Regression guard for the live-verified quirk `search()` depends on:
    // Jobicy returns HTTP 404 with a valid JSON body for a genuine zero-match
    // `tag` search. That must resolve `Ok(vec![])`, not an `Err` — otherwise a
    // healthy "no results for this keyword" search would misreport as a
    // failed board in `BoardScrapeSummary.error`.
    let scraper = JobicyScraper;
    let input = BoardSearchInput {
        query: "zzz-guaranteed-no-match-xyz123".to_string(),
        amount: 5,
        ..default_search_input()
    };
    let ctx = default_ctx();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        scraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(
        result.is_ok(),
        "a zero-match tag search must be Ok(empty), not Err: {:?}",
        result.err()
    );
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn search_respects_pre_cancelled_token() {
    // Not `#[ignore = "live network"]`: `fetch_text` checks `signal.is_cancelled()`
    // BEFORE issuing any request (scraping/http/mod.rs), so a pre-cancelled token
    // makes zero network calls — this hermetically exercises the cancellation
    // contract. `AppError::Cancelled` must propagate as an `Err`, not a
    // fabricated empty success.
    let scraper = JobicyScraper;
    let signal = tokio_util::sync::CancellationToken::new();
    signal.cancel();
    let input = default_search_input();
    let ctx = ScrapeContext {
        signal,
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        scraper.search(input, ctx),
    )
    .await
    .expect("cancelled search must not hang");
    assert!(
        result.is_err(),
        "a pre-cancelled token must surface as an error, not a silent empty Ok"
    );
}
