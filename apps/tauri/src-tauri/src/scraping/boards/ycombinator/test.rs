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
fn test_hn_item_company_extraction() {
    // "Company (YC S24) Is Hiring …" → company = "Company"
    let title = "Trellis AI (YC W24) hiring a product lead";
    let pos = title.find(" (YC ");
    assert!(pos.is_some());
    let company = &title[..pos.unwrap()];
    assert_eq!(company, "Trellis AI");
}

#[test]
fn test_hn_item_no_yc_tag() {
    let title = "Some random job posting";
    let pos = title.find(" (YC ");
    assert!(pos.is_none());
}

#[test]
fn test_hn_item_english_parenthetical_not_truncated() {
    // "(Senior)" looks like " (S" but must NOT be treated as a YC batch marker.
    // Company extraction should fall through to `by` field, not produce garbage.
    let title = "Engineer (Senior) at Acme";
    let pos = title.find(" (YC ");
    // No YC prefix → no extraction from title
    assert!(pos.is_none(), "title should not match YC prefix");
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
    let results = scraper.search(input, ctx).await;
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    println!("ycombinator: {} results", postings.len());
    println!("first: {:?}", first.title);
}
