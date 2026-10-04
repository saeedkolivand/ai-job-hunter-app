//! `scrape_diagnostics`: the user-visible "why did this run come up short" line.

use super::super::*;
use crate::autopilot::tests::support::board_summary;
use crate::scraping::BoardScrapeSummary;

fn summary(board: &str, error: Option<&str>, skipped: Option<&str>) -> BoardScrapeSummary {
    board_summary(board, 0, error, skipped, None)
}

#[test]
fn empty_slice_returns_empty_string() {
    assert_eq!(scrape_diagnostics(&[]), "");
}

#[test]
fn board_with_error_and_no_skip_shows_error() {
    let s = summary("linkedin", Some("429 Too Many Requests"), None);
    let diag = scrape_diagnostics(&[s]);
    assert!(
        diag.contains("linkedin"),
        "board name must appear; got: {diag}"
    );
    assert!(
        diag.contains("429 Too Many Requests"),
        "error text must appear; got: {diag}"
    );
}

#[test]
fn exact_format_is_board_colon_space_reason() {
    // Pin the exact `"<board>: <reason>"` format the impl produces.
    // If the separator or spacing ever changes this test catches it immediately.
    assert_eq!(
        scrape_diagnostics(&[summary("aggregator", Some("network timeout"), None)]),
        "aggregator: network timeout"
    );
}

#[test]
fn skipped_only_board_appears_in_output() {
    // A board that was skipped (no error) must still surface its reason.
    let diag = scrape_diagnostics(&[summary("glassdoor", None, Some("needs-login"))]);
    assert!(
        diag.contains("needs-login"),
        "skipped reason must appear; got: {diag}"
    );
}

#[test]
fn truncated_only_board_appears_in_output() {
    // A paginated board (stage 1) that kept only a partial harvest — no
    // `error`, no `skipped` — must still surface its truncation reason, not
    // be silently treated as a clean run.
    let s = board_summary(
        "arbeitnow",
        0,
        None,
        None,
        Some("page 2 of 5 failed: HTTP 429"),
    );
    assert_eq!(
        scrape_diagnostics(&[s]),
        "arbeitnow: page 2 of 5 failed: HTTP 429"
    );
}

#[test]
fn error_takes_precedence_over_skipped_when_both_are_set() {
    // `error` is checked first in the `or` chain; `skipped` must be shadowed.
    let s = summary("aggregator", Some("network timeout"), Some("needs-login"));
    let diag = scrape_diagnostics(&[s]);
    assert!(
        diag.contains("network timeout"),
        "error must win over skipped; got: {diag}"
    );
    assert!(
        !diag.contains("needs-login"),
        "skipped must be suppressed when error is set; got: {diag}"
    );
}

#[test]
fn board_with_neither_error_nor_skipped_contributes_nothing() {
    let clean = summary("indeed", None, None);
    assert_eq!(scrape_diagnostics(&[clean]), "");
}

#[test]
fn multiple_errored_boards_are_joined_with_semicolon() {
    let summaries = vec![
        summary("linkedin", Some("rate-limited"), None),
        summary("indeed", None, Some("needs-login")),
    ];
    let diag = scrape_diagnostics(&summaries);
    // Both boards must appear; they are joined with "; ".
    assert!(
        diag.contains("linkedin"),
        "first board missing; got: {diag}"
    );
    assert!(diag.contains("indeed"), "second board missing; got: {diag}");
    assert!(
        diag.contains("; "),
        "boards must be joined with \"; \"; got: {diag}"
    );
}

#[test]
fn clean_board_mixed_with_errored_does_not_appear_in_output() {
    let summaries = vec![
        summary("linkedin", Some("timeout"), None),
        summary("remotive", None, None), // clean — must not appear
    ];
    let diag = scrape_diagnostics(&summaries);
    assert!(
        !diag.contains("remotive"),
        "clean board must not appear; got: {diag}"
    );
    assert!(
        !diag.contains("; "),
        "single problem board must not add trailing separator; got: {diag}"
    );
}

#[test]
fn diagnostics_redact_absolute_paths_and_urls() {
    // A raw error from an upstream `e.to_string()` carrying an absolute
    // Windows path, a Unix path, and a full URL must NOT leak any of them into
    // the user-visible step log (repo path-privacy rule).
    let raw = "failed to open C:\\Users\\alice\\secret.json or /home/alice/cfg via https://api.example.com/v1/jobs?token=abc";
    let diag = scrape_diagnostics(&[summary("aggregator", Some(raw), None)]);

    assert!(
        !diag.contains("C:\\Users\\alice"),
        "windows path leaked; got: {diag}"
    );
    assert!(
        !diag.contains("/home/alice"),
        "unix path leaked; got: {diag}"
    );
    assert!(
        !diag.contains("https://api.example.com"),
        "url leaked; got: {diag}"
    );
    assert!(
        !diag.contains("token=abc"),
        "url query/secret leaked; got: {diag}"
    );
    // The high-level message + placeholders survive.
    assert!(
        diag.contains("aggregator:"),
        "board prefix missing; got: {diag}"
    );
    assert!(
        diag.contains("failed to open"),
        "message dropped; got: {diag}"
    );
    assert!(
        diag.contains("<path-redacted>"),
        "path placeholder missing; got: {diag}"
    );
    assert!(
        diag.contains("<url-redacted>"),
        "url placeholder missing; got: {diag}"
    );
}
