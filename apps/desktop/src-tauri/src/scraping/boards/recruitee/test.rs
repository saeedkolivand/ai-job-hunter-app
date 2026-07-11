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
// Hostname-slug validation guard
// ---------------------------------------------------------------------------

/// Slugs with characters outside [a-zA-Z0-9-] must be rejected without any
/// network request (the guard rejects them before the first fetch). A run where
/// EVERY slug is rejected now surfaces a distinct board error instead of a
/// silent zero (claude review #597).
#[tokio::test]
async fn all_invalid_slugs_error_without_network() {
    let scraper = RecruiteeScraper;

    // `@`, space, dot, and percent-encoding are each invalid hostname-label
    // characters — every one is rejected pre-fetch, so each run is a board error.
    for slug in ["bad@host", "has space", "dotted.host", "bad%20slug"] {
        let err = scraper
            .search(make_input(vec![slug.to_string()]), make_ctx())
            .await
            .expect_err("an all-invalid-slug run must be a board error, not a silent zero");
        assert!(
            err.to_string().contains("slug(s) invalid"),
            "error for '{slug}' must name the invalid-slug reason, got: {err}"
        );
    }
}

/// A mixed list with one invalid slug followed by one valid-shaped slug must
/// not panic.  The invalid entry is skipped; the valid one would normally
/// attempt a network fetch — cancel the token immediately so no real request
/// completes, then assert no panic and that the invalid slug didn't sneak
/// through as output.
#[tokio::test]
async fn mixed_list_invalid_slug_skipped_no_panic() {
    let scraper = RecruiteeScraper;
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    // Cancel immediately so the valid slug's fetch attempt is aborted before
    // any I/O, keeping the test network-free.
    ctx.signal.cancel();

    let result = scraper
        .search(
            make_input(vec!["bad@host".to_string(), "valid-slug".to_string()]),
            ctx,
        )
        .await;

    assert!(result.is_ok(), "mixed list must return Ok, not Err");
    // With the token cancelled no offers can be collected; the invalid slug
    // was skipped before cancellation was even checked.
    let postings = result.unwrap();
    assert!(
        postings.iter().all(|p| p.company != "bad@host"),
        "invalid slug must not appear in output"
    );
}

// ---------------------------------------------------------------------------
// Boundary: valid slugs are NOT rejected by the guard
// ---------------------------------------------------------------------------

/// Slugs that use only [a-zA-Z0-9-] must pass the production guard; slugs with
/// any other character must be rejected.  Uses `is_valid_recruitee_slug` from
/// `mod.rs` directly so this test exercises the real guard — not a local
/// re-implementation that could drift from the production path.
#[test]
fn valid_slug_passes_guard_predicate() {
    let valid_slugs = ["acme", "my-company", "ACME123", "a1b2-c3d4"];
    for slug in valid_slugs {
        assert!(
            is_valid_recruitee_slug(slug),
            "slug '{slug}' should be accepted by the production hostname guard"
        );
    }

    let invalid_slugs = [
        "bad@host",
        "has space",
        "dotted.host",
        "bad%20slug",
        "_under",
        // DNS-label constraints (must match Personio guard)
        "-leading",
        "trailing-",
        &"a".repeat(64), // >63 chars
    ];
    for slug in invalid_slugs {
        assert!(
            !is_valid_recruitee_slug(slug),
            "slug '{slug}' should be rejected by the production hostname guard"
        );
    }
}

// ---------------------------------------------------------------------------
// Existing scraper-metadata tests
// ---------------------------------------------------------------------------

#[test]
fn test_recruitee_scraper_id() {
    let scraper = RecruiteeScraper;
    assert_eq!(scraper.id(), "recruitee");
}

#[test]
fn test_recruitee_scraper_display_name() {
    let scraper = RecruiteeScraper;
    assert_eq!(scraper.display_name(), "Recruitee");
}

#[test]
fn test_recruitee_scraper_mode() {
    let scraper = RecruiteeScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_recruitee_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_recruitee_requires_company() {
    assert!(
        RecruiteeScraper.requires_company(),
        "Recruitee is an ATS board and must return true for requires_company()"
    );
}

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = RecruiteeScraper;
    let result = scraper.search(make_input(Vec::new()), make_ctx()).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

#[test]
fn test_offer_struct_fields() {
    let offer = Offer {
        id: 123,
        slug: "test-job".to_string(),
        title: "Software Engineer".to_string(),
        description: Some("Test description".to_string()),
        requirements: Some("Test requirements".to_string()),
        careers_url: "https://example.com".to_string(),
        city: Some("Berlin".to_string()),
        country: Some("Germany".to_string()),
        remote: Some(true),
        created_at: None,
        company_name: Some("Test Corp".to_string()),
    };
    assert_eq!(offer.id, 123);
    assert_eq!(offer.title, "Software Engineer");
    assert_eq!(offer.remote, Some(true));
}

#[test]
fn test_offer_struct_defaults() {
    let offer = Offer {
        id: 123,
        slug: "test-job".to_string(),
        title: "Software Engineer".to_string(),
        description: None,
        requirements: None,
        careers_url: "https://example.com".to_string(),
        city: None,
        country: None,
        remote: None,
        created_at: None,
        company_name: None,
    };
    assert!(offer.description.is_none());
    assert!(offer.remote.is_none());
}

#[test]
fn test_resp_struct() {
    let resp = Resp { offers: vec![] };
    assert!(resp.offers.is_empty());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = RecruiteeScraper;
    let input = BoardSearchInput {
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
        companies: vec!["personio".to_string()], // confirmed live: personio.recruitee.com has offers
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
    println!("recruitee: {} results", postings.len());
    println!("first: {:?}", first.title);
}
