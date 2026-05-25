use super::*;

#[test]
fn test_wwr_scraper_id() {
    let scraper = WeWorkRemotelyScraper;
    assert_eq!(scraper.id(), "wwr");
}

#[test]
fn test_wwr_scraper_display_name() {
    let scraper = WeWorkRemotelyScraper;
    assert_eq!(scraper.display_name(), "We Work Remotely");
}

#[test]
fn test_wwr_scraper_mode() {
    let scraper = WeWorkRemotelyScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_wwr_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_wwr_title_split() {
    let title = "Company: Senior Engineer";
    let split: Vec<&str> = title.splitn(2, ": ").collect();
    assert_eq!(split.len(), 2);
    assert_eq!(split[0], "Company");
    assert_eq!(split[1], "Senior Engineer");
}

#[test]
fn test_wwr_title_split_no_colon() {
    let title = "Senior Engineer";
    let split: Vec<&str> = title.splitn(2, ": ").collect();
    assert_eq!(split.len(), 1);
    assert_eq!(split[0], "Senior Engineer");
}

#[test]
fn test_wwr_title_split_multiple_colons() {
    let title = "Company: Senior: Engineer";
    let split: Vec<&str> = title.splitn(2, ": ").collect();
    assert_eq!(split.len(), 2);
    assert_eq!(split[0], "Company");
    assert_eq!(split[1], "Senior: Engineer");
}
