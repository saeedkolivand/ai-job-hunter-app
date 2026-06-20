use super::*;

#[test]
fn test_recruitee_scraper_id() {
    let scraper = RecruiteeScraper;
    assert_eq!(scraper.id(), "recruitee");
}

#[test]
fn test_recruitee_scraper_display_name() {
    let scraper = RecruiteeScraper;
    assert_eq!(scraper.display_name(), "Recruitee");
}

#[test]
fn test_recruitee_scraper_mode() {
    let scraper = RecruiteeScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_recruitee_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_offer_struct_fields() {
    let offer = Offer {
        id: 123,
        slug: "test-job".to_string(),
        title: "Software Engineer".to_string(),
        description: Some("Test description".to_string()),
        requirements: Some("Test requirements".to_string()),
        careers_url: "https://example.com".to_string(),
        city: Some("Berlin".to_string()),
        country: Some("Germany".to_string()),
        remote: Some(true),
        created_at: None,
        company_name: Some("Test Corp".to_string()),
    };
    assert_eq!(offer.id, 123);
    assert_eq!(offer.title, "Software Engineer");
    assert_eq!(offer.remote, Some(true));
}

#[test]
fn test_offer_struct_defaults() {
    let offer = Offer {
        id: 123,
        slug: "test-job".to_string(),
        title: "Software Engineer".to_string(),
        description: None,
        requirements: None,
        careers_url: "https://example.com".to_string(),
        city: None,
        country: None,
        remote: None,
        created_at: None,
        company_name: None,
    };
    assert!(offer.description.is_none());
    assert!(offer.remote.is_none());
}

#[test]
fn test_resp_struct() {
    let resp = Resp { offers: vec![] };
    assert!(resp.offers.is_empty());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = RecruiteeScraper;
    let input = BoardSearchInput {
        query: "personio".to_string(), // confirmed live: personio.recruitee.com has offers
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
    println!("recruitee: {} results", postings.len());
    println!("first: {:?}", first.title);
}
