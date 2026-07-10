use super::*;

#[test]
fn test_wwr_scraper_id() {
    let scraper = WeWorkRemotelyScraper;
    assert_eq!(scraper.id(), "wwr");
}

#[test]
fn test_wwr_scraper_display_name() {
    let scraper = WeWorkRemotelyScraper;
    assert_eq!(scraper.display_name(), "We Work Remotely");
}

#[test]
fn test_wwr_scraper_mode() {
    let scraper = WeWorkRemotelyScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_wwr_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_wwr_title_split() {
    let title = "Company: Senior Engineer";
    let split: Vec<&str> = title.splitn(2, ": ").collect();
    assert_eq!(split.len(), 2);
    assert_eq!(split[0], "Company");
    assert_eq!(split[1], "Senior Engineer");
}

#[test]
fn test_wwr_title_split_no_colon() {
    let title = "Senior Engineer";
    let split: Vec<&str> = title.splitn(2, ": ").collect();
    assert_eq!(split.len(), 1);
    assert_eq!(split[0], "Senior Engineer");
}

#[test]
fn test_wwr_title_split_multiple_colons() {
    let title = "Company: Senior: Engineer";
    let split: Vec<&str> = title.splitn(2, ": ").collect();
    assert_eq!(split.len(), 2);
    assert_eq!(split[0], "Company");
    assert_eq!(split[1], "Senior: Engineer");
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = WeWorkRemotelyScraper;
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
    println!("wwr: {} results", postings.len());
    println!("first: {:?}", first.title);
}
