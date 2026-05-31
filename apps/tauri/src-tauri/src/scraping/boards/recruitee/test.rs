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
