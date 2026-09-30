use super::super::*;
use super::metadata_and_search::{make_ctx, make_input};

// ---------------------------------------------------------------------------
// rows_to_jobs — per-row resilience (regression guard for silent-batch-loss)
// ---------------------------------------------------------------------------

/// Core regression guard: one malformed row (missing required `uuid`) must
/// not fail the whole batch — the other, well-formed rows must still come
/// through. Before this fix, `fetch_json::<Vec<RpJob>>` deserialized the
/// array atomically, so a single bad row silently yielded zero jobs.
#[test]
fn rows_to_jobs_skips_row_missing_uuid_keeps_good_rows() {
    let values: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"uuid": "good-1", "name": "Engineer One", "url": "https://ats.rippling.com/acme/jobs/good-1", "workLocation": null},
            {"name": "No UUID Row", "url": "https://ats.rippling.com/acme/jobs/x", "workLocation": null},
            {"uuid": "good-2", "name": "Engineer Two", "url": "https://ats.rippling.com/acme/jobs/good-2", "workLocation": null}
        ]"#,
    )
    .unwrap();

    let jobs = rows_to_jobs(values);
    let uuids: Vec<&str> = jobs.iter().map(|j| j.uuid.as_str()).collect();
    assert_eq!(
        uuids,
        vec!["good-1", "good-2"],
        "the missing-uuid row must be dropped, both good rows kept: {uuids:?}"
    );
}

/// A non-object `workLocation` (e.g. a bare number, which the untagged enum
/// can't match as either `Object` or `Text`) fails per-row deserialize and
/// must be skipped without taking down sibling rows.
#[test]
fn rows_to_jobs_skips_row_with_non_object_work_location() {
    let values: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"uuid": "good-1", "name": "Engineer One", "url": "https://ats.rippling.com/acme/jobs/good-1", "workLocation": null},
            {"uuid": "bad-location", "name": "Bad Location", "url": "https://ats.rippling.com/acme/jobs/bad-location", "workLocation": 42},
            {"uuid": "good-2", "name": "Engineer Two", "url": "https://ats.rippling.com/acme/jobs/good-2", "workLocation": null}
        ]"#,
    )
    .unwrap();

    let jobs = rows_to_jobs(values);
    let uuids: Vec<&str> = jobs.iter().map(|j| j.uuid.as_str()).collect();
    assert_eq!(
        uuids,
        vec!["good-1", "good-2"],
        "the non-object workLocation row must be dropped, both good rows kept: {uuids:?}"
    );
}

#[test]
fn rows_to_jobs_empty_array_returns_empty_vec() {
    let values: Vec<serde_json::Value> = serde_json::from_str("[]").unwrap();
    assert!(rows_to_jobs(values).is_empty());
}

/// trust-H item 2: when EVERY row in a non-empty batch fails to deserialize,
/// `rows_to_jobs` returns an empty `Vec` — the exact signal `search()` uses
/// (`raw_row_count > 0 && rows_to_jobs(..).is_empty()`) to treat the company as
/// a FETCH FAILURE (recorded into `first_fetch_error`) instead of a silent
/// success-with-zero-jobs. Mirrors Breezy's round-2 fix, now applied to
/// Rippling too.
#[test]
fn rows_to_jobs_all_rows_undeserializable_returns_empty() {
    // Every row is unparseable as `RpJob`: missing the required `uuid`, or not
    // even an object.
    let values: Vec<serde_json::Value> =
        serde_json::from_str(r#"[{"name": "No UUID"}, "not-an-object", 42, null]"#).unwrap();
    let jobs = rows_to_jobs(values);
    assert!(
        jobs.is_empty(),
        "every row failing to deserialize must yield an empty Vec (the all-drift signal)"
    );
}

// ---------------------------------------------------------------------------
// parse_rippling_response — fixture-based parsing
// ---------------------------------------------------------------------------

/// Data-shape unknown #1a: `workLocation` as an object `{ "label": "..." }`.
#[test]
fn parse_rippling_response_work_location_object_form() {
    let json = r#"[
        {
            "uuid": "job-abc-123",
            "name": "Backend Engineer",
            "url": "https://ats.rippling.com/acme/jobs/job-abc-123",
            "workLocation": { "label": "Remote (US)" }
        }
    ]"#;
    let jobs: Vec<RpJob> = serde_json::from_str(json).expect("fixture must parse");
    let postings = parse_rippling_response(jobs, "acme", 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    let p = &postings[0];
    assert_eq!(p.title, "Backend Engineer");
    assert_eq!(p.url, "https://ats.rippling.com/acme/jobs/job-abc-123");
    assert_eq!(p.company, "acme");
    assert_eq!(p.location, Some("Remote (US)".to_string()));
    assert_eq!(p.id, "rippling:job-abc-123");
    assert_eq!(p.external_id, Some("job-abc-123".to_string()));
    assert_eq!(p.source, "rippling");
    assert_eq!(p.captured_at, 1_700_000_000_000);
}

/// Data-shape unknown #1b: `workLocation` as a bare string.
#[test]
fn parse_rippling_response_work_location_string_form() {
    let json = r#"[
        {
            "uuid": "job-xyz",
            "name": "Frontend Engineer",
            "url": "https://ats.rippling.com/acme/jobs/job-xyz",
            "workLocation": "Remote"
        }
    ]"#;
    let jobs: Vec<RpJob> = serde_json::from_str(json).expect("fixture must parse");
    let postings = parse_rippling_response(jobs, "acme", 0);

    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].location, Some("Remote".to_string()));
}

#[test]
fn parse_rippling_response_empty_array_returns_empty_vec() {
    let jobs: Vec<RpJob> = serde_json::from_str("[]").unwrap();
    assert!(
        parse_rippling_response(jobs, "acme", 0).is_empty(),
        "empty array must parse to an empty Vec, not an error"
    );
}

/// Missing/empty name and missing/malformed (wrong-host) url each drop the
/// row; valid rows in the same payload must still come through. `uuid` is a
/// required (non-Option) field so every row must carry one to parse at all.
#[test]
fn parse_rippling_response_drops_malformed_rows() {
    let json = r#"[
        {"uuid": "valid-1", "name": "Valid One", "url": "https://ats.rippling.com/acme/jobs/valid-1", "workLocation": null},
        {"uuid": "missing-name", "name": null, "url": "https://ats.rippling.com/acme/jobs/missing-name", "workLocation": null},
        {"uuid": "empty-name", "name": "", "url": "https://ats.rippling.com/acme/jobs/empty-name", "workLocation": null},
        {"uuid": "missing-url", "name": "Missing URL", "url": null, "workLocation": null},
        {"uuid": "wrong-host", "name": "Wrong Host", "url": "https://evil.example/acme/jobs/wrong-host", "workLocation": null},
        {"uuid": "valid-2", "name": "Valid Two", "url": "https://ats.rippling.com/acme/jobs/valid-2", "workLocation": null}
    ]"#;
    let jobs: Vec<RpJob> = serde_json::from_str(json).unwrap();
    let postings = parse_rippling_response(jobs, "acme", 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = RipplingScraper;
    let input = make_input(vec!["rippling".to_string()]);
    let ctx = make_ctx();
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
    println!("rippling: {} results", postings.len());
    println!("first: {:?}", first.title);
}
