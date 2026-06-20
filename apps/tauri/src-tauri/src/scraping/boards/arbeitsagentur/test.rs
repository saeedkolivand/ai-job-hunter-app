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

#[test]
fn test_arbeitsort_struct() {
    let ort = Arbeitsort {
        ort: Some("Berlin".to_string()),
        region: Some("Berlin".to_string()),
        land: Some("Germany".to_string()),
    };
    assert_eq!(ort.ort, Some("Berlin".to_string()));
}

#[test]
fn test_arbeitsort_struct_defaults() {
    let ort = Arbeitsort {
        ort: None,
        region: None,
        land: None,
    };
    assert!(ort.ort.is_none());
}

#[test]
fn test_stellenangebot_struct() {
    let offer = Stellenangebot {
        refnr: "123456".to_string(),
        titel: Some("Software Engineer".to_string()),
        beruf: None,
        arbeitgeber: Some("Test Corp".to_string()),
        arbeitsort: None,
        aktuelle_veroeffentlichungsdatum: None,
        eintrittsdatum: None,
        externe_url: None,
        hash_id: None,
    };
    assert_eq!(offer.refnr, "123456");
    assert_eq!(offer.titel, Some("Software Engineer".to_string()));
}

#[test]
fn test_branche_struct() {
    let branche = Branche {
        bezeichnung: Some("IT".to_string()),
    };
    assert_eq!(branche.bezeichnung, Some("IT".to_string()));
}

#[test]
fn test_list_resp_struct() {
    let resp = ListResp {
        stellenangebote: Some(vec![]),
        max_ergebnisse: Some(100),
    };
    assert!(resp
        .stellenangebote
        .as_ref()
        .map(|v| v.is_empty())
        .unwrap_or(true));
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
    println!("arbeitsagentur: {} results", postings.len());
    println!("first: {:?}", first.title);
}
