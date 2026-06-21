use super::*;

// ── Slug validation guard ─────────────────────────────────────────────────────

/// `is_valid_personio_slug` must accept normal lowercase slugs and reject
/// values that could alter the URL authority (SSRF guard).
#[test]
fn slug_validation_accepts_valid_slugs() {
    assert!(is_valid_personio_slug("clark"));
    assert!(is_valid_personio_slug("my-company"));
    assert!(is_valid_personio_slug("acme123"));
    assert!(is_valid_personio_slug("a1b2-c3d4"));
}

#[test]
fn slug_validation_rejects_ssrf_slugs() {
    // IP with port — the classic SSRF vector for subdomain-based URLs.
    assert!(!is_valid_personio_slug("127.0.0.1:8443"));
    // Path injection.
    assert!(!is_valid_personio_slug("127.0.0.1/foo"));
    // Dot in label (would split subdomain or allow IP).
    assert!(!is_valid_personio_slug("dotted.host"));
    // Colon (port injection).
    assert!(!is_valid_personio_slug("host:8080"));
    // Leading hyphen (invalid DNS label).
    assert!(!is_valid_personio_slug("-leading"));
    // Trailing hyphen (invalid DNS label).
    assert!(!is_valid_personio_slug("trailing-"));
    // Empty string.
    assert!(!is_valid_personio_slug(""));
    // Exceeds 63-char DNS label limit.
    assert!(!is_valid_personio_slug(&"a".repeat(64)));
}

/// An invalid slug must be skipped without any network request — the search
/// returns Ok([]) immediately.
#[tokio::test]
async fn invalid_slug_skipped_without_network() {
    let scraper = PersonioScraper;

    let make_input = |companies: Vec<String>| BoardSearchInput {
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
        locale: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies,
    };
    let make_ctx = || ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    };

    // IP:port — the primary SSRF vector.
    let result = scraper
        .search(make_input(vec!["127.0.0.1:8443".to_string()]), make_ctx())
        .await;
    assert!(result.is_ok(), "invalid slug must return Ok");
    assert!(
        result.unwrap().is_empty(),
        "SSRF slug must produce empty result (skipped, no network)"
    );

    // Dotted host.
    let result = scraper
        .search(make_input(vec!["dotted.host".to_string()]), make_ctx())
        .await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty(), "dotted slug must be skipped");

    // Path injection.
    let result = scraper
        .search(make_input(vec!["127.0.0.1/foo".to_string()]), make_ctx())
        .await;
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_empty(),
        "path-injection slug must be skipped"
    );
}

// ── R5: Personio dotall regex — multi-line content capture ────────────────────
//
// `DESC_RE` uses the `(?s)` flag so `.` matches newlines.  Without it, a
// description spanning multiple lines would be truncated at the first `\n`.
// These tests are regression guards: they must FAIL if `(?s)` is removed.
//
// Same guard for `POSITION_RE`, which must capture multi-line position blocks.

#[test]
fn desc_re_captures_multiline_value_content() {
    let xml = r#"<jobDescription>
  <value>line one
line two
line three</value>
</jobDescription>"#;

    let cap = DESC_RE.captures(xml).expect("DESC_RE must match");
    let captured = cap.get(1).expect("group 1 must be present").as_str().trim();

    assert!(
        captured.contains("line one"),
        "DESC_RE must capture first line; got: {captured:?}"
    );
    assert!(
        captured.contains("line two"),
        "DESC_RE must capture second line (dotall required); got: {captured:?}"
    );
    assert!(
        captured.contains("line three"),
        "DESC_RE must capture third line (dotall required); got: {captured:?}"
    );
}

#[test]
fn position_re_captures_multiline_position_block() {
    let xml = r#"<position>
  <id>123</id>
  <name>Software Engineer</name>
  <jobDescription>
    <value>Build great things.
Work with great people.</value>
  </jobDescription>
</position>"#;

    let mut caps = POSITION_RE.captures_iter(xml);
    let cap = caps
        .next()
        .expect("POSITION_RE must match a position block");
    let block = cap.get(1).expect("group 1 must be present").as_str();

    assert!(
        block.contains("<id>123</id>"),
        "captured block must contain the id element; got: {block:?}"
    );
    assert!(
        block.contains("Software Engineer"),
        "captured block must span across newlines to include name; got: {block:?}"
    );
    assert!(
        block.contains("Build great things."),
        "captured block must include the description first line; got: {block:?}"
    );
}

#[test]
fn parse_xml_feed_extracts_multiline_description() {
    // End-to-end: parse_xml_feed must surface multiline descriptions intact
    // (after strip_html — here no HTML so the text passes through).
    let xml = r#"<?xml version="1.0"?>
<workzag-jobs>
  <position>
    <id>42</id>
    <name>Dev</name>
    <office>Berlin</office>
    <jobDescription>
      <value>Responsibility one.
Responsibility two.
Responsibility three.</value>
    </jobDescription>
    <createdAt>2024-01-01T00:00:00Z</createdAt>
  </position>
</workzag-jobs>"#;

    let positions = parse_xml_feed(xml);
    assert_eq!(positions.len(), 1, "one position must be parsed");
    let desc = &positions[0].description;
    assert!(
        desc.contains("Responsibility one."),
        "description must include first line; got: {desc:?}"
    );
    assert!(
        desc.contains("Responsibility two."),
        "description must include second line (dotall required); got: {desc:?}"
    );
    assert!(
        desc.contains("Responsibility three."),
        "description must include third line (dotall required); got: {desc:?}"
    );
}

#[test]
fn test_personio_scraper_id() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.id(), "personio");
}

#[test]
fn test_personio_scraper_display_name() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.display_name(), "Personio");
}

#[test]
fn test_personio_scraper_mode() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_personio_hosts_not_empty() {
    assert!(!HOSTS.is_empty());
    assert!(HOSTS.contains(&"jobs.personio.de"));
    assert!(HOSTS.contains(&"jobs.personio.com"));
}

#[test]
fn test_personio_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_personio_requires_company() {
    assert!(
        PersonioScraper.requires_company(),
        "Personio is an ATS board and must return true for requires_company()"
    );
}

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = PersonioScraper;
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
        locale: None,
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

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = PersonioScraper;
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
        locale: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["clark".to_string()], // clark.jobs.personio.de has confirmed live listings
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
    println!("personio: {} results", postings.len());
    println!("first: {:?}", first.title);
}
