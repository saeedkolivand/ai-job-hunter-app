use super::*;

#[test]
fn test_remotive_scraper_id() {
    let scraper = RemotiveScraper;
    assert_eq!(scraper.id(), "remotive");
}

#[test]
fn test_remotive_scraper_display_name() {
    let scraper = RemotiveScraper;
    assert_eq!(scraper.display_name(), "Remotive");
}

#[test]
fn test_remotive_scraper_mode() {
    let scraper = RemotiveScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}
