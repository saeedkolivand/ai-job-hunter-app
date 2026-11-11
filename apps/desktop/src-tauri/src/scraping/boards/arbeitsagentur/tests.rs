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

#[test]
fn page_one_without_list_and_nonzero_total_is_an_error() {
    let drift: ListResp = serde_json::from_str(r#"{"maxErgebnisse":7,"page":1}"#).unwrap();
    assert!(matches!(drift.check_first_page(), Err(AppError::Parse(_))));
    let zero: ListResp = serde_json::from_str(r#"{"maxErgebnisse":0}"#).unwrap();
    assert!(zero.check_first_page().is_ok());
}

#[test]
fn posted_at_prefers_the_current_publication_window() {
    let items = serde_json::from_str::<ListResp>(LIST_FIXTURE)
        .unwrap()
        .ergebnisliste
        .unwrap();
    // Real specimen: veroeffentlichungszeitraum.von 2024-04-11 vs first 2024-04-08.
    let ms = |d: &str| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis()
    };
    assert_eq!(items[1].posted_at_ms(), Some(ms("2024-04-11")));
    let mut no_window = items.into_iter().nth(1).unwrap();
    no_window.veroeffentlichungszeitraum = None;
    assert_eq!(no_window.posted_at_ms(), Some(ms("2024-04-08")));
}

#[test]
fn location_drops_duplicate_region_and_title_cases_upper_case_text() {
    let a = |o: &str, r: &str, l: &str| Adresse {
        ort: Some(o.into()),
        region: Some(r.into()),
        land: Some(l.into()),
    };
    assert_eq!(
        format_location(Some(&a("Berlin", "BERLIN", "DEUTSCHLAND"))),
        "Berlin, Deutschland"
    );
    assert_eq!(
        format_location(Some(&a("Leipzig", "SACHSEN", "DEUTSCHLAND"))),
        "Leipzig, Sachsen, Deutschland"
    );
    assert_eq!(format_location(None), "");
}

#[test]
fn real_multi_word_regions_render_readably_and_match_the_requested_region() {
    use crate::scraping::engine::location_filter::{location_verdict, LocationVerdict};
    use crate::scraping::types::LocationSpec;
    for (ort, region, shown, requested) in [
        (
            "Köln",
            "NORDRHEIN_WESTFALEN",
            "Nordrhein-Westfalen",
            "Nordrhein-Westfalen",
        ),
        (
            "Stuttgart",
            "BADEN_WUERTTEMBERG",
            "Baden-Württemberg",
            "Baden-Württemberg",
        ),
        (
            "Stuttgart",
            "BADEN_WUERTTEMBERG",
            "Baden-Württemberg",
            "Baden-Wuerttemberg",
        ),
    ] {
        let loc = format_location(Some(&Adresse {
            ort: Some(ort.into()),
            region: Some(region.into()),
            land: Some("DEUTSCHLAND".into()),
        }));
        assert_eq!(loc, format!("{ort}, {shown}, Deutschland"));
        let spec = LocationSpec {
            region: Some(requested.into()),
            ..Default::default()
        };
        assert_eq!(
            location_verdict(Some(&loc), false, &spec),
            LocationVerdict::PlaceMatch,
            "{requested} must place-match {loc}"
        );
    }
}
