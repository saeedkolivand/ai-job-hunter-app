//! `found-jobs`'s own argv — the tests that were the second half of the old
//! suite's `// ── argv parsing ──` block.

use super::*;
use crate::extension_bridge::agent_cli::tests::support::{found_jobs, s};

#[test]
fn parses_found_jobs_with_just_an_autopilot_id() {
    assert_eq!(
        parse_verb(&s(&["found-jobs", "ap-1"])).unwrap(),
        found_jobs(
            Some("ap-1"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false
        )
    );
}

#[test]
fn parses_found_jobs_with_no_autopilot_id_spans_every_autopilot() {
    // Issue #1168 — a bare `found-jobs` (no positional argument at all) is
    // now a VALID call, not a usage error: it means "every autopilot".
    assert_eq!(
        parse_verb(&s(&["found-jobs"])).unwrap(),
        found_jobs(None, None, None, None, None, None, None, None, false)
    );
}

#[test]
fn parses_found_jobs_with_flags_only_does_not_swallow_a_flag_as_the_autopilot_id() {
    // The first token is a `--flag`, so it must NOT be misread as a
    // positional `autopilotId` — flag parsing has to start at index 0.
    assert_eq!(
        parse_verb(&s(&["found-jobs", "--limit", "10"])).unwrap(),
        found_jobs(None, Some(10), None, None, None, None, None, None, false)
    );
}

#[test]
fn parses_found_jobs_with_limit_and_cursor() {
    assert_eq!(
        parse_verb(&s(&[
            "found-jobs",
            "ap-1",
            "--limit",
            "50",
            "--cursor",
            "100"
        ]))
        .unwrap(),
        found_jobs(
            Some("ap-1"),
            Some(50),
            Some("100"),
            None,
            None,
            None,
            None,
            None,
            false
        )
    );
}

#[test]
fn parses_found_jobs_with_every_new_filter_flag() {
    assert_eq!(
        parse_verb(&s(&[
            "found-jobs",
            "ap-1",
            "--min-score",
            "70",
            "--country",
            "Germany",
            "--remote",
            "true",
            "--applied",
            "false",
            "--query",
            "engineer",
            "--include-description",
        ]))
        .unwrap(),
        found_jobs(
            Some("ap-1"),
            None,
            None,
            Some(70.0),
            Some("Germany"),
            Some(true),
            Some(false),
            Some("engineer"),
            true
        )
    );
}

#[test]
fn rejects_found_jobs_a_non_bool_remote_or_applied_value() {
    assert!(parse_verb(&s(&["found-jobs", "ap-1", "--remote", "maybe"])).is_err());
    assert!(parse_verb(&s(&["found-jobs", "ap-1", "--applied", "yes"])).is_err());
}

#[test]
fn rejects_found_jobs_a_non_numeric_min_score() {
    assert!(parse_verb(&s(&["found-jobs", "ap-1", "--min-score", "abc"])).is_err());
}

/// Round 3 fix (B3-r3-F9) — the canonical unset-shell-variable repro
/// (`agent found-jobs "$AP_ID"` with `AP_ID` unset) is an EMPTY positional,
/// which must refuse as a blank selector, not fall through to the flag loop
/// and report "unknown argument" against the empty token itself.
#[test]
fn rejects_found_jobs_an_empty_positional_as_a_blank_selector_not_an_unknown_argument() {
    let err = parse_verb(&s(&["found-jobs", ""])).unwrap_err();
    assert_eq!(err.to_string(), BLANK_FOUND_JOBS_AUTOPILOT_ID_MESSAGE);
}

/// A whitespace-only id is a DIFFERENT mistake shape (not empty) — this
/// positional check must not swallow it; it is still refused, just by the
/// downstream `parse_autopilot_id_arg` trim once the payload is built, not
/// here.
#[test]
fn a_whitespace_only_found_jobs_positional_is_not_caught_by_the_empty_check() {
    assert_eq!(
        parse_verb(&s(&["found-jobs", " "])).unwrap(),
        found_jobs(Some(" "), None, None, None, None, None, None, None, false)
    );
}

/// B3-r1-F3 — `"1e400"`/`"inf"`/`"nan"` all parse as valid `f64` values
/// (`f64::INFINITY`/`f64::NAN`), so `.parse::<f64>()` alone accepted them;
/// `serde_json::json!` then serializes a non-finite `f64` as `null`, and the
/// filter silently vanished on the other end. Must be refused HERE, at
/// parse, rather than reaching the wire as an inert `null`.
#[test]
fn rejects_found_jobs_a_non_finite_min_score() {
    for bad in ["1e400", "inf", "-inf", "nan"] {
        assert!(
            parse_verb(&s(&["found-jobs", "ap-1", "--min-score", bad])).is_err(),
            "--min-score {bad} must be rejected, not silently accepted as non-finite"
        );
    }
}

#[test]
fn rejects_found_jobs_non_numeric_limit() {
    assert!(parse_verb(&s(&["found-jobs", "ap-1", "--limit", "abc"])).is_err());
}
