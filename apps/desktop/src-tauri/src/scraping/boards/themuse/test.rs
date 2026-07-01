use super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_input(query: &str, location: Option<&str>) -> BoardSearchInput {
    BoardSearchInput {
        query: query.to_string(),
        location: location.map(|s| s.to_string()),
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
        companies: Vec::new(),
    }
}

fn make_ctx() -> ScrapeContext {
    ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    }
}

fn jobs_from(json: &str) -> Vec<TmJob> {
    let resp: TmResponse = serde_json::from_str(json).expect("fixture must parse");
    resp.results
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_themuse_scraper_id() {
    let scraper = TheMuseScraper;
    assert_eq!(scraper.id(), "themuse");
}

#[test]
fn test_themuse_scraper_display_name() {
    let scraper = TheMuseScraper;
    assert_eq!(scraper.display_name(), "The Muse");
}

#[test]
fn test_themuse_scraper_mode() {
    let scraper = TheMuseScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

// ---------------------------------------------------------------------------
// is_valid_http_url
// ---------------------------------------------------------------------------

#[test]
fn url_guard_accepts_http_and_https() {
    assert!(is_valid_http_url(
        "https://www.themuse.com/jobs/acme/engineer"
    ));
    assert!(is_valid_http_url(
        "http://www.themuse.com/jobs/acme/engineer"
    ));
}

#[test]
fn url_guard_rejects_non_http_schemes_and_garbage() {
    assert!(!is_valid_http_url("not-a-url"));
    assert!(!is_valid_http_url("ftp://example.com/job"));
    assert!(!is_valid_http_url(""));
}

// ---------------------------------------------------------------------------
// parse_themuse_response — fixture-based parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_themuse_response_happy_path() {
    let json = r#"{
        "results": [
            {
                "name": "Senior Backend Engineer",
                "refs": { "landing_page": "https://www.themuse.com/jobs/acme/senior-backend-engineer" },
                "company": { "name": "Acme Corp" },
                "locations": [ { "name": "Remote (US)" }, { "name": "New York, NY" } ]
            }
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    let p = &postings[0];
    assert_eq!(p.title, "Senior Backend Engineer");
    assert_eq!(
        p.url,
        "https://www.themuse.com/jobs/acme/senior-backend-engineer"
    );
    assert_eq!(p.company, "Acme Corp");
    assert_eq!(p.location, Some("Remote (US)".to_string()));
    assert_eq!(p.id, format!("themuse:{}", p.url));
    assert_eq!(p.external_id, Some(p.url.clone()));
    assert_eq!(p.source, "themuse");
    assert_eq!(p.captured_at, 1_700_000_000_000);
}

#[test]
fn parse_themuse_response_company_falls_back_to_the_muse() {
    let json = r#"{
        "results": [
            {
                "name": "No Company Listing",
                "refs": { "landing_page": "https://www.themuse.com/jobs/x/no-company" },
                "company": null,
                "locations": null
            },
            {
                "name": "Blank Company Name",
                "refs": { "landing_page": "https://www.themuse.com/jobs/x/blank-company" },
                "company": { "name": "   " },
                "locations": null
            }
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 0);
    assert_eq!(postings.len(), 2);
    assert_eq!(postings[0].company, "The Muse");
    assert_eq!(postings[1].company, "The Muse");
}

#[test]
fn parse_themuse_response_location_empty_when_missing_or_empty() {
    let json = r#"{
        "results": [
            {
                "name": "No Locations Field",
                "refs": { "landing_page": "https://www.themuse.com/jobs/x/no-locations" },
                "company": { "name": "Acme" },
                "locations": null
            },
            {
                "name": "Empty Locations Array",
                "refs": { "landing_page": "https://www.themuse.com/jobs/x/empty-locations" },
                "company": { "name": "Acme" },
                "locations": []
            }
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 0);
    assert_eq!(postings.len(), 2);
    assert_eq!(postings[0].location, Some(String::new()));
    assert_eq!(postings[1].location, Some(String::new()));
}

#[test]
fn parse_themuse_response_empty_results_returns_empty_vec() {
    let json = r#"{ "results": [], "page_count": 0 }"#;
    let jobs = jobs_from(json);
    assert!(
        parse_themuse_response(jobs, 0).is_empty(),
        "empty results array must parse to an empty Vec, not an error"
    );
}

/// Missing/empty `name` and missing/invalid `refs.landing_page` each drop the
/// row; valid rows in the same payload must still come through.
#[test]
fn parse_themuse_response_drops_malformed_rows() {
    let json = r#"{
        "results": [
            {"name": "Valid One", "refs": {"landing_page": "https://www.themuse.com/jobs/x/valid-one"}, "company": null, "locations": null},
            {"name": null, "refs": {"landing_page": "https://www.themuse.com/jobs/x/missing-name"}, "company": null, "locations": null},
            {"name": "", "refs": {"landing_page": "https://www.themuse.com/jobs/x/empty-name"}, "company": null, "locations": null},
            {"name": "Missing Refs", "refs": null, "company": null, "locations": null},
            {"name": "Missing Landing Page", "refs": {"landing_page": null}, "company": null, "locations": null},
            {"name": "Malformed URL", "refs": {"landing_page": "not-a-url"}, "company": null, "locations": null},
            {"name": "Valid Two", "refs": {"landing_page": "https://www.themuse.com/jobs/x/valid-two"}, "company": null, "locations": null}
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
}

/// The Muse response has no stable job id — the (validated) posting URL
/// doubles as the id. The parser does not itself dedupe (unlike Pinpoint), so
/// two rows sharing a landing_page produce two postings that share the same
/// id format `themuse:{url}` — the property the DB PK layer relies on to
/// collapse duplicates.
#[test]
fn parse_themuse_response_url_as_id_format_and_identical_urls_share_id() {
    let json = r#"{
        "results": [
            {"name": "First Listing", "refs": {"landing_page": "https://www.themuse.com/jobs/x/dup"}, "company": {"name": "Acme"}, "locations": null},
            {"name": "Duplicate Listing", "refs": {"landing_page": "https://www.themuse.com/jobs/x/dup"}, "company": {"name": "Acme"}, "locations": null},
            {"name": "Distinct Listing", "refs": {"landing_page": "https://www.themuse.com/jobs/x/other"}, "company": {"name": "Acme"}, "locations": null}
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 0);
    assert_eq!(postings.len(), 3, "parser itself does not dedupe rows");

    assert_eq!(
        postings[0].id,
        format!("themuse:{}", postings[0].url),
        "id must be `themuse:{{url}}`"
    );
    assert_eq!(
        postings[0].id, postings[1].id,
        "identical landing_page urls must produce identical ids (DB PK dedupe key)"
    );
    assert_ne!(
        postings[0].id, postings[2].id,
        "distinct urls must produce distinct ids"
    );
}

// ---------------------------------------------------------------------------
// search() — client-side query/location filters (network-free: results come
// from an already-fetched page, so these exercise the filter logic in
// isolation by constructing the haystack the same way `search()` does).
// ---------------------------------------------------------------------------

fn sample_postings() -> Vec<JobPosting> {
    let json = r#"{
        "results": [
            {"name": "Senior Backend Engineer", "refs": {"landing_page": "https://www.themuse.com/jobs/acme/backend"}, "company": {"name": "Acme Corp"}, "locations": [{"name": "Berlin, Germany"}]},
            {"name": "Product Designer", "refs": {"landing_page": "https://www.themuse.com/jobs/globex/designer"}, "company": {"name": "Globex"}, "locations": [{"name": "Remote"}]}
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    parse_themuse_response(jobs, 0)
}

/// Thin filter-a-slice wrapper around the real `matches_filters` — not a
/// reimplementation (that was the bug: a byte-duplicated mirror here could
/// diverge from `search()`'s actual filter and hide a regression). Every
/// test below now exercises the real extracted fn through this.
fn apply_filters(postings: &[JobPosting], query: &str, location: &str) -> Vec<JobPosting> {
    postings
        .iter()
        .filter(|posting| matches_filters(posting, query, location))
        .cloned()
        .collect()
}

#[test]
fn query_filter_matches_title_case_insensitive() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "BACKEND", "");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Senior Backend Engineer");
}

#[test]
fn query_filter_matches_company_case_insensitive() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "globex", "");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].company, "Globex");
}

#[test]
fn query_filter_empty_returns_all() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "", "");
    assert_eq!(filtered.len(), 2);
}

#[test]
fn query_filter_matching_nothing_returns_empty() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "nonexistent-role", "");
    assert!(filtered.is_empty());
}

#[test]
fn location_filter_substring_match_case_insensitive() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "", "berlin");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Senior Backend Engineer");
}

#[test]
fn location_filter_blank_passes_through_with_active_query() {
    // A blank location must not itself exclude anything — combined with an
    // active query, only the query clause should narrow the result.
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "engineer", "");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Senior Backend Engineer");
}

#[test]
fn query_and_location_filters_combine_with_and() {
    let postings = sample_postings();
    // Title matches "engineer" but location does not match "remote" -> excluded.
    let filtered = apply_filters(&postings, "engineer", "remote");
    assert!(filtered.is_empty());
    // Both match -> included.
    let filtered = apply_filters(&postings, "engineer", "berlin");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Senior Backend Engineer");
}

// ---------------------------------------------------------------------------
// total_pages / page budget clamp — pure arithmetic, no network needed.
// ---------------------------------------------------------------------------

#[test]
fn page_count_clamps_to_at_least_one() {
    // `resp.page_count.max(1)` — a zero/missing page_count must not zero out
    // the loop bound (which would make `page >= total_pages` true immediately
    // on page 1+ but still allow page 0 through). Values come from real
    // fixture deserialization, not literals, so this exercises the actual
    // `#[serde(default)]` behaviour, not just arithmetic.
    let missing: TmResponse = serde_json::from_str(r#"{"results": []}"#).unwrap();
    assert_eq!(missing.page_count, 0);
    assert_eq!(missing.page_count.max(1), 1);

    let zero: TmResponse = serde_json::from_str(r#"{"results": [], "page_count": 0}"#).unwrap();
    assert_eq!(zero.page_count.max(1), 1);

    let three: TmResponse = serde_json::from_str(r#"{"results": [], "page_count": 3}"#).unwrap();
    assert_eq!(three.page_count.max(1), 3);
}

#[test]
fn requested_pages_clamps_into_one_to_max_pages_range() {
    let want_zero = BoardSearchInput {
        pages: 0,
        ..make_input("", None)
    };
    assert_eq!(
        want_zero.pages.clamp(1, MAX_PAGES),
        1,
        "0 requested pages must clamp up to 1"
    );

    let want_one = BoardSearchInput {
        pages: 1,
        ..make_input("", None)
    };
    assert_eq!(want_one.pages.clamp(1, MAX_PAGES), 1);

    let want_oversized = BoardSearchInput {
        pages: 100,
        ..make_input("", None)
    };
    assert_eq!(
        want_oversized.pages.clamp(1, MAX_PAGES),
        MAX_PAGES,
        "an oversized request must clamp down to MAX_PAGES"
    );
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

/// A pre-cancelled signal must make the loop break immediately without
/// attempting a fetch, returning Ok with an empty Vec.
#[tokio::test]
async fn cancelled_before_fetch_returns_ok_empty() {
    let scraper = TheMuseScraper;
    let ctx = make_ctx();
    ctx.signal.cancel();
    let result = scraper.search(make_input("", None), ctx).await;
    assert!(
        result.is_ok(),
        "cancelled run must return Ok, not Err: {:?}",
        result.err()
    );
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = TheMuseScraper;
    let input = make_input("engineer", None);
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
    println!("themuse: {} results", postings.len());
    println!("first: {:?}", first.title);
}
