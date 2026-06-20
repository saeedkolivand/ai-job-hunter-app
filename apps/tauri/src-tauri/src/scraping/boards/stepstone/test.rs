use super::*;

#[test]
fn test_stepstone_scraper_id() {
    let scraper = StepStoneScraper;
    assert_eq!(scraper.id(), "stepstone");
}

#[test]
fn test_stepstone_scraper_display_name() {
    let scraper = StepStoneScraper;
    assert_eq!(scraper.display_name(), "StepStone");
}

#[test]
fn test_stepstone_scraper_mode() {
    let scraper = StepStoneScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_stepstone_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_ld_json_regex() {
    let re = regex::Regex::new(r#"<script type="application/ld\+json">(.*?)</script>"#).unwrap();
    let html = r#"<script type="application/ld+json">{"test": true}</script>"#;
    let caps = re.captures(html);
    assert!(caps.is_some());
}

#[test]
fn test_stepstone_id_extraction_with_query() {
    let url = "https://www.stepstone.de/job?id=123456&other=param";
    let id = regex::Regex::new(r"[?&]ID=([^&]+)")
        .unwrap()
        .captures(url)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
    // Case-sensitive, so this won't match
    assert!(id.is_none());
}

#[test]
fn test_stepstone_id_extraction_uppercase() {
    let url = "https://www.stepstone.de/job?ID=123456&other=param";
    let id = regex::Regex::new(r"[?&]ID=([^&]+)")
        .unwrap()
        .captures(url)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
    assert_eq!(id, Some("123456".to_string()));
}

#[test]
fn test_stepstone_id_extraction_six_digits() {
    let url = "https://www.stepstone.de/job/1234567";
    let id = regex::Regex::new(r"(\d{6,})")
        .unwrap()
        .captures(url)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
    assert_eq!(id, Some("1234567".to_string()));
}

#[test]
fn test_stepstone_id_extraction_short_digits() {
    let url = "https://www.stepstone.de/job/123";
    let id = regex::Regex::new(r"(\d{6,})")
        .unwrap()
        .captures(url)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
    assert!(id.is_none());
}

#[test]
fn test_hiring_organization_struct() {
    let org = HiringOrganization {
        name: Some("Test Corp".to_string()),
    };
    assert_eq!(org.name, Some("Test Corp".to_string()));
}

#[test]
fn test_address_struct() {
    let addr = Address {
        address_locality: Some("Berlin".to_string()),
        address_country: Some("Germany".to_string()),
    };
    assert_eq!(addr.address_locality, Some("Berlin".to_string()));
}

#[test]
fn test_job_location_struct() {
    let loc = JobLocation {
        address: Some(Address {
            address_locality: Some("Berlin".to_string()),
            address_country: None,
        }),
    };
    assert!(loc.address.is_some());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = StepStoneScraper;
    // NOTE: StepStone is bot-sensitive (timeout/403 from certain IPs / CI).
    // A failure here is an infrastructure issue, NOT a code bug in the scraper.
    let input = BoardSearchInput {
        query: "software".to_string(),
        location: Some("Berlin".to_string()),
        amount: 10,
        pages: 1,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        locale: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    };
    let results = scraper.search(input, ctx).await;
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    println!("stepstone: {} results", postings.len());
    println!("first: {:?}", first.title);
}
