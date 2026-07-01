use super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_input(companies: Vec<String>) -> BoardSearchInput {
    BoardSearchInput {
        query: String::new(),
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
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies,
    }
}

fn make_ctx() -> ScrapeContext {
    ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    }
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_pinpoint_scraper_id() {
    let scraper = PinpointScraper;
    assert_eq!(scraper.id(), "pinpoint");
}

#[test]
fn test_pinpoint_scraper_display_name() {
    let scraper = PinpointScraper;
    assert_eq!(scraper.display_name(), "Pinpoint");
}

#[test]
fn test_pinpoint_scraper_mode() {
    let scraper = PinpointScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_pinpoint_requires_company() {
    assert!(
        PinpointScraper.requires_company(),
        "Pinpoint is an ATS board and must return true for requires_company()"
    );
}

// ---------------------------------------------------------------------------
// normalize_companies — unit tests (network-free)
// ---------------------------------------------------------------------------

#[test]
fn normalize_drops_blank_entries() {
    let input = vec![
        "acme".to_string(),
        "".to_string(),
        "   ".to_string(),
        "\t".to_string(),
        "beta".to_string(),
    ];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["acme", "beta"]);
}

#[test]
fn normalize_trims_whitespace() {
    let input = vec!["  acme  ".to_string(), "\tbeta\n".to_string()];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["acme", "beta"]);
}

#[test]
fn normalize_dedupes_first_seen_order() {
    let input = vec![
        "alpha".to_string(),
        "beta".to_string(),
        "alpha".to_string(), // duplicate — must be dropped
        "gamma".to_string(),
        "beta".to_string(), // duplicate — must be dropped
    ];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn normalize_dedupes_after_trim() {
    let input = vec!["  alpha  ".to_string(), "alpha".to_string()];
    let result = normalize_companies(&input, 50);
    assert_eq!(result, vec!["alpha"]);
}

#[test]
fn normalize_caps_at_max() {
    let input: Vec<String> = (0..60).map(|i| format!("company-{i}")).collect();
    let result = normalize_companies(&input, 50);
    assert_eq!(result.len(), 50);
    assert_eq!(result[0], "company-0");
    assert_eq!(result[49], "company-49");
}

#[test]
fn normalize_cap_exact_boundary() {
    let input: Vec<String> = (0..50).map(|i| format!("co-{i}")).collect();
    let result = normalize_companies(&input, 50);
    assert_eq!(result.len(), 50);
}

#[test]
fn normalize_empty_input_returns_empty() {
    let result = normalize_companies(&[], 50);
    assert!(result.is_empty());
}

#[test]
fn normalize_all_blanks_returns_empty() {
    let input = vec!["".to_string(), "   ".to_string(), "\n".to_string()];
    let result = normalize_companies(&input, 50);
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// Slug guard — is_valid_pinpoint_slug (SSRF: subdomain DNS-label guard)
// ---------------------------------------------------------------------------

#[test]
fn slug_validation_accepts_valid_slugs() {
    assert!(is_valid_pinpoint_slug("acme"));
    assert!(is_valid_pinpoint_slug("my-company"));
    assert!(is_valid_pinpoint_slug("acme123"));
    assert!(is_valid_pinpoint_slug("a1b2-c3d4"));
    assert!(
        is_valid_pinpoint_slug(&"a".repeat(63)),
        "exactly 63 chars must be accepted"
    );
}

#[test]
fn slug_validation_rejects_invalid_slugs() {
    assert!(
        !is_valid_pinpoint_slug("acme.corp"),
        "dot must alter URL authority — rejected"
    );
    assert!(
        !is_valid_pinpoint_slug("acme/corp"),
        "slash must be rejected"
    );
    assert!(!is_valid_pinpoint_slug("acme@corp"), "@ must be rejected");
    assert!(
        !is_valid_pinpoint_slug("acme_corp"),
        "underscore must be rejected"
    );
    assert!(
        !is_valid_pinpoint_slug("-acme"),
        "leading hyphen is not a valid DNS label"
    );
    assert!(
        !is_valid_pinpoint_slug("acme-"),
        "trailing hyphen is not a valid DNS label"
    );
    assert!(!is_valid_pinpoint_slug(""), "empty slug must be rejected");
    assert!(
        !is_valid_pinpoint_slug(&"a".repeat(64)),
        "exceeds 63-char DNS label limit"
    );
}

// ---------------------------------------------------------------------------
// URL guard — is_https_url (userinfo / scheme sanity check)
// ---------------------------------------------------------------------------

#[test]
fn url_guard_accepts_plain_https_rejects_others() {
    assert!(is_https_url("https://acme.pinpointhq.com/postings/foo"));
    assert!(
        !is_https_url("http://acme.pinpointhq.com/postings/foo"),
        "non-https must be rejected"
    );
    assert!(
        !is_https_url("not-a-url"),
        "unparseable url must be rejected"
    );
    assert!(
        !is_https_url("https://user:pass@evil.example/job"),
        "embedded userinfo must be rejected (phishing vector)"
    );
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = PinpointScraper;
    let result = scraper.search(make_input(Vec::new()), make_ctx()).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

#[tokio::test]
async fn invalid_slug_skipped_without_network() {
    let scraper = PinpointScraper;
    let result = scraper
        .search(make_input(vec!["dotted.host".to_string()]), make_ctx())
        .await;
    assert!(
        result.is_ok(),
        "search must return Ok even for invalid slug"
    );
    assert!(
        result.unwrap().is_empty(),
        "invalid slug must produce empty result (skipped, no network)"
    );
}

/// A pre-cancelled signal must make the loop break immediately without
/// recording `first_fetch_error`.
#[tokio::test]
async fn cancelled_before_fetch_returns_ok_not_err() {
    let scraper = PinpointScraper;
    let ctx = make_ctx();
    ctx.signal.cancel();
    let result = scraper
        .search(make_input(vec!["acme".to_string()]), ctx)
        .await;
    assert!(
        result.is_ok(),
        "cancelled run must return Ok, not Err — cancellation must not be recorded as first_fetch_error"
    );
}

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
