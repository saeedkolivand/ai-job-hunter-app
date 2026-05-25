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
