use super::*;

#[test]
fn test_linkedin_scraper_id() {
    let scraper = LinkedInScraper;
    assert_eq!(scraper.id(), "linkedin");
}

#[test]
fn test_linkedin_scraper_display_name() {
    let scraper = LinkedInScraper;
    assert_eq!(scraper.display_name(), "LinkedIn");
}

#[test]
fn test_linkedin_scraper_mode() {
    let scraper = LinkedInScraper;
    assert_eq!(scraper.mode(), crate::scraping::types::ScraperMode::Http);
}

#[test]
fn test_linkedin_scraper_mode_partial_eq() {
    let mode = crate::scraping::types::ScraperMode::Http;
    assert_eq!(mode, crate::scraping::types::ScraperMode::Http);
    assert_ne!(mode, crate::scraping::types::ScraperMode::Browser);
}
