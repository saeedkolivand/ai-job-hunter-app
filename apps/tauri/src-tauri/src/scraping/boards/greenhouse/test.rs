use super::*;

#[test]
fn test_greenhouse_scraper_id() {
    let scraper = GreenhouseScraper;
    assert_eq!(scraper.id(), "greenhouse");
}

#[test]
fn test_greenhouse_scraper_display_name() {
    let scraper = GreenhouseScraper;
    assert_eq!(scraper.display_name(), "Greenhouse");
}

#[test]
fn test_greenhouse_scraper_mode() {
    let scraper = GreenhouseScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}
