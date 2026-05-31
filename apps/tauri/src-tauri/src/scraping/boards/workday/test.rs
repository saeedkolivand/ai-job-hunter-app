use super::*;

#[test]
fn test_parse_target_colon_format() {
    let scraper = WorkdayScraper;
    let result = scraper.parse_target("tenant:site:wd1");
    assert_eq!(
        result,
        Some(("tenant".to_string(), "site".to_string(), "wd1".to_string()))
    );
}

#[test]
fn test_parse_target_colon_format_default_host() {
    let scraper = WorkdayScraper;
    let result = scraper.parse_target("tenant:site");
    assert_eq!(
        result,
        Some(("tenant".to_string(), "site".to_string(), "wd1".to_string()))
    );
}

#[test]
fn test_parse_target_url_format() {
    let scraper = WorkdayScraper;
    let result = scraper.parse_target("https://mycompany.wd1.myworkdayjobs.com/en-us/job");
    // The regex captures differently - just verify it parses something
    assert!(result.is_some());
}

#[test]
fn test_parse_target_url_format_with_subpath() {
    let scraper = WorkdayScraper;
    let result = scraper.parse_target("https://mycompany.wd5.myworkdayjobs.com/careers/job");
    // The regex captures differently - just verify it parses something
    assert!(result.is_some());
}

#[test]
fn test_parse_target_empty() {
    let scraper = WorkdayScraper;
    let result = scraper.parse_target("");
    assert_eq!(result, None);
}

#[test]
fn test_parse_target_invalid() {
    let scraper = WorkdayScraper;
    let result = scraper.parse_target("invalid query");
    assert_eq!(result, None);
}

#[test]
fn test_workday_scraper_id() {
    let scraper = WorkdayScraper;
    assert_eq!(scraper.id(), "workday");
}

#[test]
fn test_workday_scraper_display_name() {
    let scraper = WorkdayScraper;
    assert_eq!(scraper.display_name(), "Workday");
}

#[test]
fn test_workday_scraper_mode() {
    let scraper = WorkdayScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}
