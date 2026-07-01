use super::*;

#[test]
fn test_greenhouse_scraper_id() {
    let scraper = GreenhouseScraper;
    assert_eq!(scraper.id(), "greenhouse");
}

#[test]
fn test_greenhouse_scraper_display_name() {
    let scraper = GreenhouseScraper;
    assert_eq!(scraper.display_name(), "Greenhouse");
}

#[test]
fn test_greenhouse_scraper_mode() {
    let scraper = GreenhouseScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_greenhouse_requires_company() {
    assert!(
        GreenhouseScraper.requires_company(),
        "Greenhouse is an ATS board and must return true for requires_company()"
    );
}

// ---------------------------------------------------------------------------
// normalize_companies — unit tests (network-free)
// ---------------------------------------------------------------------------

#[test]
fn normalize_drops_blank_entries() {
    let input = vec![
        "airbnb".to_string(),
        "".to_string(),
        "   ".to_string(),
        "\t".to_string(),
        "stripe".to_string(),
    ];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["airbnb", "stripe"]);
}

#[test]
fn normalize_trims_whitespace() {
    let input = vec!["  airbnb  ".to_string(), "\tstripe\n".to_string()];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["airbnb", "stripe"]);
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
    // 60 distinct slugs, cap = 50 (MAX_COMPANIES for greenhouse).
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
    // No companies → early return; no network call can happen.
    let scraper = GreenhouseScraper;
    let input = BoardSearchInput {
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
        companies: Vec::new(), // empty → defensive guard fires
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
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
    let scraper = GreenhouseScraper;
    let input = BoardSearchInput {
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
        // Valid slug that would normally trigger a fetch.
        companies: vec!["airbnb".to_string()],
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
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
/// empty — the scraper produces Ok([]) without attempting any network I/O.
///
/// ponytail: the all-fail Err path (successful_fetches==0 + first_fetch_error
/// set) requires at least one non-blank company that then fails HTTP — not
/// reachable without a real or mocked HTTP layer. The normalisation guarantee
/// is covered by the normalize_companies unit tests above.
#[tokio::test]
async fn all_blank_companies_returns_ok_empty() {
    let scraper = GreenhouseScraper;
    let input = BoardSearchInput {
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
        companies: vec!["".to_string(), "   ".to_string()],
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    };
    let result = scraper.search(input, ctx).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = GreenhouseScraper;
    let input = BoardSearchInput {
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
        companies: vec!["airbnb".to_string()],
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
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
    println!("greenhouse: {} results", postings.len());
    println!("first: {:?}", first.title);
}
