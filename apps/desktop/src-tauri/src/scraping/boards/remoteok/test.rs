use super::*;

/// RemoteOK sends `id` as a number for some rows and a string for others.
/// `Value::to_string()` JSON-serialises, so the string variant used to come back
/// WITH quotes and corrupt the posting id / external_id (the stable cache key).
#[test]
fn normalize_id_reads_the_value_per_variant() {
    assert_eq!(normalize_id(Some(serde_json::json!(1234567))), "1234567");
    assert_eq!(normalize_id(Some(serde_json::json!("1234567"))), "1234567");
    assert_eq!(normalize_id(None), "");
    // Anything that is neither a string nor an integer stays empty, so the
    // caller's `id_str.is_empty()` guard drops the row as before.
    assert_eq!(normalize_id(Some(serde_json::json!(null))), "");
    assert_eq!(normalize_id(Some(serde_json::json!({}))), "");
}

#[test]
fn test_remoteok_scraper_id() {
    let scraper = RemoteOkScraper;
    assert_eq!(scraper.id(), "remoteok");
}

#[test]
fn test_remoteok_scraper_display_name() {
    let scraper = RemoteOkScraper;
    assert_eq!(scraper.display_name(), "RemoteOK");
}

#[test]
fn test_remoteok_scraper_mode() {
    let scraper = RemoteOkScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_remoteok_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_remote_ok_item_job_variant() {
    let item = RemoteOkItem::Job {
        id: Some(serde_json::json!(123)),
        slug: Some("test-job".to_string()),
        position: Some("Software Engineer".to_string()),
        company: Some("Test Corp".to_string()),
        location: Some("Remote".to_string()),
        tags: Some(vec!["rust".to_string()]),
        description: None,
        url: None,
        apply_url: None,
        date: None,
    };
    match item {
        RemoteOkItem::Job { position, .. } => {
            assert_eq!(position, Some("Software Engineer".to_string()));
        }
        _ => panic!("Expected Job variant"),
    }
}

#[test]
fn test_remote_ok_item_legend_variant() {
    let item = RemoteOkItem::Legend {
        _slug: "legend".to_string(),
    };
    match item {
        RemoteOkItem::Legend { .. } => {
            // Successfully matched legend
        }
        _ => panic!("Expected Legend variant"),
    }
}

#[test]
fn test_remote_ok_item_job_defaults() {
    let item = RemoteOkItem::Job {
        id: None,
        slug: None,
        position: None,
        company: None,
        location: None,
        tags: None,
        description: None,
        url: None,
        apply_url: None,
        date: None,
    };
    match item {
        RemoteOkItem::Job {
            position, company, ..
        } => {
            assert!(position.is_none());
            assert!(company.is_none());
        }
        _ => panic!("Expected Job variant"),
    }
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = RemoteOkScraper;
    let input = BoardSearchInput {
        query: "engineer".to_string(), // broader query; "rust" had 0 matches transiently
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
    println!("remoteok: {} results", postings.len());
    println!("first: {:?}", first.title);
}
