use super::*;
use crate::scraping::types::{BoardSearchInput, ScrapeContext, Scraper};

#[test]
fn test_linkedin_scraper_id() {
    let scraper = LinkedInScraper;
    assert_eq!(scraper.id(), "linkedin");
}

#[test]
fn test_linkedin_scraper_display_name() {
    let scraper = LinkedInScraper;
    assert_eq!(scraper.display_name(), "LinkedIn");
}

#[test]
fn test_linkedin_scraper_mode() {
    let scraper = LinkedInScraper;
    assert_eq!(scraper.mode(), crate::scraping::types::ScraperMode::Http);
}

#[test]
fn test_linkedin_scraper_mode_partial_eq() {
    let mode = crate::scraping::types::ScraperMode::Http;
    assert_eq!(mode, crate::scraping::types::ScraperMode::Http);
    assert_ne!(mode, crate::scraping::types::ScraperMode::Browser);
}

// ── country-biased geoId selection (trust PR G, #49) ─────────────────────────

#[test]
fn country_aliases_known_and_unknown() {
    assert!(country_aliases("de").is_some());
    assert!(
        country_aliases("uk").is_some(),
        "uk must alias united kingdom"
    );
    assert!(
        country_aliases("gb").is_some(),
        "gb must alias united kingdom"
    );
    assert!(
        country_aliases("be").is_some(),
        "be must alias belgium (Adzuna allowlist parity)"
    );
    assert!(
        country_aliases("za").is_some(),
        "za must alias south africa (Adzuna allowlist parity)"
    );
    assert!(country_aliases("zz").is_none(), "unknown code → no bias");
    assert!(country_aliases("").is_none(), "empty code → no bias");
}

/// The whole point of #49: "Berlin" typeaheads to Germany first AND a US
/// "Berlin"; a `us` search must pick the US hit, a `de` search the German one —
/// not blindly the first hit.
#[test]
fn select_geo_id_biases_to_requested_country() {
    let hits: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
        {"id": "103035651", "displayName": "Berlin, Germany"},
        {"id": "105506608", "displayName": "Berlin, Connecticut, United States"}
    ]"#,
    )
    .unwrap();
    assert_eq!(
        select_geo_id(&hits, Some("us")).as_deref(),
        Some("105506608")
    );
    assert_eq!(
        select_geo_id(&hits, Some("de")).as_deref(),
        Some("103035651")
    );
}

/// PR #603 review: matching must anchor to the TRAILING country segment
/// (`ends_with`), not a substring anywhere (`contains`) — "india" is a
/// substring of "Indiana, United States" and "ireland" is a substring of
/// "Northern Ireland, United Kingdom", so a naive `contains` false-positives
/// an `in`/`ie` search onto the wrong hit.
#[test]
fn select_geo_id_does_not_substring_match_country_name_inside_a_place_name() {
    let hits: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
        {"id": "1", "displayName": "Indianapolis, Indiana, United States"},
        {"id": "2", "displayName": "Mumbai, India"}
    ]"#,
    )
    .unwrap();
    assert_eq!(
        select_geo_id(&hits, Some("in")).as_deref(),
        Some("2"),
        "'in' must pick the real India hit, not fall for 'Indiana' containing 'india'"
    );

    let hits: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
        {"id": "3", "displayName": "Belfast, Northern Ireland, United Kingdom"},
        {"id": "4", "displayName": "Dublin, Ireland"}
    ]"#,
    )
    .unwrap();
    assert_eq!(
        select_geo_id(&hits, Some("ie")).as_deref(),
        Some("4"),
        "'ie' must pick the real Ireland hit, not fall for 'Northern Ireland' containing 'ireland'"
    );
}

/// No country, an unlisted code, or no matching displayName all fall back to the
/// first usable hit — the prior behaviour, so a resolvable location never regresses.
#[test]
fn select_geo_id_falls_back_to_first_hit() {
    let hits: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
        {"id": "111", "displayName": "Somewhere, Nowhereland"},
        {"id": "222", "displayName": "Elsewhere, Germany"}
    ]"#,
    )
    .unwrap();
    assert_eq!(select_geo_id(&hits, None).as_deref(), Some("111"));
    assert_eq!(
        select_geo_id(&hits, Some("fr")).as_deref(),
        Some("111"),
        "a country with no matching hit falls back to the first hit"
    );
    assert_eq!(select_geo_id(&hits, Some("")).as_deref(), Some("111"));
}

/// A numeric `id` (LinkedIn returns both) must resolve; empty hits → None.
#[test]
fn select_geo_id_numeric_id_and_empty() {
    let hits: Vec<serde_json::Value> =
        serde_json::from_str(r#"[{"id": 103035651, "displayName": "Berlin, Germany"}]"#).unwrap();
    assert_eq!(
        select_geo_id(&hits, Some("de")).as_deref(),
        Some("103035651")
    );
    assert!(select_geo_id(&[], Some("de")).is_none());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = LinkedInScraper;
    let input = BoardSearchInput {
        query: "software engineer".to_string(),
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
    println!("linkedin: {} results", postings.len());
    println!("first: {:?}", first.title);
}
