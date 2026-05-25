use super::*;

#[test]
fn test_extract_jk_from_href_standard() {
    let href = "/viewjob?jk=abc123&from=web";
    let result = extract_jk_from_href(href);
    assert_eq!(result, Some("abc123"));
}

#[test]
fn test_extract_jk_from_href_rc() {
    let href = "/rc/clk?jk=xyz789&from=serp";
    let result = extract_jk_from_href(href);
    assert_eq!(result, Some("xyz789"));
}

#[test]
fn test_extract_jk_from_href_no_jk() {
    let href = "/viewjob?from=web";
    let result = extract_jk_from_href(href);
    assert_eq!(result, None);
}

#[test]
fn test_extract_jk_from_href_empty() {
    let href = "";
    let result = extract_jk_from_href(href);
    assert_eq!(result, None);
}

#[test]
fn test_extract_jk_from_href_no_ampersand() {
    let href = "/viewjob?jk=abc123";
    let result = extract_jk_from_href(href);
    assert_eq!(result, Some("abc123"));
}

#[test]
fn test_indeed_scraper_id() {
    let scraper = IndeedScraper;
    assert_eq!(scraper.id(), "indeed");
}

#[test]
fn test_indeed_scraper_display_name() {
    let scraper = IndeedScraper;
    assert_eq!(scraper.display_name(), "Indeed");
}

#[test]
fn test_indeed_scraper_mode() {
    let scraper = IndeedScraper;
    assert_eq!(scraper.mode(), ScraperMode::Browser);
}

#[test]
fn test_indeed_scraper_mode_partial_eq() {
    let mode = ScraperMode::Browser;
    assert_eq!(mode, ScraperMode::Browser);
    assert_ne!(mode, ScraperMode::Http);
}

#[test]
fn test_indeed_domains_not_empty() {
    assert!(!INDEED_DOMAINS.is_empty());
    assert!(INDEED_DOMAINS.contains(&("us", "www.indeed.com")));
    assert!(INDEED_DOMAINS.contains(&("de", "de.indeed.com")));
}

#[test]
fn test_indeed_domains_count() {
    assert_eq!(INDEED_DOMAINS.len(), 17);
}

#[test]
fn test_extract_jk_from_href_with_fragment() {
    let href = "/viewjob?jk=abc123&from=web#section";
    let result = extract_jk_from_href(href);
    assert_eq!(result, Some("abc123"));
}

#[test]
fn test_extract_jk_from_href_multiple_params() {
    let href = "/viewjob?from=web&vjk=xyz789&jk=abc123";
    let result = extract_jk_from_href(href);
    // Function returns first jk match
    assert_eq!(result, Some("xyz789"));
}

#[test]
fn test_extract_jk_from_href_encoded() {
    let href = "/viewjob?jk=abc%20123&from=web";
    let result = extract_jk_from_href(href);
    assert_eq!(result, Some("abc%20123"));
}
