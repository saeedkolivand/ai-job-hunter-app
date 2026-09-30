use super::super::*;
use super::metadata_and_search::{make_ctx, make_input};

// ---------------------------------------------------------------------------
// parse_pinpoint_response — fixture-based parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_pinpoint_response_happy_path() {
    let json = r#"{
        "data": [
            {
                "title": "Senior Backend Engineer",
                "url": "https://acme.pinpointhq.com/postings/senior-backend-engineer",
                "location": { "name": "Remote (US)", "city": null, "province": null }
            }
        ]
    }"#;
    let resp: PpResponse = serde_json::from_str(json).expect("fixture must parse");
    let postings = parse_pinpoint_response(resp, "acme", 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    let p = &postings[0];
    assert_eq!(p.title, "Senior Backend Engineer");
    assert_eq!(
        p.url,
        "https://acme.pinpointhq.com/postings/senior-backend-engineer"
    );
    assert_eq!(p.company, "acme");
    assert_eq!(p.location, Some("Remote (US)".to_string()));
    assert_eq!(p.id, format!("pinpoint:{}", p.url));
    assert_eq!(p.external_id, Some(p.url.clone()));
    assert_eq!(p.source, "pinpoint");
    assert_eq!(p.captured_at, 1_700_000_000_000);
}

#[test]
fn parse_pinpoint_response_location_falls_back_to_city_province() {
    let json = r#"{
        "data": [
            {
                "title": "Support Engineer",
                "url": "https://acme.pinpointhq.com/postings/support-engineer",
                "location": { "name": null, "city": "Berlin", "province": "BE" }
            }
        ]
    }"#;
    let resp: PpResponse = serde_json::from_str(json).unwrap();
    let postings = parse_pinpoint_response(resp, "acme", 0);
    assert_eq!(postings[0].location, Some("Berlin, BE".to_string()));
}

#[test]
fn parse_pinpoint_response_empty_data_returns_empty_vec() {
    let resp: PpResponse = serde_json::from_str(r#"{"data": []}"#).unwrap();
    assert!(
        parse_pinpoint_response(resp, "acme", 0).is_empty(),
        "empty data array must parse to an empty Vec, not an error"
    );
}

/// Missing/empty title and missing/malformed url each drop the row; valid
/// rows in the same payload must still come through.
#[test]
fn parse_pinpoint_response_drops_malformed_rows() {
    let json = r#"{
        "data": [
            {"title": "Valid One", "url": "https://acme.pinpointhq.com/postings/valid-one", "location": null},
            {"title": null, "url": "https://acme.pinpointhq.com/postings/missing-title", "location": null},
            {"title": "", "url": "https://acme.pinpointhq.com/postings/empty-title", "location": null},
            {"title": "Missing URL", "url": null, "location": null},
            {"title": "Malformed URL", "url": "not-a-url", "location": null},
            {"title": "Valid Two", "url": "https://acme.pinpointhq.com/postings/valid-two", "location": null}
        ]
    }"#;
    let resp: PpResponse = serde_json::from_str(json).unwrap();
    let postings = parse_pinpoint_response(resp, "acme", 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
}

/// Pinpoint has no stable job id — the (deduped) posting URL doubles as the
/// id/dedup key: two rows sharing a url dedupe to one, distinct urls are kept.
#[test]
fn parse_pinpoint_response_dedupes_by_url_distinct_urls_kept() {
    let json = r#"{
        "data": [
            {"title": "First Listing", "url": "https://acme.pinpointhq.com/postings/dup", "location": null},
            {"title": "Duplicate Listing", "url": "https://acme.pinpointhq.com/postings/dup", "location": null},
            {"title": "Distinct Listing", "url": "https://acme.pinpointhq.com/postings/other", "location": null}
        ]
    }"#;
    let resp: PpResponse = serde_json::from_str(json).unwrap();
    let postings = parse_pinpoint_response(resp, "acme", 0);
    assert_eq!(
        postings.len(),
        2,
        "duplicate url must be deduped, distinct url kept"
    );
    assert_eq!(
        postings[0].title, "First Listing",
        "first-seen row wins the dedupe"
    );
    assert_eq!(
        postings[1].url,
        "https://acme.pinpointhq.com/postings/other"
    );
}

/// Regression: a `https://user:pass@evil.example/job` url must be dropped —
/// the userinfo-rejecting URL sanity check applies inside the parser too, not
/// just at the network layer.
#[test]
fn parse_pinpoint_response_rejects_userinfo_url() {
    let json = r#"{
        "data": [
            {"title": "Phishy Listing", "url": "https://user:pass@evil.example/job", "location": null},
            {"title": "Legit Listing", "url": "https://acme.pinpointhq.com/postings/legit", "location": null}
        ]
    }"#;
    let resp: PpResponse = serde_json::from_str(json).unwrap();
    let postings = parse_pinpoint_response(resp, "acme", 0);
    assert_eq!(
        postings.len(),
        1,
        "userinfo url must be dropped, legit row kept"
    );
    assert_eq!(postings[0].title, "Legit Listing");
}

/// `workplace_type` maps to `extra.workType`; `workplace_type_text` must never
/// be read (it is i18n'd), and an absent field writes nothing.
#[test]
fn parse_pinpoint_response_workplace_type_maps_to_extra_work_type() {
    let json = r#"{
        "data": [
            {"title": "Remote Role", "url": "https://acme.pinpointhq.com/postings/1", "location": null, "workplace_type": "remote"},
            {"title": "Hybrid Role", "url": "https://acme.pinpointhq.com/postings/2", "location": null, "workplace_type": "hybrid"},
            {"title": "Undeclared Role", "url": "https://acme.pinpointhq.com/postings/3", "location": null}
        ]
    }"#;
    let resp: PpResponse = serde_json::from_str(json).unwrap();
    let postings = parse_pinpoint_response(resp, "acme", 0);

    let work_type = |title: &str| -> Option<String> {
        postings
            .iter()
            .find(|p| p.title == title)
            .and_then(|p| p.extra.get("workType"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    assert_eq!(work_type("Remote Role"), Some("remote".to_string()));
    assert_eq!(work_type("Hybrid Role"), Some("hybrid".to_string()));
    assert_eq!(
        work_type("Undeclared Role"),
        None,
        "an absent workplace_type must write nothing, not a guessed value"
    );
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = PinpointScraper;
    let input = make_input(vec!["pinpoint".to_string()]);
    let ctx = make_ctx();
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
    println!("pinpoint: {} results", postings.len());
    println!("first: {:?}", first.title);
}
