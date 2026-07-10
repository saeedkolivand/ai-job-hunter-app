use super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_input(companies: Vec<String>) -> BoardSearchInput {
    BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies,
    }
}

fn make_ctx() -> ScrapeContext {
    ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
    }
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_rippling_scraper_id() {
    let scraper = RipplingScraper;
    assert_eq!(scraper.id(), "rippling");
}

#[test]
fn test_rippling_scraper_display_name() {
    let scraper = RipplingScraper;
    assert_eq!(scraper.display_name(), "Rippling");
}

#[test]
fn test_rippling_scraper_mode() {
    let scraper = RipplingScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_rippling_requires_company() {
    assert!(
        RipplingScraper.requires_company(),
        "Rippling is an ATS board and must return true for requires_company()"
    );
}

// ---------------------------------------------------------------------------
// normalize_companies — unit tests (network-free)
// ---------------------------------------------------------------------------

#[test]
fn normalize_drops_blank_entries() {
    let input = vec![
        "acme".to_string(),
        "".to_string(),
        "   ".to_string(),
        "\t".to_string(),
        "beta".to_string(),
    ];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["acme", "beta"]);
}

#[test]
fn normalize_trims_whitespace() {
    let input = vec!["  acme  ".to_string(), "\tbeta\n".to_string()];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["acme", "beta"]);
}

#[test]
fn normalize_dedupes_first_seen_order() {
    let input = vec![
        "alpha".to_string(),
        "beta".to_string(),
        "alpha".to_string(), // duplicate — must be dropped
        "gamma".to_string(),
        "beta".to_string(), // duplicate — must be dropped
    ];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn normalize_dedupes_after_trim() {
    let input = vec!["  alpha  ".to_string(), "alpha".to_string()];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["alpha"]);
}

#[test]
fn normalize_caps_at_max() {
    let input: Vec<String> = (0..60).map(|i| format!("company-{i}")).collect();
    let result = normalize_companies(&input, 50);
    assert_eq!(result.len(), 50);
    assert_eq!(result[0], "company-0");
    assert_eq!(result[49], "company-49");
}

#[test]
fn normalize_cap_exact_boundary() {
    let input: Vec<String> = (0..50).map(|i| format!("co-{i}")).collect();
    let result = normalize_companies(&input, 50);
    assert_eq!(result.len(), 50);
}

#[test]
fn normalize_empty_input_returns_empty() {
    let result = normalize_companies(&[], 50);
    assert!(result.is_empty());
}

#[test]
fn normalize_all_blanks_returns_empty() {
    let input = vec!["".to_string(), "   ".to_string(), "\n".to_string()];
    let result = normalize_companies(&input, 50);
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// Slug guard — is_valid_rippling_slug (path segment, not a DNS label: mixed
// case is allowed, but path-traversal/query-injection characters are not)
// ---------------------------------------------------------------------------

#[test]
fn slug_validation_accepts_mixed_case_and_valid_slugs() {
    assert!(is_valid_rippling_slug("acme"));
    assert!(
        is_valid_rippling_slug("Acme-Corp"),
        "mixed case must be accepted — this is a URL path segment, not a DNS label"
    );
    assert!(is_valid_rippling_slug("ACME123"));
    assert!(
        is_valid_rippling_slug(&"a".repeat(63)),
        "exactly 63 chars must be accepted"
    );
}

#[test]
fn slug_validation_rejects_path_traversal_and_invalid_slugs() {
    assert!(!is_valid_rippling_slug("acme.corp"), "dot must be rejected");
    assert!(
        !is_valid_rippling_slug("acme/corp"),
        "slash must be rejected"
    );
    assert!(
        !is_valid_rippling_slug("../etc/passwd"),
        "path traversal must be rejected"
    );
    assert!(!is_valid_rippling_slug("acme@corp"), "@ must be rejected");
    assert!(
        !is_valid_rippling_slug("acme_corp"),
        "underscore must be rejected"
    );
    assert!(
        !is_valid_rippling_slug("-acme"),
        "leading hyphen must be rejected"
    );
    assert!(
        !is_valid_rippling_slug("acme-"),
        "trailing hyphen must be rejected"
    );
    assert!(!is_valid_rippling_slug(""), "empty slug must be rejected");
    assert!(
        !is_valid_rippling_slug(&"a".repeat(64)),
        "exceeds 63-char limit"
    );
}

// ---------------------------------------------------------------------------
// Job URL guard — is_valid_rippling_job_url (host allowlist)
// ---------------------------------------------------------------------------

#[test]
fn job_url_guard_accepts_ats_host_rejects_others() {
    assert!(is_valid_rippling_job_url(
        "https://ats.rippling.com/acme/jobs/abc"
    ));
    assert!(
        !is_valid_rippling_job_url("http://ats.rippling.com/acme/jobs/abc"),
        "non-https must be rejected"
    );
    assert!(
        !is_valid_rippling_job_url("https://evil.example/acme/jobs/abc"),
        "wrong host must be rejected"
    );
    assert!(!is_valid_rippling_job_url("not-a-url"));
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = RipplingScraper;
    let result = scraper.search(make_input(Vec::new()), make_ctx()).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

#[tokio::test]
async fn invalid_slug_skipped_without_network() {
    let scraper = RipplingScraper;
    let result = scraper
        .search(make_input(vec!["dotted.host".to_string()]), make_ctx())
        .await;
    assert!(
        result.is_ok(),
        "search must return Ok even for invalid slug"
    );
    assert!(
        result.unwrap().is_empty(),
        "invalid slug must produce empty result (skipped, no network)"
    );
}

/// A pre-cancelled signal must make the loop break immediately without
/// recording `first_fetch_error`.
#[tokio::test]
async fn cancelled_before_fetch_returns_ok_not_err() {
    let scraper = RipplingScraper;
    let ctx = make_ctx();
    ctx.signal.cancel();
    let result = scraper
        .search(make_input(vec!["acme".to_string()]), ctx)
        .await;
    assert!(
        result.is_ok(),
        "cancelled run must return Ok, not Err — cancellation must not be recorded as first_fetch_error"
    );
}

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
