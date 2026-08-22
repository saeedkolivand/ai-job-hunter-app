use super::*;

#[test]
fn test_arbeitnow_scraper_id() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.id(), "arbeitnow");
}

#[test]
fn test_arbeitnow_scraper_display_name() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.display_name(), "Arbeitnow");
}

#[test]
fn test_arbeitnow_scraper_mode() {
    let scraper = ArbeitnowScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

// ---------------------------------------------------------------------------
// arbeitnow_work_type — Some(false) must never become OnSite
// ---------------------------------------------------------------------------

#[test]
fn work_type_true_maps_to_remote() {
    assert_eq!(arbeitnow_work_type(Some(true)), Some("remote"));
}

/// The under-populated-field guard: `remote:false` on this board is measured
/// to co-occur with titles like "Germany Remote" / "Berlin, Hybrid" — must
/// write nothing, never a guessed OnSite.
#[test]
fn work_type_false_writes_nothing_never_on_site() {
    assert_eq!(arbeitnow_work_type(Some(false)), None);
}

#[test]
fn work_type_absent_writes_nothing() {
    assert_eq!(arbeitnow_work_type(None), None);
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = ArbeitnowScraper;
    let input = BoardSearchInput {
        query: "engineer".to_string(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
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
    println!("arbeitnow: {} results", postings.len());
    println!("first: {:?}", first.title);
}
