use super::*;

#[test]
fn test_lever_scraper_id() {
    let scraper = LeverScraper;
    assert_eq!(scraper.id(), "lever");
}

#[test]
fn test_lever_scraper_display_name() {
    let scraper = LeverScraper;
    assert_eq!(scraper.display_name(), "Lever");
}

#[test]
fn test_lever_scraper_mode() {
    let scraper = LeverScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}
