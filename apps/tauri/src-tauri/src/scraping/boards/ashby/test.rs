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

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = AshbyScraper;
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
    let scraper = AshbyScraper;
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
        companies: vec!["ramp".to_string()], // confirmed live: 112 jobs
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
    println!("ashby: {} results", postings.len());
    println!("first: {:?}", first.title);
}
