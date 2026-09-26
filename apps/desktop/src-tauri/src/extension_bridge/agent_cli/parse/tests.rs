//! argv → `Verb`: the verb dispatch and `best-matches`'s own flags.

use super::*;
use crate::extension_bridge::agent_cli::tests::support::s;

// ── argv parsing ─────────────────────────────────────────────────────────

#[test]
fn parses_best_matches_with_no_flags() {
    assert_eq!(
        parse_verb(&s(&["best-matches"])).unwrap(),
        Verb::BestMatches {
            limit: None,
            cursor: None,
            query: None,
        }
    );
}

#[test]
fn parses_best_matches_with_limit() {
    assert_eq!(
        parse_verb(&s(&["best-matches", "--limit", "5"])).unwrap(),
        Verb::BestMatches {
            limit: Some(5),
            cursor: None,
            query: None,
        }
    );
}

#[test]
fn parses_best_matches_with_cursor_and_query() {
    assert_eq!(
        parse_verb(&s(&[
            "best-matches",
            "--cursor",
            "20",
            "--query",
            "engineer"
        ]))
        .unwrap(),
        Verb::BestMatches {
            limit: None,
            cursor: Some("20".to_string()),
            query: Some("engineer".to_string()),
        }
    );
}

#[test]
fn rejects_a_non_numeric_limit() {
    assert!(parse_verb(&s(&["best-matches", "--limit", "abc"])).is_err());
}

#[test]
fn rejects_limit_missing_its_value() {
    assert!(parse_verb(&s(&["best-matches", "--limit"])).is_err());
}

#[test]
fn parses_job_with_url() {
    assert_eq!(
        parse_verb(&s(&["job", "https://example.com/1"])).unwrap(),
        Verb::Job {
            url: "https://example.com/1".to_string()
        }
    );
}

#[test]
fn rejects_job_without_a_url() {
    assert!(parse_verb(&s(&["job"])).is_err());
}
#[test]
fn parses_the_three_no_arg_verbs() {
    assert_eq!(parse_verb(&s(&["profile"])).unwrap(), Verb::Profile);
    assert_eq!(parse_verb(&s(&["automations"])).unwrap(), Verb::Automations);
    assert_eq!(parse_verb(&s(&["schema"])).unwrap(), Verb::Schema);
}

#[test]
fn rejects_an_unknown_verb() {
    assert!(parse_verb(&s(&["delete-everything"])).is_err());
}

#[test]
fn rejects_a_missing_verb() {
    assert!(parse_verb(&s(&[])).is_err());
}

#[test]
fn unknown_verb_error_never_echoes_the_typed_argv_token() {
    // LOW fix (security review): argv can carry a path/username, and
    // this reply lands in an agent transcript — the error must list the
    // allowed verbs, never the token the caller typed.
    let leaky = r"C:\Users\alice\Desktop\secret-notes";
    let err = parse_verb(&s(&[leaky])).unwrap_err().to_string();
    assert!(
        !err.contains(leaky),
        "unknown-verb error must not echo argv: {err}"
    );
    assert!(
        err.contains("best-matches"),
        "must list the allowed verbs: {err}"
    );
}

#[test]
fn unknown_best_matches_flag_error_never_echoes_the_typed_argv_token() {
    // MINOR fix (security review round 2): the SAME leak one branch over
    // — `ajh-tauri agent best-matches "/home/alice/secret"` used to put
    // that path straight into the exit-2 `detail` field. Named flags
    // instead, mirroring the unknown-verb branch's own fix above.
    let leaky = r"C:\Users\alice\Desktop\secret-notes";
    let err = parse_verb(&s(&["best-matches", leaky]))
        .unwrap_err()
        .to_string();
    assert!(
        !err.contains(leaky),
        "unknown-argument error must not echo argv: {err}"
    );
    assert!(
        err.contains("--limit"),
        "must name the flag this verb accepts: {err}"
    );
}
