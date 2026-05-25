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
