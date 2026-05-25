use super::*;

#[test]
fn test_personio_scraper_id() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.id(), "personio");
}

#[test]
fn test_personio_scraper_display_name() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.display_name(), "Personio");
}

#[test]
fn test_personio_scraper_mode() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_personio_hosts_not_empty() {
    assert!(!HOSTS.is_empty());
    assert!(HOSTS.contains(&"jobs.personio.de"));
    assert!(HOSTS.contains(&"jobs.personio.com"));
}

#[test]
fn test_personio_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}
