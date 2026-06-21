use super::*;

#[test]
fn test_job_posting_creation() {
    let posting = JobPosting {
        id: "test:123".to_string(),
        external_id: Some("123".to_string()),
        title: "Software Engineer".to_string(),
        company: "Test Corp".to_string(),
        location: Some("Berlin".to_string()),
        url: "https://example.com/job/123".to_string(),
        source: "test".to_string(),
        description: Some("Test description".to_string()),
        requirements: Some(vec!["Rust".to_string(), "TypeScript".to_string()]),
        posted_at: Some(1234567890),
        captured_at: 9876543210,
        extra: std::collections::HashMap::new(),
    };
    assert_eq!(posting.id, "test:123");
    assert_eq!(posting.title, "Software Engineer");
}

#[test]
fn test_job_posting_defaults() {
    let posting = JobPosting {
        id: "test:123".to_string(),
        external_id: None,
        title: "Software Engineer".to_string(),
        company: "Test Corp".to_string(),
        location: None,
        url: "https://example.com/job/123".to_string(),
        source: "test".to_string(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 9876543210,
        extra: std::collections::HashMap::new(),
    };
    assert!(posting.external_id.is_none());
    assert!(posting.location.is_none());
}

#[test]
fn test_job_posting_clone() {
    let posting = JobPosting {
        id: "test:123".to_string(),
        external_id: Some("123".to_string()),
        title: "Software Engineer".to_string(),
        company: "Test Corp".to_string(),
        location: Some("Berlin".to_string()),
        url: "https://example.com/job/123".to_string(),
        source: "test".to_string(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 9876543210,
        extra: std::collections::HashMap::new(),
    };
    let cloned = posting.clone();
    assert_eq!(posting.id, cloned.id);
    assert_eq!(posting.title, cloned.title);
}

#[test]
fn test_board_search_input_creation() {
    let input = BoardSearchInput {
        query: "Software Engineer".to_string(),
        location: Some("Berlin".to_string()),
        amount: 25,
        pages: 5,
        date_filter: Some("7d".to_string()),
        job_type: Some("F".to_string()),
        work_type: Some("2".to_string()),
        experience_level: Some("2".to_string()),
        easy_apply: Some(true),
        actively_hiring: Some(true),
        verified: Some(true),
        sort_by: Some("DD".to_string()),
        locale: Some("de".to_string()),
        country_code: Some("DE".to_string()),
        latitude: Some(52.52),
        longitude: Some(13.405),
        radius_km: Some(25),
        companies: vec!["acme".to_string(), "globex".to_string()],
    };
    assert_eq!(input.query, "Software Engineer");
    assert_eq!(input.pages, 5);
    assert_eq!(input.amount, 25);
    assert_eq!(input.companies, vec!["acme", "globex"]);
}

#[test]
fn test_board_search_input_defaults() {
    let input = BoardSearchInput {
        query: "Software Engineer".to_string(),
        location: None,
        amount: 25,
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
        // Plumbing for ATS company slugs: an unset filter is the empty list, the
        // no-op default every current board sees.
        companies: Vec::new(),
    };
    assert!(input.location.is_none());
    assert_eq!(input.pages, 1);
    assert!(input.companies.is_empty());
}

#[test]
fn test_scraper_mode_http() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_scraper_mode_browser() {
    let mode = ScraperMode::Browser;
    assert_eq!(mode, ScraperMode::Browser);
    assert_ne!(mode, ScraperMode::Http);
}

#[test]
fn test_scraper_mode_copy() {
    let mode = ScraperMode::Http;
    let copied = mode;
    assert_eq!(mode, copied);
}

#[test]
fn test_scrape_context_creation() {
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    };
    assert!(!ctx.signal.is_cancelled());
}

#[test]
fn test_scrape_context_with_callbacks() {
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: Some(Box::new(|_| {})),
        on_item: Some(Box::new(|_| {})),
    };
    assert!(ctx.on_progress.is_some());
    assert!(ctx.on_item.is_some());
}
