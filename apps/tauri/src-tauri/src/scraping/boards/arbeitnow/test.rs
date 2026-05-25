use super::*;

#[test]
fn test_arbeitnow_scraper_id() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.id(), "arbeitnow");
}

#[test]
fn test_arbeitnow_scraper_display_name() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.display_name(), "Arbeitnow");
}

#[test]
fn test_arbeitnow_scraper_mode() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}
