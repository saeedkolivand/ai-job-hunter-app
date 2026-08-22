use super::*;

#[test]
fn test_ashby_scraper_id() {
    let scraper = AshbyScraper;
    assert_eq!(scraper.id(), "ashby");
}

#[test]
fn test_ashby_scraper_display_name() {
    let scraper = AshbyScraper;
    assert_eq!(scraper.display_name(), "Ashby");
}

#[test]
fn test_ashby_scraper_mode() {
    let scraper = AshbyScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_ashby_requires_company() {
    assert!(
        AshbyScraper.requires_company(),
        "Ashby is an ATS board and must return true for requires_company()"
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
    // "  alpha  " and "alpha" are the same after trimming.
    let input = vec!["  alpha  ".to_string(), "alpha".to_string()];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["alpha"]);
}

#[test]
fn normalize_caps_at_max() {
    // 60 distinct slugs, cap = 50 (MAX_COMPANIES for ashby).
    let input: Vec<String> = (0..60).map(|i| format!("company-{i}")).collect();
    let result = normalize_companies(&input, 50);
    assert_eq!(result.len(), 50);
    assert_eq!(result[0], "company-0");
    assert_eq!(result[49], "company-49");
}

#[test]
fn normalize_cap_exact_boundary() {
    // Exactly MAX_COMPANIES entries — none should be dropped.
    let input: Vec<String> = (0..50).map(|i| format!("co-{i}")).collect();
    let result = normalize_companies(&input, 50);
    assert_eq!(result.len(), 50);
}

/// Ashby slug casing is exact and must match the registered board (e.g.
/// `Linear`, `Perplexity`) — `normalize_companies` must never lowercase it.
#[test]
fn ats_seed_ashby_casing_survives_normalize_companies() {
    let slugs: Vec<String> = crate::scraping::boards::ats_seed::by_ats("ashby")
        .map(|e| e.slug.to_string())
        .collect();
    assert!(!slugs.is_empty(), "ashby must have seed entries");
    let normalized = normalize_companies(&slugs, 50);
    for casing_sensitive in ["Linear", "Perplexity"] {
        assert!(
            normalized.iter().any(|s| s == casing_sensitive),
            "casing '{casing_sensitive}' must survive verbatim; got {normalized:?}"
        );
    }
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
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = AshbyScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let result = scraper.search(input, ctx).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

/// A pre-cancelled signal must make the loop break immediately without
/// recording `first_fetch_error`.  If cancellation were treated as a genuine
/// failure the board would return `Err` on any cancelled run that happened to
/// receive a network error — this ensures it always returns `Ok` instead.
#[tokio::test]
async fn cancelled_before_fetch_returns_ok_not_err() {
    let scraper = AshbyScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        // Valid slug that would normally trigger a fetch.
        companies: vec!["acme".to_string()],
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    // Cancel before search runs so the per-company loop breaks on the first
    // iteration without attempting any network I/O.
    ctx.signal.cancel();

    let result = scraper.search(input, ctx).await;
    assert!(
        result.is_ok(),
        "cancelled run must return Ok, not Err — cancellation must not be recorded as first_fetch_error"
    );
}

/// When all company entries are blank or whitespace the list normalises to
/// empty — the scraper falls through the normalisation stage with an empty
/// Vec, producing Ok([]) rather than Err.
///
/// ponytail: the all-fail Err path (successful_fetches==0 + first_fetch_error
/// set) is only reachable when at least one company makes it past normalisation
/// and then the HTTP call fails — that requires a real or mocked HTTP layer
/// which we don't have here. The guarantee is already tested at the
/// normalize_companies unit-test level (all blanks → empty vec → loop doesn't
/// run → no error state can form).
#[tokio::test]
async fn all_blank_companies_returns_ok_empty() {
    let scraper = AshbyScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["".to_string(), "   ".to_string()],
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let result = scraper.search(input, ctx).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

/// The partial-failure note, end to end against a mock Ashby host: company
/// `good` returns a board (200), company `rotted` 404s. The board must KEEP the
/// good company's postings (`Ok`, not a whole-board error) and report exactly
/// `companies-failed:1` through the `on_note` sink — the failure that used to be
/// log-only whenever a sibling company succeeded. Mirrors lever's copy.
#[tokio::test]
async fn partial_company_failure_reports_the_companies_failed_note() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/posting-api/job-board/good"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "apiVersion": "1",
            "jobs": [{
                "id": "abc123",
                "title": "Rust Engineer",
                "locationName": "Berlin",
                "isRemote": true,
                "jobUrl": "https://jobs.ashbyhq.com/good/abc123",
                "descriptionPlain": "Build things in Rust.",
                "publishedAt": "2026-06-01T09:00:00Z",
            }],
        })))
        .mount(&server)
        .await;
    // A rotted/renamed slug — the exact shape that used to vanish silently.
    Mock::given(method("GET"))
        .and(path("/posting-api/job-board/rotted"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let notes = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = notes.clone();
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: Some(std::sync::Arc::new(move |n: String| {
            sink.lock().expect("note sink poisoned").push(n)
        })),
    };
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["good".to_string(), "rotted".to_string()],
    };

    let out = AshbyScraper
        .search_with_base(&server.uri(), input, ctx)
        .await
        .expect("a partial run must stay Ok — one succeeding company is a result");

    assert_eq!(
        out.len(),
        1,
        "the succeeding company's postings must be kept, got {out:?}"
    );
    assert_eq!(
        notes.lock().expect("note sink poisoned").as_slice(),
        ["companies-failed:1".to_string()],
        "the failed company must surface as exactly one companies-failed note"
    );
}

/// The complement: every company succeeds → NO note (a clean run must not grow a
/// chip). Guards against the counter being incremented on a success path.
#[tokio::test]
async fn clean_run_reports_no_note() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "apiVersion": "1",
            "jobs": [],
        })))
        .mount(&server)
        .await;

    let notes = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = notes.clone();
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: Some(std::sync::Arc::new(move |n: String| {
            sink.lock().expect("note sink poisoned").push(n)
        })),
    };
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["good".to_string()],
    };

    AshbyScraper
        .search_with_base(&server.uri(), input, ctx)
        .await
        .expect("an all-success run must be Ok");

    assert!(
        notes.lock().expect("note sink poisoned").is_empty(),
        "a run with zero failures must emit no note"
    );
}

/// `workplaceType` must win over `isRemote` — a Hybrid row with `isRemote:true`
/// (the live-measured shape: 107 of 136 Ramp postings) must map to
/// `extra.workType == "hybrid"`, not fall through to a remote badge, AND
/// `extra.remote` must NOT be `true` for that same row. `extra.remote` feeds
/// `location_filter`'s "a board-flagged-remote posting can never conflict with
/// a place" short-circuit, so writing `true` there for a Hybrid job would make
/// a New-York-Hybrid posting immune to a Berlin location search — the dual-write
/// defect this test pins. Covers all three declared values plus an absent field.
#[tokio::test]
async fn workplace_type_maps_to_extra_work_type() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/posting-api/job-board/acme"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "apiVersion": "1",
            "jobs": [
                {
                    "id": "hybrid-1",
                    "title": "Hybrid Engineer",
                    "locationName": "Berlin",
                    "isRemote": true,
                    "workplaceType": "Hybrid",
                    "jobUrl": "https://jobs.ashbyhq.com/acme/hybrid-1",
                },
                {
                    "id": "remote-1",
                    "title": "Remote Engineer",
                    "locationName": "Anywhere",
                    "isRemote": true,
                    "workplaceType": "Remote",
                    "jobUrl": "https://jobs.ashbyhq.com/acme/remote-1",
                },
                {
                    "id": "onsite-1",
                    "title": "Onsite Engineer",
                    "locationName": "Berlin",
                    "isRemote": false,
                    "workplaceType": "OnSite",
                    "jobUrl": "https://jobs.ashbyhq.com/acme/onsite-1",
                },
                {
                    "id": "absent-1",
                    "title": "Undeclared Engineer",
                    "locationName": "Berlin",
                    "isRemote": false,
                    "jobUrl": "https://jobs.ashbyhq.com/acme/absent-1",
                },
            ],
        })))
        .mount(&server)
        .await;

    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["acme".to_string()],
    };

    let out = AshbyScraper
        .search_with_base(&server.uri(), input, ctx)
        .await
        .expect("mocked run must succeed");

    let work_type = |id: &str| -> Option<String> {
        out.iter()
            .find(|p| p.external_id.as_deref() == Some(id))
            .and_then(|p| p.extra.get("workType"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    let is_remote = |id: &str| -> bool {
        out.iter()
            .find(|p| p.external_id.as_deref() == Some(id))
            .and_then(|p| p.extra.get("remote"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    assert_eq!(work_type("hybrid-1"), Some("hybrid".to_string()));
    assert_eq!(work_type("remote-1"), Some("remote".to_string()));
    assert_eq!(work_type("onsite-1"), Some("on-site".to_string()));
    assert_eq!(
        work_type("absent-1"),
        None,
        "an undeclared workplaceType must write nothing, not a guessed value"
    );

    // The dual-write regression: `isRemote:true` + `workplaceType:"Hybrid"`
    // must NOT produce `extra.remote == true` — this is the exact shape
    // measured live on Ramp (107 of 136 rows).
    assert!(
        !is_remote("hybrid-1"),
        "a Hybrid row must not also read extra.remote == true, even though isRemote is true"
    );
    assert!(
        is_remote("remote-1"),
        "a genuinely Remote row must still read extra.remote == true"
    );
    assert!(
        !is_remote("onsite-1"),
        "an OnSite row (isRemote:false) must not read extra.remote == true"
    );
    assert!(
        !is_remote("absent-1"),
        "an undeclared workplaceType with isRemote:false must fall back to isRemote and stay false"
    );
}

/// The `isRemote`-only fallback path — no `workplaceType` field at all (older/
/// odd tenants). `extra.remote` must fall back to the raw `isRemote` boolean,
/// and `extra.workType` must stay absent (no declared value to classify).
#[tokio::test]
async fn isremote_only_fallback_when_workplace_type_is_absent() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/posting-api/job-board/legacy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "apiVersion": "1",
            "jobs": [{
                "id": "legacy-remote",
                "title": "Legacy Remote Engineer",
                "locationName": "Anywhere",
                "isRemote": true,
                "jobUrl": "https://jobs.ashbyhq.com/legacy/legacy-remote",
            }],
        })))
        .mount(&server)
        .await;

    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["legacy".to_string()],
    };

    let out = AshbyScraper
        .search_with_base(&server.uri(), input, ctx)
        .await
        .expect("mocked run must succeed");
    let posting = &out[0];
    assert_eq!(
        posting.extra.get("remote").and_then(|v| v.as_bool()),
        Some(true),
        "no workplaceType at all must fall back to the raw isRemote boolean"
    );
    assert!(
        !posting.extra.contains_key("workType"),
        "no declared workplaceType must never produce a guessed workType"
    );
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = AshbyScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["ramp".to_string()], // confirmed live: 112 jobs
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
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
    println!("ashby: {} results", postings.len());
    println!("first: {:?}", first.title);
}
