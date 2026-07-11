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
        on_truncation: None,
        on_note: None,
    }
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_bamboohr_scraper_id() {
    let scraper = BambooHrScraper;
    assert_eq!(scraper.id(), "bamboohr");
}

#[test]
fn test_bamboohr_scraper_display_name() {
    let scraper = BambooHrScraper;
    assert_eq!(scraper.display_name(), "BambooHR");
}

#[test]
fn test_bamboohr_scraper_mode() {
    let scraper = BambooHrScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_bamboohr_requires_company() {
    assert!(
        BambooHrScraper.requires_company(),
        "BambooHR is an ATS board and must return true for requires_company()"
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
// Slug guard — is_valid_dns_label_slug (SSRF: subdomain DNS-label guard)
// ---------------------------------------------------------------------------

#[test]
fn slug_validation_accepts_valid_slugs() {
    assert!(is_valid_dns_label_slug("acme"));
    assert!(is_valid_dns_label_slug("my-company"));
    assert!(is_valid_dns_label_slug("acme123"));
    assert!(is_valid_dns_label_slug("a1b2-c3d4"));
    assert!(
        is_valid_dns_label_slug(&"a".repeat(63)),
        "exactly 63 chars must be accepted"
    );
}

#[test]
fn slug_validation_rejects_invalid_slugs() {
    assert!(
        !is_valid_dns_label_slug("acme.corp"),
        "dot must alter URL authority — rejected"
    );
    assert!(
        !is_valid_dns_label_slug("acme/corp"),
        "slash must be rejected"
    );
    assert!(!is_valid_dns_label_slug("acme@corp"), "@ must be rejected");
    assert!(
        !is_valid_dns_label_slug("acme_corp"),
        "underscore must be rejected"
    );
    assert!(
        !is_valid_dns_label_slug("-acme"),
        "leading hyphen is not a valid DNS label"
    );
    assert!(
        !is_valid_dns_label_slug("acme-"),
        "trailing hyphen is not a valid DNS label"
    );
    assert!(!is_valid_dns_label_slug(""), "empty slug must be rejected");
    assert!(
        !is_valid_dns_label_slug(&"a".repeat(64)),
        "exceeds 63-char DNS label limit"
    );
}

// ---------------------------------------------------------------------------
// bamboohr_id_to_string — number and string id normalisation
// ---------------------------------------------------------------------------

#[test]
fn bamboohr_id_to_string_accepts_number_and_string() {
    let number: serde_json::Value = serde_json::from_str("1").unwrap();
    let string: serde_json::Value = serde_json::from_str("\"1\"").unwrap();
    assert_eq!(bamboohr_id_to_string(&number), Some("1".to_string()));
    assert_eq!(bamboohr_id_to_string(&string), Some("1".to_string()));
}

#[test]
fn bamboohr_id_to_string_rejects_blank_and_other_types() {
    let blank: serde_json::Value = serde_json::from_str("\"  \"").unwrap();
    let boolean: serde_json::Value = serde_json::from_str("true").unwrap();
    let null: serde_json::Value = serde_json::from_str("null").unwrap();
    assert_eq!(
        bamboohr_id_to_string(&blank),
        None,
        "blank string must be rejected"
    );
    assert_eq!(
        bamboohr_id_to_string(&boolean),
        None,
        "non-string/number types must be rejected"
    );
    assert_eq!(bamboohr_id_to_string(&null), None, "null must be rejected");
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = BambooHrScraper;
    let result = scraper.search(make_input(Vec::new()), make_ctx()).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

#[tokio::test]
async fn invalid_slug_skipped_without_network() {
    let scraper = BambooHrScraper;
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
    let scraper = BambooHrScraper;
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
// parse_bamboohr_response — fixture-based parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_bamboohr_response_happy_path() {
    let json = r#"{
        "result": [
            {
                "id": 1,
                "jobOpeningName": "DevOps Engineer",
                "location": { "city": "Austin", "state": "TX" },
                "isRemote": false
            }
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).expect("fixture must parse");
    let postings = parse_bamboohr_response(resp, "acme", 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    let p = &postings[0];
    assert_eq!(p.title, "DevOps Engineer");
    assert_eq!(p.company, "acme");
    assert_eq!(p.location, Some("Austin, TX".to_string()));
    assert_eq!(p.url, "https://acme.bamboohr.com/careers/1");
    assert_eq!(p.id, "bamboohr:acme:1");
    assert_eq!(p.external_id, Some("1".to_string()));
    assert_eq!(p.source, "bamboohr");
    assert_eq!(p.captured_at, 1_700_000_000_000);
}

#[test]
fn parse_bamboohr_response_empty_result_returns_empty_vec() {
    let resp: BhResponse = serde_json::from_str(r#"{"result": []}"#).unwrap();
    assert!(
        parse_bamboohr_response(resp, "acme", 0).is_empty(),
        "empty result array must parse to an empty Vec, not an error"
    );
}

/// Missing/blank id and missing/empty title each drop the row; valid rows in
/// the same payload must still come through.
#[test]
fn parse_bamboohr_response_drops_malformed_rows() {
    let json = r#"{
        "result": [
            {"id": 1, "jobOpeningName": "Valid One", "location": null, "isRemote": null},
            {"id": null, "jobOpeningName": "Missing ID", "location": null, "isRemote": null},
            {"id": "", "jobOpeningName": "Blank ID", "location": null, "isRemote": null},
            {"id": 2, "jobOpeningName": null, "location": null, "isRemote": null},
            {"id": 3, "jobOpeningName": "", "location": null, "isRemote": null},
            {"id": 4, "jobOpeningName": "Valid Two", "location": null, "isRemote": null}
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).unwrap();
    let postings = parse_bamboohr_response(resp, "acme", 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
}

/// `isRemote: true` appends "Remote" to the joined location string.
#[test]
fn parse_bamboohr_response_is_remote_true_appends_remote_to_location() {
    let json = r#"{
        "result": [
            {
                "id": 1,
                "jobOpeningName": "Support Engineer",
                "location": { "city": "Austin", "state": "TX" },
                "isRemote": true
            }
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).unwrap();
    let postings = parse_bamboohr_response(resp, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].location, Some("Austin, TX, Remote".to_string()));
}

/// `isRemote: null` must not add "Remote" — the happy-path test above already
/// covers the explicit `false` case.
#[test]
fn parse_bamboohr_response_is_remote_null_omits_remote() {
    let json = r#"{
        "result": [
            {
                "id": 1,
                "jobOpeningName": "Support Engineer",
                "location": { "city": "Austin", "state": "TX" },
                "isRemote": null
            }
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).unwrap();
    let postings = parse_bamboohr_response(resp, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(
        postings[0].location,
        Some("Austin, TX".to_string()),
        "null isRemote must not add Remote"
    );
}

/// Data-shape unknown: `id` observed as both a JSON number and a JSON string
/// across tenants — both must be accepted and normalise to the same id.
#[test]
fn parse_bamboohr_response_id_number_and_string_forms_both_accepted() {
    let json_number = r#"{"result":[{"id":1,"jobOpeningName":"DevOps Engineer","location":null,"isRemote":null}]}"#;
    let json_string = r#"{"result":[{"id":"1","jobOpeningName":"DevOps Engineer","location":null,"isRemote":null}]}"#;

    let resp_number: BhResponse = serde_json::from_str(json_number).unwrap();
    let resp_string: BhResponse = serde_json::from_str(json_string).unwrap();

    let out_number = parse_bamboohr_response(resp_number, "acme", 0);
    let out_string = parse_bamboohr_response(resp_string, "acme", 0);

    assert_eq!(out_number.len(), 1);
    assert_eq!(out_string.len(), 1);
    assert_eq!(out_number[0].id, "bamboohr:acme:1");
    assert_eq!(out_string[0].id, "bamboohr:acme:1");
    assert_eq!(out_number[0].external_id, Some("1".to_string()));
    assert_eq!(out_string[0].external_id, Some("1".to_string()));
}

/// Regression (HIGH fix): the same raw job id from two different tenants must
/// produce two distinct `JobPosting.id` values (`bamboohr:acme:1` vs
/// `bamboohr:globex:1`) — neither must overwrite the other in a dedup layer.
#[test]
fn parse_bamboohr_response_cross_tenant_ids_do_not_collide() {
    let json =
        r#"{"result":[{"id":1,"jobOpeningName":"Engineer","location":null,"isRemote":null}]}"#;

    let resp_acme: BhResponse = serde_json::from_str(json).unwrap();
    let resp_globex: BhResponse = serde_json::from_str(json).unwrap();

    let out_acme = parse_bamboohr_response(resp_acme, "acme", 0);
    let out_globex = parse_bamboohr_response(resp_globex, "globex", 0);

    assert_eq!(out_acme.len(), 1);
    assert_eq!(out_globex.len(), 1);
    assert_ne!(
        out_acme[0].id, out_globex[0].id,
        "same raw job id=1 from different tenants must produce distinct JobPosting.id"
    );
    assert_eq!(out_acme[0].id, "bamboohr:acme:1");
    assert_eq!(out_globex[0].id, "bamboohr:globex:1");
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = BambooHrScraper;
    let input = make_input(vec!["bamboohr".to_string()]);
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
    println!("bamboohr: {} results", postings.len());
    println!("first: {:?}", first.title);
}
