use super::*;

#[test]
fn test_berlin_startup_jobs_scraper_id() {
    let scraper = BerlinStartupJobsScraper;
    assert_eq!(scraper.id(), "berlinstartupjobs");
}

#[test]
fn test_berlin_startup_jobs_scraper_display_name() {
    let scraper = BerlinStartupJobsScraper;
    assert_eq!(scraper.display_name(), "Berlin Startup Jobs");
}

#[test]
fn test_berlin_startup_jobs_scraper_mode() {
    let scraper = BerlinStartupJobsScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_berlin_startup_jobs_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_berlin_startup_jobs_title_regex() {
    let re = regex::Regex::new(r" at (.+)$").unwrap();
    let title = "Software Engineer at StartupCo";
    let caps = re.captures(title);
    assert!(caps.is_some());
    if let Some(c) = caps {
        assert_eq!(c.get(1).map(|m| m.as_str()), Some("StartupCo"));
    }
}

#[test]
fn test_berlin_startup_jobs_title_regex_no_match() {
    let re = regex::Regex::new(r" at (.+)$").unwrap();
    let title = "Software Engineer";
    let caps = re.captures(title);
    assert!(caps.is_none());
}
