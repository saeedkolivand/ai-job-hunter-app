use super::*;

// ── ats_all_fetches_failed ───────────────────────────────────────────────────
//
// `common.rs` (not a board's own `mod.rs`) is the natural home for these
// tests: `ats_all_fetches_failed` is shared by 10 ATS boards (lever, recruitee,
// smartrecruiters, greenhouse, ashby, breezy, bamboohr, pinpoint, rippling,
// workable) and `should_propagate_page_error` by 3 paginated boards (themuse,
// arbeitnow, arbeitsagentur) — unlike `normalize_companies`/`is_https_url`
// (2-3 users each, tested by per-board copy in each user's `test.rs`),
// duplicating one test suite across 10+ board files would be pure repetition
// of the exact same assertions against the exact same pure fn.

#[test]
fn all_fail_returns_error_naming_board_and_first_error() {
    // successful_fetches == 0 AND a real error was recorded → Some(message)
    // that names the board id and carries the first recorded error verbatim
    // (this is what lands in `BoardScrapeSummary.error` — must be attributable
    // to a specific board, not a bare "HTTP 403").
    let result = ats_all_fetches_failed("lever", 0, &Some("HTTP 403".to_string()));
    assert_eq!(
        result.as_deref(),
        Some("all lever company fetches failed: HTTP 403"),
        "all-fail message must name the board id and carry the first error verbatim"
    );
}

#[test]
fn partial_success_returns_none_even_with_a_recorded_error() {
    // At least one company succeeded → None, regardless of whether an earlier
    // slug also recorded a fetch error. Partial success must be kept, not
    // treated as a board failure (this is the case the audit called out —
    // one bad slug in a multi-company batch must not zero out the good ones).
    let result = ats_all_fetches_failed("smartrecruiters", 1, &Some("HTTP 404".to_string()));
    assert!(
        result.is_none(),
        "at least one successful fetch must suppress the all-fail error, got {result:?}"
    );

    // Also true with more than one success and no recorded error at all.
    let clean = ats_all_fetches_failed("smartrecruiters", 3, &None);
    assert!(clean.is_none());
}

/// Zero-companies / nothing-attempted edge: `successful_fetches == 0` AND no
/// error was ever recorded (every slug rejected by a pre-fetch validation
/// guard, e.g. `is_valid_dns_label_slug`, before any fetch ran — or the
/// company list was empty). This is the exact state every ATS board reaches
/// when its per-company loop never records a fetch attempt.
///
/// Must be `None` — "nothing to fetch" is not a board failure. Confirmed
/// behavior-preserving against the pre-extraction inline shape (still visible
/// in `smartrecruiters/mod.rs`: `if successful_fetches == 0 { if let
/// Some(error) = first_fetch_error { return Err(...) } }`, falling through to
/// `Ok(out)` when `first_fetch_error` is `None`) — the extraction did not
/// change this edge's outcome.
#[test]
fn zero_companies_edge_no_error_recorded_returns_none() {
    let result = ats_all_fetches_failed("bamboohr", 0, &None);
    assert!(
        result.is_none(),
        "no companies ever attempted (0 successes, no recorded error) must not synthesize a \
         board error — got {result:?}"
    );
}

// ── should_propagate_page_error ──────────────────────────────────────────────

#[test]
fn zero_collected_propagates_page_failure() {
    // Page 0 (or any page reached with nothing collected yet) failing must
    // propagate as a board error — this is what fixed themuse's page-0 silent
    // zero (a blocked/rotted feed on the very first page must not look like a
    // genuine empty result).
    assert!(
        should_propagate_page_error(0),
        "a page failure with nothing collected yet must propagate"
    );
}

#[test]
fn partial_harvest_is_kept_not_propagated() {
    // A later page failing after some items were already streamed must stop
    // pagination and keep the partial result, not discard everything already
    // collected by turning it into a board error.
    assert!(
        !should_propagate_page_error(1),
        "a page failure after 1 item was already collected must keep the partial harvest"
    );
    assert!(
        !should_propagate_page_error(50),
        "a page failure after many items were already collected must keep the partial harvest"
    );
}

// ── canonical_job_key (trust PR E, stage 1) ──────────────────────────────────
//
// The app-wide cross-source dedup key. Stage 2 (autopilot merge) and stage 3
// (renderer `mergePostings`, mirrored in TS) key on the EXACT same algorithm, so
// these assertions pin the contract all three stages must agree on.

#[test]
fn canonical_key_same_url_across_boards_collapses() {
    // www/tracking variants of the same URL from different boards → same key
    // (URL identity via normalize_job_url); differing titles don't matter once a
    // usable URL exists (the URL is the identity).
    let a = canonical_job_key(
        "https://www.acme.example/jobs/42?utm_source=x",
        "Senior Engineer",
        "Acme",
    );
    let b = canonical_job_key("https://acme.example/jobs/42", "Sr. Engineer", "Acme Inc");
    assert_eq!(
        a, b,
        "same canonical URL must key identically regardless of board/title"
    );
    assert_eq!(a, "https://acme.example/jobs/42");
}

#[test]
fn canonical_key_urlless_matches_on_title_and_company() {
    // No usable URL → normalized `title\u{1}company` key; case- and
    // whitespace(edge)-insensitive via trim + lowercase.
    let a = canonical_job_key("", " Senior Rust Engineer ", "Acme");
    let b = canonical_job_key("", "senior rust engineer", "  ACME ");
    assert_eq!(
        a, b,
        "same title+company must key identically when no URL exists"
    );
    assert_eq!(a, "senior rust engineer\u{1}acme");
}

/// Cross-language drift fixture — the TS mirror
/// (`features/jobs/lib/canonical-job-key.ts`) asserts against this EXACT same
/// title/company pair, so a divergence in either language's lowercasing rules
/// is caught by comparing the two expected strings side by side.
#[test]
fn canonical_key_urlless_non_ascii_lowercases_correctly() {
    let key = canonical_job_key("", "Développeur Sénior", "Müller GmbH");
    assert_eq!(key, "développeur sénior\u{1}müller gmbh");
}

#[test]
fn canonical_key_near_miss_titles_stay_distinct() {
    // Precision requirement: a broader/narrower title at the same company must
    // NOT merge.
    let senior = canonical_job_key("", "Senior Rust Engineer", "Acme");
    let plain = canonical_job_key("", "Rust Engineer", "Acme");
    assert_ne!(senior, plain, "near-miss titles must stay distinct");
}

#[test]
fn canonical_key_separator_cannot_be_forged_by_title() {
    // The U+0001 (SOH) separator means a title that merely CONTAINS the company
    // name can't collide with a genuine title+company split.
    let forged = canonical_job_key("", "Engineer Acme", "");
    let genuine = canonical_job_key("", "Engineer", "Acme");
    assert_ne!(
        forged, genuine,
        "the SOH separator must prevent a title-contains-company collision"
    );
}

#[test]
fn canonical_key_empty_or_non_http_url_falls_back_to_title_company() {
    // Empty, whitespace-only, and dangerous non-http(s) schemes all normalize to
    // "" (no openable link) → fall back to the title+company key.
    let expected = "t\u{1}co";
    assert_eq!(canonical_job_key("", "T", "Co"), expected);
    assert_eq!(canonical_job_key("   ", "T", "Co"), expected);
    assert_eq!(
        canonical_job_key("javascript:alert(1)", "T", "Co"),
        expected
    );
    assert_eq!(canonical_job_key("file:///etc/passwd", "T", "Co"), expected);
}
