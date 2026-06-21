use super::*;

// Regression: createdAt is already epoch-milliseconds; the old code did `* 1000`
// which produced microseconds (~year 3000). The mapping must carry the value through
// verbatim — no multiplication.
//
// This test uses the production mapper (`map_posted_at`) exposed for testing,
// not a local re-implementation, so any regression in the real mapping path
// is caught here.
#[test]
fn posted_at_carries_epoch_ms_verbatim() {
    // 1_700_000_000_000 ms = 2023-11-14T22:13:20Z — a known, sane date.
    let created_at_ms: i64 = 1_700_000_000_000;
    // Exercise the production mapping path.
    let posted_at = map_posted_at(Some(created_at_ms));

    assert_eq!(
        posted_at,
        Some(1_700_000_000_000_i64),
        "posted_at must equal the raw createdAt epoch-ms value (no * 1000)"
    );

    // Sanity bounds: must fall between year 2000 and year 2100 in epoch-ms.
    // Fails if someone reintroduces * 1000 (pushes into year ~3000)
    // or a / 1000 regression (drops to epoch-seconds ~year 1970).
    const MS_YEAR_2000: i64 = 946_684_800_000;
    const MS_YEAR_2100: i64 = 4_102_444_800_000;
    let v = posted_at.unwrap();
    assert!(
        (MS_YEAR_2000..=MS_YEAR_2100).contains(&v),
        "posted_at {v} is outside [2000, 2100] epoch-ms range — unit error reintroduced?"
    );
}

#[test]
fn posted_at_none_when_created_at_absent() {
    // Exercise the production mapping path with None input.
    let posted_at = map_posted_at(None);
    assert_eq!(
        posted_at, None,
        "posted_at must be None when createdAt is absent"
    );
}

#[test]
fn test_lever_scraper_id() {
    let scraper = LeverScraper;
    assert_eq!(scraper.id(), "lever");
}

#[test]
fn test_lever_scraper_display_name() {
    let scraper = LeverScraper;
    assert_eq!(scraper.display_name(), "Lever");
}

#[test]
fn test_lever_scraper_mode() {
    let scraper = LeverScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_lever_requires_company() {
    assert!(
        LeverScraper.requires_company(),
        "Lever is an ATS board and must return true for requires_company()"
    );
}

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = LeverScraper;
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
        locale: None,
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
    };
    let result = scraper.search(input, ctx).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = LeverScraper;
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
        locale: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["mistral".to_string()], // confirmed live: jobs.lever.co/mistral
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
    println!("lever: {} results", postings.len());
    println!("first: {:?}", first.title);
}
