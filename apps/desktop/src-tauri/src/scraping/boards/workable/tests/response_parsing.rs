use super::super::*;
use super::metadata_and_guards::{make_ctx, make_input};

// ---------------------------------------------------------------------------
// rows_to_jobs — per-row resilience
// ---------------------------------------------------------------------------

#[test]
fn rows_to_jobs_skips_malformed_rows_keeps_valid_ones() {
    let values: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"title": "Valid", "shortcode": "abc123", "url": "https://apply.workable.com/j/abc123/", "telecommuting": false},
            {"title": "Bad telecommuting type", "shortcode": "def456", "url": "https://apply.workable.com/j/def456/", "telecommuting": "yes"},
            {"title": "Also Valid", "shortcode": "ghi789", "url": "https://apply.workable.com/j/ghi789/", "telecommuting": true}
        ]"#,
    )
    .unwrap();
    let jobs = rows_to_jobs(values);
    assert_eq!(
        jobs.len(),
        2,
        "the row with a malformed 'telecommuting' type must be dropped, valid rows kept"
    );
}

#[test]
fn rows_to_jobs_empty_input_returns_empty() {
    assert!(rows_to_jobs(vec![]).is_empty());
}

/// trust-H item 2: when EVERY job row in a non-empty batch fails to deserialize,
/// `rows_to_jobs` returns an empty `Vec` — the signal `search()` uses
/// (`raw_row_count > 0 && rows_to_jobs(..).is_empty()`) to treat the company as
/// a FETCH FAILURE instead of a silent success-with-zero-jobs. Mirrors Breezy's
/// round-2 fix, now applied to Workable too.
#[test]
fn rows_to_jobs_all_rows_undeserializable_returns_empty() {
    // Every row is unparseable as `WkJob`: a bad `telecommuting` type, or not
    // even an object (all of `WkJob`'s fields are optional, so an EMPTY object
    // would parse — these must genuinely fail instead).
    let values: Vec<serde_json::Value> = serde_json::from_str(
        r#"[{"telecommuting": "yes"}, "not-an-object", 42, {"title": "X", "telecommuting": 3}]"#,
    )
    .unwrap();
    let jobs = rows_to_jobs(values);
    assert!(
        jobs.is_empty(),
        "every row failing to deserialize must yield an empty Vec (the all-drift signal)"
    );
}

// ---------------------------------------------------------------------------
// parse_workable_response — fixture-based parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_workable_response_happy_path() {
    let json = r#"[
        {
            "title": "Backend Engineer",
            "shortcode": "ABC123",
            "url": "https://apply.workable.com/careers-at-sleek/j/ABC123/",
            "published_on": "2024-03-15",
            "city": "Singapore",
            "state": null,
            "country": "Singapore",
            "telecommuting": false,
            "description": "<p>Build things</p>"
        }
    ]"#;
    let jobs: Vec<WkJob> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_workable_response(jobs, "Sleek", "careers-at-sleek", 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    let p = &postings[0];
    assert_eq!(p.title, "Backend Engineer");
    assert_eq!(
        p.url,
        "https://apply.workable.com/careers-at-sleek/j/ABC123/"
    );
    assert_eq!(p.company, "Sleek");
    assert_eq!(p.location, Some("Singapore, Singapore".to_string()));
    assert_eq!(p.id, "workable:careers-at-sleek:ABC123");
    assert_eq!(p.external_id, Some("ABC123".to_string()));
    assert_eq!(p.source, "workable");
    assert_eq!(p.description.as_deref(), Some("Build things"));
    assert_eq!(p.captured_at, 1_700_000_000_000);
    assert!(p.posted_at.is_some());
}

#[test]
fn parse_workable_response_telecommuting_appends_remote() {
    let json = r#"[
        {"title": "Support Engineer", "shortcode": "REM1", "url": "https://apply.workable.com/j/REM1/", "city": "Berlin", "state": null, "country": "Germany", "telecommuting": true}
    ]"#;
    let jobs: Vec<WkJob> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_workable_response(jobs, "Acme", "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(
        postings[0].location,
        Some("Berlin, Germany, Remote".to_string())
    );
    assert_eq!(
        postings[0].extra.get("workType").and_then(|v| v.as_str()),
        Some("remote")
    );
}

#[test]
fn parse_workable_response_telecommuting_with_no_other_location_yields_bare_remote() {
    let json = r#"[
        {"title": "Fully Remote Role", "shortcode": "REM2", "url": "https://apply.workable.com/j/REM2/", "city": null, "state": null, "country": null, "telecommuting": true}
    ]"#;
    let jobs: Vec<WkJob> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_workable_response(jobs, "Acme", "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].location, Some("Remote".to_string()));
}

/// `telecommuting: false` is a binary "not flagged remote", not a positive
/// on-site declaration — must write nothing to `extra.workType`.
#[test]
fn parse_workable_response_telecommuting_false_writes_nothing() {
    let json = r#"[
        {"title": "Office Role", "shortcode": "OFF1", "url": "https://apply.workable.com/j/OFF1/", "city": "Berlin", "state": null, "country": "Germany", "telecommuting": false}
    ]"#;
    let jobs: Vec<WkJob> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_workable_response(jobs, "Acme", "acme", 0);
    assert_eq!(postings.len(), 1);
    assert!(
        !postings[0].extra.contains_key("workType"),
        "telecommuting:false must not write a guessed on-site value"
    );
}

#[test]
fn parse_workable_response_empty_jobs_returns_empty_vec() {
    assert!(parse_workable_response(vec![], "Acme", "acme", 0).is_empty());
}

/// Missing title, missing shortcode, and an off-host url each drop the row;
/// a valid row in the same payload must still come through.
#[test]
fn parse_workable_response_drops_malformed_rows() {
    let json = r#"[
        {"title": "Valid One", "shortcode": "OK1", "url": "https://apply.workable.com/j/OK1/", "city": null, "state": null, "country": null, "telecommuting": false},
        {"title": null, "shortcode": "OK2", "url": "https://apply.workable.com/j/OK2/", "city": null, "state": null, "country": null, "telecommuting": false},
        {"title": "Missing Shortcode", "shortcode": null, "url": "https://apply.workable.com/j/OK3/", "city": null, "state": null, "country": null, "telecommuting": false},
        {"title": "Off Host", "shortcode": "OK4", "url": "https://evil.example/j/OK4/", "city": null, "state": null, "country": null, "telecommuting": false},
        {"title": "Valid Two", "shortcode": "OK5", "url": "https://apply.workable.com/j/OK5/", "city": null, "state": null, "country": null, "telecommuting": false}
    ]"#;
    let jobs: Vec<WkJob> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_workable_response(jobs, "Acme", "acme", 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
}

#[test]
fn parse_workable_response_dedupes_by_url() {
    let json = r#"[
        {"title": "First", "shortcode": "DUP", "url": "https://apply.workable.com/j/DUP/", "city": null, "state": null, "country": null, "telecommuting": false},
        {"title": "Duplicate", "shortcode": "DUP", "url": "https://apply.workable.com/j/DUP/", "city": null, "state": null, "country": null, "telecommuting": false},
        {"title": "Distinct", "shortcode": "OTHER", "url": "https://apply.workable.com/j/OTHER/", "city": null, "state": null, "country": null, "telecommuting": false}
    ]"#;
    let jobs: Vec<WkJob> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_workable_response(jobs, "Acme", "acme", 0);
    assert_eq!(postings.len(), 2, "duplicate url must be deduped");
    assert_eq!(postings[0].title, "First", "first-seen row wins the dedupe");
}

/// A row with an absent `url` but a present `shortcode` must not be dropped —
/// it falls back to Workable's own canonical apply-page URL pattern
/// (`https://apply.workable.com/j/{shortcode}`), which still passes the
/// host-lock validation.
#[test]
fn parse_workable_response_missing_url_falls_back_to_apply_j_shortcode() {
    let json = r#"[
        {"title": "No URL Row", "shortcode": "NOURL1", "city": null, "state": null, "country": null, "telecommuting": false}
    ]"#;
    let jobs: Vec<WkJob> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_workable_response(jobs, "Acme", "acme", 0);
    assert_eq!(
        postings.len(),
        1,
        "url-absent row with a shortcode must be kept via the fallback URL"
    );
    assert_eq!(
        postings[0].url, "https://apply.workable.com/j/NOURL1",
        "must build the canonical apply-page URL from the shortcode"
    );
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let result = WorkableScraper
        .search(make_input(Vec::new()), make_ctx())
        .await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(result.unwrap().is_empty());
}

/// An all-invalid-slug run rejects every slug pre-fetch (no network — the path-
/// traversal guard) and now surfaces a distinct board error instead of a silent
/// zero (claude review #597).
#[tokio::test]
async fn all_invalid_slugs_error_without_network() {
    let result = WorkableScraper
        .search(make_input(vec!["../escape".to_string()]), make_ctx())
        .await;
    let err = result.expect_err("an all-invalid-slug run must be a board error, not a silent zero");
    assert!(
        err.to_string().contains("slug(s) invalid"),
        "error must name the invalid-slug reason, got: {err}"
    );
}

#[tokio::test]
async fn cancelled_before_fetch_returns_ok_not_err() {
    let ctx = make_ctx();
    ctx.signal.cancel();
    let result = WorkableScraper
        .search(make_input(vec!["careers-at-sleek".to_string()]), ctx)
        .await;
    assert!(
        result.is_ok(),
        "cancelled run must return Ok, not Err — cancellation must not be recorded as first_fetch_error"
    );
    assert!(
        result.unwrap().is_empty(),
        "cancelled run must return an empty Vec — no postings from a run that never fetched"
    );
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let input = make_input(vec!["careers-at-sleek".to_string()]);
    let ctx = make_ctx();
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        WorkableScraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    println!("workable: {} results", postings.len());
}
