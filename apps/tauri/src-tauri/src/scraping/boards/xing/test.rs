use super::*;

#[test]
fn test_extract_id_from_href_standard() {
    let href = "/jobs/software-engineer-abc123";
    let result = extract_id_from_href(href);
    assert_eq!(result, "software-engineer-abc123");
}

#[test]
fn test_extract_id_from_href_with_query() {
    let href = "/jobs/software-engineer-abc123?param=value";
    let result = extract_id_from_href(href);
    assert_eq!(result, "software-engineer-abc123");
}

#[test]
fn test_extract_id_from_href_trailing_slash() {
    let href = "/jobs/software-engineer-abc123/";
    let result = extract_id_from_href(href);
    assert_eq!(result, "software-engineer-abc123");
}

#[test]
fn test_extract_id_from_href_empty() {
    let href = "";
    let result = extract_id_from_href(href);
    assert_eq!(result, "");
}

#[test]
fn test_extract_id_from_href_no_slash() {
    let href = "software-engineer-abc123";
    let result = extract_id_from_href(href);
    assert_eq!(result, "software-engineer-abc123");
}

#[test]
fn test_xing_scraper_id() {
    let scraper = XingScraper;
    assert_eq!(scraper.id(), "xing");
}

#[test]
fn test_xing_scraper_display_name() {
    let scraper = XingScraper;
    assert_eq!(scraper.display_name(), "Xing");
}

#[test]
fn test_xing_scraper_mode() {
    let scraper = XingScraper;
    assert_eq!(scraper.mode(), ScraperMode::Browser);
}

#[test]
fn test_xing_scraper_mode_partial_eq() {
    let mode = ScraperMode::Browser;
    assert_eq!(mode, ScraperMode::Browser);
    assert_ne!(mode, ScraperMode::Http);
}

#[test]
fn test_extract_id_from_href_with_fragment() {
    let href = "/jobs/software-engineer-abc123#section";
    let result = extract_id_from_href(href);
    // Function doesn't strip fragments
    assert_eq!(result, "software-engineer-abc123#section");
}

#[test]
fn test_extract_id_from_href_with_multiple_slashes() {
    let href = "/jobs/software-engineer-abc123/extra/path";
    let result = extract_id_from_href(href);
    // Function takes the last segment
    assert_eq!(result, "path");
}

#[test]
fn test_extract_id_from_href_with_special_chars() {
    let href = "/jobs/senior-developer-frontend-react-123";
    let result = extract_id_from_href(href);
    assert_eq!(result, "senior-developer-frontend-react-123");
}
