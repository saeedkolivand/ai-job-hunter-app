use super::*;

#[test]
fn test_smartrecruiters_scraper_id() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.id(), "smartrecruiters");
}

#[test]
fn test_smartrecruiters_scraper_display_name() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.display_name(), "SmartRecruiters");
}

#[test]
fn test_smartrecruiters_scraper_mode() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_location_struct_fields() {
    let location = Location {
        city: Some("Berlin".to_string()),
        country: Some("Germany".to_string()),
        remote: Some(true),
    };
    assert_eq!(location.city, Some("Berlin".to_string()));
    assert_eq!(location.remote, Some(true));
}

#[test]
fn test_location_struct_defaults() {
    let location = Location {
        city: None,
        country: None,
        remote: None,
    };
    assert!(location.city.is_none());
    assert!(location.remote.is_none());
}

#[test]
fn test_posting_struct_fields() {
    let posting = Posting {
        id: "123".to_string(),
        uuid: Some("abc".to_string()),
        name: "Software Engineer".to_string(),
        location: None,
        released_date: None,
        ref_field: None,
    };
    assert_eq!(posting.id, "123");
    assert_eq!(posting.name, "Software Engineer");
}

#[test]
fn test_smartrecruiters_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = SmartRecruitersScraper;
    let input = BoardSearchInput {
        query: "Visa".to_string(), // confirmed live: 7 postings via SmartRecruiters API
        location: None,
        amount: 5,
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
    println!("smartrecruiters: {} results", postings.len());
    println!("first: {:?}", first.title);
}
