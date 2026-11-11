use super::*;

#[test]
fn test_arbeitsagentur_scraper_id() {
    let scraper = ArbeitsagenturScraper;
    assert_eq!(scraper.id(), "arbeitsagentur");
}

#[test]
fn test_arbeitsagentur_scraper_display_name() {
    let scraper = ArbeitsagenturScraper;
    assert_eq!(scraper.display_name(), "Arbeitsagentur");
}

#[test]
fn test_arbeitsagentur_scraper_mode() {
    let scraper = ArbeitsagenturScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_to_base64_url() {
    let scraper = ArbeitsagenturScraper;
    let result = scraper.to_base64_url("test");
    assert!(!result.is_empty());
}

#[test]
fn test_to_base64_url_empty() {
    let scraper = ArbeitsagenturScraper;
    let result = scraper.to_base64_url("");
    assert_eq!(result, "");
}

#[test]
fn test_to_base64_url_special_chars() {
    let scraper = ArbeitsagenturScraper;
    let result = scraper.to_base64_url("test-123_abc");
    assert!(!result.is_empty());
}

#[test]
fn test_arbeitsagentur_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

const LIST_FIXTURE: &str = include_str!("fixtures/list_v6.json");
const DETAIL_FIXTURE: &str = include_str!("fixtures/details_v4.json");

#[test]
fn v6_list_fixture_parses_into_postings_fields() {
    let resp: ListResp = serde_json::from_str(LIST_FIXTURE).unwrap();
    assert!(!resp.is_shape_drift());
    let items = resp.ergebnisliste.unwrap();
    assert_eq!(items.len(), 2);
    let j = &items[0];
    assert_eq!(j.referenznummer, "13635-8d04dc16_JB5288611-S");
    assert_eq!(
        j.titel.as_deref(),
        Some("Senior Frontend Angular Developer (all genders)")
    );
    assert_eq!(j.firma.as_deref(), Some("]init[ AG"));
    assert!(j.externe_url.as_deref().unwrap().starts_with("https://"));
    assert_eq!(j.erste_veroeffentlichung.as_deref(), Some("2026-11-03"));
    let ort = j.stellenlokationen.as_ref().unwrap()[0]
        .adresse
        .as_ref()
        .unwrap();
    assert_eq!(ort.ort.as_deref(), Some("Berlin"));
    // The second item has no externeURL: the deep link falls back to the hash.
    assert!(items[1].externe_url.is_none());
}

#[test]
fn v4_detail_fixture_still_yields_a_description() {
    let d: DetailResp = serde_json::from_str(DETAIL_FIXTURE).unwrap();
    assert!(d.stellenangebots_beschreibung.unwrap().contains("Aufgaben"));
}

#[test]
fn detail_key_is_base64url_of_referenznummer() {
    // Live: v4 /jobdetails/MTAwMDEtMTAwMDA3ODI1OS1T answered 200 for this refnr.
    assert_eq!(
        ArbeitsagenturScraper.to_base64_url("10001-1000078259-S"),
        "MTAwMDEtMTAwMDA3ODI1OS1T"
    );
}

#[test]
fn zero_hit_response_is_not_drift_but_missing_list_is() {
    // Real zero-hit v6 body: no `ergebnisliste`, `maxErgebnisse` 0.
    let empty: ListResp = serde_json::from_str(r#"{"maxErgebnisse":0,"page":1,"size":3}"#).unwrap();
    assert!(!empty.is_shape_drift());
    // The old v4 shape (`stellenangebote`) deserializes to neither -> drift.
    let old: ListResp =
        serde_json::from_str(r#"{"stellenangebote":[],"maxErgebnisse":5}"#).unwrap();
    assert!(old.is_shape_drift());
    let bare: ListResp = serde_json::from_str("{}").unwrap();
    assert!(bare.is_shape_drift());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = ArbeitsagenturScraper;
    let input = BoardSearchInput {
        query: "Softwareentwickler".to_string(),
        location: Some("Berlin".to_string()),
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
    println!("arbeitsagentur: {} results", postings.len());
    println!("first: {:?}", first.title);
}
