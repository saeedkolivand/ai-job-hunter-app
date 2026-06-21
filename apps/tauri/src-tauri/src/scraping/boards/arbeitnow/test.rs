use super::*;

#[test]
fn test_arbeitnow_scraper_id() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.id(), "arbeitnow");
}

#[test]
fn test_arbeitnow_scraper_display_name() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.display_name(), "Arbeitnow");
}

#[test]
fn test_arbeitnow_scraper_mode() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = ArbeitnowScraper;
    let input = BoardSearchInput {
        query: "engineer".to_string(),
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
    println!("arbeitnow: {} results", postings.len());
    println!("first: {:?}", first.title);
}
