use super::*;

#[test]
fn test_berlin_startup_jobs_scraper_id() {
    let scraper = BerlinStartupJobsScraper;
    assert_eq!(scraper.id(), "berlinstartupjobs");
}

#[test]
fn test_berlin_startup_jobs_scraper_display_name() {
    let scraper = BerlinStartupJobsScraper;
    assert_eq!(scraper.display_name(), "Berlin Startup Jobs");
}

#[test]
fn test_berlin_startup_jobs_scraper_mode() {
    let scraper = BerlinStartupJobsScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_berlin_startup_jobs_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_berlin_startup_jobs_title_regex() {
    let re = regex::Regex::new(r" at (.+)$").unwrap();
    let title = "Software Engineer at StartupCo";
    let caps = re.captures(title);
    assert!(caps.is_some());
    if let Some(c) = caps {
        assert_eq!(c.get(1).map(|m| m.as_str()), Some("StartupCo"));
    }
}

#[test]
fn test_berlin_startup_jobs_title_regex_no_match() {
    let re = regex::Regex::new(r" at (.+)$").unwrap();
    let title = "Software Engineer";
    let caps = re.captures(title);
    assert!(caps.is_none());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = BerlinStartupJobsScraper;
    let input = BoardSearchInput {
        query: "".to_string(),
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
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    };
    let results = scraper.search(input, ctx).await;
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    println!("berlinstartupjobs: {} results", postings.len());
    println!("first: {:?}", first.title);
}
