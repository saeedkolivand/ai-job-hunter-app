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

#[test]
fn test_parse_company_yc_title() {
    // "Company (YC W24) Is Hiring …" → company extracted from title prefix
    assert_eq!(
        parse_company("Trellis AI (YC W24) hiring a product lead", "hn_user"),
        "Trellis AI"
    );
}

#[test]
fn test_parse_company_no_yc_tag() {
    // No " (YC " marker → fall back to `by`
    assert_eq!(parse_company("Some random job posting", "poster"), "poster");
}

#[test]
fn test_parse_company_english_parenthetical_not_truncated() {
    // "(Senior)" must NOT match as a YC batch marker; falls back to `by`
    assert_eq!(
        parse_company("Engineer (Senior) at Acme", "acme_recruiter"),
        "acme_recruiter"
    );
}

#[test]
fn test_parse_company_yc_marker_at_start_falls_back_to_by() {
    // Title starts with " (YC " making the prefix empty → fall back to `by`
    assert_eq!(
        parse_company(" (YC S21) some posting", "real_user"),
        "real_user"
    );
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = YCombinatorScraper;
    let input = BoardSearchInput {
        query: "engineer".to_string(),
        location: None,
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
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        scraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    println!("ycombinator: {} results", postings.len());
    println!("first: {:?}", first.title);
}
