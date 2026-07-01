use super::*;

#[test]
fn test_smartrecruiters_scraper_id() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.id(), "smartrecruiters");
}

#[test]
fn test_smartrecruiters_scraper_display_name() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.display_name(), "SmartRecruiters");
}

#[test]
fn test_smartrecruiters_scraper_mode() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_location_struct_fields() {
    let location = Location {
        city: Some("Berlin".to_string()),
        country: Some("Germany".to_string()),
        remote: Some(true),
    };
    assert_eq!(location.city, Some("Berlin".to_string()));
    assert_eq!(location.remote, Some(true));
}

#[test]
fn test_location_struct_defaults() {
    let location = Location {
        city: None,
        country: None,
        remote: None,
    };
    assert!(location.city.is_none());
    assert!(location.remote.is_none());
}

#[test]
fn test_posting_struct_fields() {
    let posting = Posting {
        id: "123".to_string(),
        uuid: Some("abc".to_string()),
        name: "Software Engineer".to_string(),
        location: None,
        released_date: None,
        ref_field: None,
    };
    assert_eq!(posting.id, "123");
    assert_eq!(posting.name, "Software Engineer");
}

#[test]
fn test_smartrecruiters_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_smartrecruiters_requires_company() {
    assert!(
        SmartRecruitersScraper.requires_company(),
        "SmartRecruiters is an ATS board and must return true for requires_company()"
    );
}

// ---------------------------------------------------------------------------
// normalize_companies — unit tests (network-free)
// ---------------------------------------------------------------------------

#[test]
fn normalize_drops_blank_entries() {
    let input = vec![
        "Visa".to_string(),
        "".to_string(),
        "   ".to_string(),
        "\t".to_string(),
        "IKEA".to_string(),
    ];
    let result = normalize_companies(&input, 20);
    assert_eq!(result, vec!["Visa", "IKEA"]);
}

#[test]
fn normalize_trims_whitespace() {
    let input = vec!["  Visa  ".to_string(), "\tIKEA\n".to_string()];
    let result = normalize_companies(&input, 20);
    assert_eq!(result, vec!["Visa", "IKEA"]);
}

#[test]
fn normalize_dedupes_first_seen_order() {
    let input = vec![
        "Alpha".to_string(),
        "Beta".to_string(),
        "Alpha".to_string(), // duplicate — must be dropped
        "Gamma".to_string(),
        "Beta".to_string(), // duplicate — must be dropped
    ];
    let result = normalize_companies(&input, 20);
    assert_eq!(result, vec!["Alpha", "Beta", "Gamma"]);
}

#[test]
fn normalize_dedupes_after_trim() {
    // "  Alpha  " and "Alpha" are the same after trimming.
    let input = vec!["  Alpha  ".to_string(), "Alpha".to_string()];
    let result = normalize_companies(&input, 20);
    assert_eq!(result, vec!["Alpha"]);
}

#[test]
fn normalize_caps_at_max_20() {
    // SmartRecruiters cap is 20 — 30 entries must be truncated to 20.
    let input: Vec<String> = (0..30).map(|i| format!("company-{i}")).collect();
    let result = normalize_companies(&input, 20);
    assert_eq!(result.len(), 20);
    assert_eq!(result[0], "company-0");
    assert_eq!(result[19], "company-19");
    // Entry 20+ must be absent.
    assert!(!result.contains(&"company-20".to_string()));
}

#[test]
fn normalize_cap_exact_boundary() {
    // Exactly MAX_COMPANIES (20) entries — none should be dropped.
    let input: Vec<String> = (0..20).map(|i| format!("co-{i}")).collect();
    let result = normalize_companies(&input, 20);
    assert_eq!(result.len(), 20);
}

#[test]
fn normalize_empty_input_returns_empty() {
    let result = normalize_companies(&[], 20);
    assert!(result.is_empty());
}

#[test]
fn normalize_all_blanks_returns_empty() {
    let input = vec!["".to_string(), "   ".to_string(), "\n".to_string()];
    let result = normalize_companies(&input, 20);
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = SmartRecruitersScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 5,
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
    };
    let result = scraper.search(input, ctx).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

/// When all company entries are blank or whitespace the list normalises to
/// empty — the scraper produces Ok([]) without attempting any network I/O.
///
/// ponytail: SmartRecruiters does not have an all-fail Err path (it uses
/// continue on list-fetch errors, not first_fetch_error tracking). The
/// normalisation guarantee is covered by the normalize_companies unit tests
/// above.
#[tokio::test]
async fn all_blank_companies_returns_ok_empty() {
    let scraper = SmartRecruitersScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 5,
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
        companies: vec!["".to_string(), "   ".to_string()],
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    };
    let result = scraper.search(input, ctx).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = SmartRecruitersScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 5,
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
        companies: vec!["Visa".to_string()], // confirmed live: 7 postings via SmartRecruiters API
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
    println!("smartrecruiters: {} results", postings.len());
    println!("first: {:?}", first.title);
}
