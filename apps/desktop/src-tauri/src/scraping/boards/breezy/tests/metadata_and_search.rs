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
fn test_breezy_scraper_id() {
    let scraper = BreezyScraper;
    assert_eq!(scraper.id(), "breezy");
}

#[test]
fn test_breezy_scraper_display_name() {
    let scraper = BreezyScraper;
    assert_eq!(scraper.display_name(), "Breezy HR");
}

#[test]
fn test_breezy_scraper_mode() {
    let scraper = BreezyScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_breezy_requires_company() {
    assert!(
        BreezyScraper.requires_company(),
        "Breezy HR is an ATS board and must return true for requires_company()"
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
// URL guard — is_https_url (userinfo / scheme sanity check)
// ---------------------------------------------------------------------------

#[test]
fn url_guard_accepts_plain_https_rejects_others() {
    assert!(is_https_url("https://acme.breezy.hr/p/abc123"));
    assert!(
        !is_https_url("http://acme.breezy.hr/p/abc123"),
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
// parse_breezy_date — RFC3339 and bare-date formats
// ---------------------------------------------------------------------------

#[test]
fn parse_breezy_date_accepts_rfc3339_and_bare_date() {
    let rfc3339 = parse_breezy_date("2024-03-15T00:00:00Z");
    assert!(rfc3339.is_some());
    let bare = parse_breezy_date("2024-03-15");
    assert!(bare.is_some());
    assert_eq!(
        rfc3339, bare,
        "midnight RFC3339 and bare date for the same day must match"
    );
    assert!(parse_breezy_date("not-a-date").is_none());
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = BreezyScraper;
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
    let scraper = BreezyScraper;
    let result = scraper
        .search(make_input(vec!["dotted.host".to_string()]), make_ctx())
        .await;
    let err = result.expect_err("an all-invalid-slug run must be a board error, not a silent zero");
    assert!(
        err.to_string().contains("slug(s) invalid"),
        "error must name the invalid-slug reason, got: {err}"
    );
}

/// trust-H item 3: an all-invalid-slug run is a whole-board FAILURE (Err), not a
/// partial — so it must NOT emit a `slugs-invalid` partial note (that note is
/// only for SOME-rejected-with-a-success runs). Wires an `on_note` sink and
/// asserts it stayed empty. Network-free (every slug is rejected pre-fetch).
#[tokio::test]
async fn all_invalid_slugs_emits_no_note_and_errors() {
    let scraper = BreezyScraper;
    let notes = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = notes.clone();
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: Some(std::sync::Arc::new(move |n: String| {
            sink.lock().unwrap().push(n);
        })),
    };
    let result = scraper
        .search(make_input(vec!["dotted.host".to_string()]), ctx)
        .await;
    assert!(
        result.is_err(),
        "an all-invalid-slug run must be a board error"
    );
    assert!(
        notes.lock().unwrap().is_empty(),
        "an all-reject run must NOT emit a partial note — it's an error, not a partial"
    );
}

/// A pre-cancelled signal must make the loop break immediately without
/// recording `first_fetch_error`.
#[tokio::test]
async fn cancelled_before_fetch_returns_ok_not_err() {
    let scraper = BreezyScraper;
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

/// Pins the exact scenario a HIGH-severity review finding raised: a cancel
/// firing AFTER an invalid slug is rejected but BEFORE a later valid slug is
/// reached must not be misattributed as "all slugs invalid" (a benign
/// cancellation, blamed on the user's company-name config). Uses the
/// `on_progress` callback the reject branch already calls as the timing seam —
/// the closure cancels the token the instant the first (invalid) slug is
/// rejected; the second (valid-shaped) slug is never reached because the
/// loop's top-of-iteration cancellation check breaks first.
#[tokio::test]
async fn cancel_after_reject_before_next_slug_returns_ok_not_all_invalid_error() {
    let scraper = BreezyScraper;
    let signal = tokio_util::sync::CancellationToken::new();
    let cancel_on_progress = signal.clone();
    let ctx = ScrapeContext {
        signal: signal.clone(),
        on_progress: Some(Box::new(move |_p: f32| cancel_on_progress.cancel())),
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let result = scraper
        .search(
            make_input(vec!["dotted.host".to_string(), "acme".to_string()]),
            ctx,
        )
        .await;
    assert!(
        result.is_ok(),
        "a cancel firing right after a reject must return Ok (interrupted), not the \
         all-slugs-invalid error — got {:?}",
        result.err()
    );
}
