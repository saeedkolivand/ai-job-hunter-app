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
        on_note: None,
    }
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_workable_scraper_id() {
    assert_eq!(WorkableScraper.id(), "workable");
}

#[test]
fn test_workable_scraper_display_name() {
    assert_eq!(WorkableScraper.display_name(), "Workable");
}

#[test]
fn test_workable_scraper_mode() {
    assert_eq!(WorkableScraper.mode(), ScraperMode::Http);
}

#[test]
fn test_workable_requires_company() {
    assert!(
        WorkableScraper.requires_company(),
        "Workable is a company-scoped board and must return true for requires_company()"
    );
}

// ---------------------------------------------------------------------------
// Slug guard — is_valid_workable_slug (path-segment SSRF/traversal guard)
// ---------------------------------------------------------------------------

#[test]
fn slug_validation_accepts_valid_slugs() {
    assert!(is_valid_workable_slug("careers-at-sleek"));
    assert!(is_valid_workable_slug("acme123"));
    assert!(is_valid_workable_slug(&"a".repeat(63)));
}

#[test]
fn slug_validation_rejects_invalid_slugs() {
    assert!(
        !is_valid_workable_slug("acme/../secret"),
        "path traversal must be rejected"
    );
    assert!(
        !is_valid_workable_slug("acme?x=1"),
        "query injection must be rejected"
    );
    assert!(!is_valid_workable_slug("acme.corp"), "dot must be rejected");
    assert!(!is_valid_workable_slug("-acme"), "leading hyphen rejected");
    assert!(!is_valid_workable_slug("acme-"), "trailing hyphen rejected");
    assert!(!is_valid_workable_slug(""), "empty slug rejected");
    assert!(
        !is_valid_workable_slug(&"a".repeat(64)),
        "exceeds 63-char limit"
    );
}

/// Every curated `ats_seed` slug for this board must pass the production
/// path-segment guard — regression guard against a seed entry silently
/// drifting out of validator-compatible shape.
#[test]
fn ats_seed_workable_slugs_pass_the_guard() {
    let entries: Vec<_> = crate::scraping::boards::ats_seed::by_ats("workable").collect();
    assert!(!entries.is_empty(), "workable must have seed entries");
    for e in entries {
        assert!(
            is_valid_workable_slug(e.slug),
            "seed slug '{}' ({}) must pass is_valid_workable_slug",
            e.slug,
            e.company
        );
    }
}

// ---------------------------------------------------------------------------
// normalize_workable_companies — lowercase-before-dedup ordering
// ---------------------------------------------------------------------------

/// Case-only variants must collapse to ONE entry — dedup has to run on the
/// same casing the slug is lowercased to for the outbound request, not on
/// the raw input casing (which would let "Acme" and "acme" both survive and
/// fire two identical fetches for the same tenant).
#[test]
fn normalize_workable_companies_collapses_case_variants() {
    let input = vec!["Acme".to_string(), "acme".to_string(), "ACME".to_string()];
    let result = normalize_workable_companies(&input);
    assert_eq!(
        result,
        vec!["acme"],
        "case-only variants of the same slug must dedupe to one lowercase entry"
    );
}

/// Distinct slugs (differing by more than casing) are both kept, still
/// lowercased and in first-seen order.
#[test]
fn normalize_workable_companies_keeps_distinct_slugs_lowercased() {
    let input = vec!["Acme".to_string(), "Beta".to_string(), "acme".to_string()];
    let result = normalize_workable_companies(&input);
    assert_eq!(result, vec!["acme", "beta"]);
}

// ---------------------------------------------------------------------------
// URL guard — is_valid_workable_job_url (host-lock)
// ---------------------------------------------------------------------------

#[test]
fn url_guard_accepts_apply_workable_host_only() {
    assert!(is_valid_workable_job_url(
        "https://apply.workable.com/careers-at-sleek/j/ABCDEF/"
    ));
    assert!(
        !is_valid_workable_job_url("http://apply.workable.com/j/ABCDEF/"),
        "non-https must be rejected"
    );
    assert!(
        !is_valid_workable_job_url("https://evil.example/j/ABCDEF/"),
        "off-host url must be rejected"
    );
    assert!(
        !is_valid_workable_job_url("https://evil-apply.workable.com/j/ABCDEF/"),
        "lookalike host must be rejected (exact host match only)"
    );
    assert!(
        !is_valid_workable_job_url("not-a-url"),
        "unparseable url rejected"
    );
}

/// Embedded userinfo must be rejected even on the correct host — `host_str()`
/// ignores userinfo, so `is_valid_workable_job_url` needs its own explicit
/// username/password check (CodeRabbit finding on PR #535).
#[test]
fn url_guard_rejects_embedded_userinfo() {
    assert!(
        !is_valid_workable_job_url("https://spoof@apply.workable.com/j/ABC"),
        "userinfo on the correct host must still be rejected"
    );
    assert!(
        !is_valid_workable_job_url("https://spoof:pw@apply.workable.com/j/ABC"),
        "userinfo with a password must still be rejected"
    );
}

// ---------------------------------------------------------------------------
// parse_workable_date — RFC3339 and bare-date formats
// ---------------------------------------------------------------------------

#[test]
fn parse_workable_date_accepts_rfc3339_and_bare_date() {
    let rfc3339 = parse_workable_date("2024-03-15T00:00:00Z");
    assert!(rfc3339.is_some());
    let bare = parse_workable_date("2024-03-15");
    assert!(bare.is_some());
    assert_eq!(rfc3339, bare, "midnight RFC3339 and bare date must match");
    assert!(parse_workable_date("not-a-date").is_none());
}

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
