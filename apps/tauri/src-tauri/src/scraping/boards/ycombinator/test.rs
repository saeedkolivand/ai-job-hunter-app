use super::*;

#[test]
fn test_ycombinator_scraper_id() {
    let scraper = YCombinatorScraper;
    assert_eq!(scraper.id(), "ycombinator");
}

#[test]
fn test_ycombinator_scraper_display_name() {
    let scraper = YCombinatorScraper;
    assert_eq!(scraper.display_name(), "Y Combinator");
}

#[test]
fn test_ycombinator_scraper_mode() {
    let scraper = YCombinatorScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}
