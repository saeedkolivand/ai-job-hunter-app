use super::super::super::test_support::*;
use super::super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(super) fn make_input(companies: Vec<String>) -> BoardSearchInput {
    BoardSearchInput {
        companies,
        ..default_search_input()
    }
}

pub(super) fn make_ctx() -> ScrapeContext {
    default_ctx()
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_rippling_scraper_id() {
    let scraper = RipplingScraper;
    assert_eq!(scraper.id(), "rippling");
}

#[test]
fn test_rippling_scraper_display_name() {
    let scraper = RipplingScraper;
    assert_eq!(scraper.display_name(), "Rippling");
}

#[test]
fn test_rippling_scraper_mode() {
    let scraper = RipplingScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_rippling_requires_company() {
    assert!(
        RipplingScraper.requires_company(),
        "Rippling is an ATS board and must return true for requires_company()"
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
// Slug guard — is_valid_rippling_slug (path segment, not a DNS label: mixed
// case is allowed, but path-traversal/query-injection characters are not)
// ---------------------------------------------------------------------------

#[test]
fn slug_validation_accepts_mixed_case_and_valid_slugs() {
    assert!(is_valid_rippling_slug("acme"));
    assert!(
        is_valid_rippling_slug("Acme-Corp"),
        "mixed case must be accepted — this is a URL path segment, not a DNS label"
    );
    assert!(is_valid_rippling_slug("ACME123"));
    assert!(
        is_valid_rippling_slug(&"a".repeat(63)),
        "exactly 63 chars must be accepted"
    );
}

#[test]
fn slug_validation_rejects_path_traversal_and_invalid_slugs() {
    assert!(!is_valid_rippling_slug("acme.corp"), "dot must be rejected");
    assert!(
        !is_valid_rippling_slug("acme/corp"),
        "slash must be rejected"
    );
    assert!(
        !is_valid_rippling_slug("../etc/passwd"),
        "path traversal must be rejected"
    );
    assert!(!is_valid_rippling_slug("acme@corp"), "@ must be rejected");
    assert!(
        !is_valid_rippling_slug("acme_corp"),
        "underscore must be rejected"
    );
    assert!(
        !is_valid_rippling_slug("-acme"),
        "leading hyphen must be rejected"
    );
    assert!(
        !is_valid_rippling_slug("acme-"),
        "trailing hyphen must be rejected"
    );
    assert!(!is_valid_rippling_slug(""), "empty slug must be rejected");
    assert!(
        !is_valid_rippling_slug(&"a".repeat(64)),
        "exceeds 63-char limit"
    );
}

// ---------------------------------------------------------------------------
// Job URL guard — is_valid_rippling_job_url (host allowlist)
// ---------------------------------------------------------------------------

#[test]
fn job_url_guard_accepts_ats_host_rejects_others() {
    assert!(is_valid_rippling_job_url(
        "https://ats.rippling.com/acme/jobs/abc"
    ));
    assert!(
        !is_valid_rippling_job_url("http://ats.rippling.com/acme/jobs/abc"),
        "non-https must be rejected"
    );
    assert!(
        !is_valid_rippling_job_url("https://evil.example/acme/jobs/abc"),
        "wrong host must be rejected"
    );
    assert!(!is_valid_rippling_job_url("not-a-url"));
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = RipplingScraper;
    let result = scraper.search(make_input(Vec::new()), make_ctx()).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

/// An all-invalid-slug run rejects every slug pre-fetch (no network — the SSRF
/// guard) and now surfaces a distinct board error instead of a silent zero
/// (claude review #597).
#[tokio::test]
async fn all_invalid_slugs_error_without_network() {
    let scraper = RipplingScraper;
    let result = scraper
        .search(make_input(vec!["dotted.host".to_string()]), make_ctx())
        .await;
    let err = result.expect_err("an all-invalid-slug run must be a board error, not a silent zero");
    assert!(
        err.to_string().contains("slug(s) invalid"),
        "error must name the invalid-slug reason, got: {err}"
    );
}

/// A pre-cancelled signal must make the loop break immediately without
/// recording `first_fetch_error`.
#[tokio::test]
async fn cancelled_before_fetch_returns_ok_not_err() {
    let scraper = RipplingScraper;
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
