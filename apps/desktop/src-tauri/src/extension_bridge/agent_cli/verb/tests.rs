//! `Verb`'s own surface: the wire/reply types and payload it emits, and the
//! `VERB_TABLE` ↔ `--help` ↔ parser anti-drift loop that keeps the three in step.

use super::*;
use crate::extension_bridge::agent_cli::tests::support::{found_jobs, s};

#[test]
fn call_verb_sends_the_agent_call_wire_type_and_expects_its_own_reply_type() {
    let verb = Verb::Call {
        namespace: "jobs".to_string(),
        command: "jobs_list".to_string(),
        input: serde_json::json!({}),
        confirm: None,
    };
    assert_eq!(verb.wire_type(), msg::AGENT_CALL);
    assert_eq!(verb.reply_type(), msg::AGENT_CALL_RESULT);
    assert_eq!(verb.resource_name(), "call");
    let payload = verb.payload();
    assert_eq!(payload["namespace"], "jobs");
    assert_eq!(payload["command"], "jobs_list");
    assert_eq!(payload["input"], serde_json::json!({}));
    assert!(
        payload.get("confirm").is_none(),
        "confirm must be absent from the payload when not supplied, not null"
    );
}

#[test]
fn call_verb_payload_carries_confirm_only_when_supplied() {
    let verb = Verb::Call {
        namespace: "documents".to_string(),
        command: "documents_remove".to_string(),
        input: serde_json::json!({ "id": "doc-1" }),
        confirm: Some("Resume A".to_string()),
    };
    assert_eq!(verb.payload()["confirm"], "Resume A");
}

#[test]
fn curated_verbs_still_send_agent_query_and_expect_agent_result() {
    assert_eq!(Verb::Schema.wire_type(), msg::AGENT_QUERY);
    assert_eq!(Verb::Schema.reply_type(), msg::AGENT_RESULT);
}
// ── --help / VERB_TABLE anti-drift (owner request) ──────────────────────
// Hand-written literal list, not derived from VERB_TABLE itself (mirrors
// the repo's standing "pair a loop-over-own-fields test with a
// hand-written literal list" lesson).

#[test]
fn verb_table_names_match_a_hand_written_literal_list() {
    let mut names: Vec<&str> = VERB_TABLE.iter().map(|v| v.name).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec![
            "automations",
            "best-matches",
            "call",
            "found-jobs",
            "job",
            "profile",
            "schema"
        ]
    );
}

/// Every verb [`VERB_TABLE`] names must actually be parseable — the
/// first half of the owner's anti-drift requirement.
#[test]
fn every_verb_in_the_table_is_parseable_with_its_minimal_args() {
    for v in VERB_TABLE {
        let args: Vec<String> = match v.name {
            "job" => s(&["job", "https://example.com/1"]),
            "found-jobs" => s(&["found-jobs", "ap-1"]),
            "call" => s(&["call", "jobs:jobs_list"]),
            other => s(&[other]),
        };
        assert!(
            parse_verb(&args).is_ok(),
            "verb `{}` listed in VERB_TABLE must parse",
            v.name
        );
    }
}

/// Every `--flag` token in `text`, in the exact form `parse_verb`'s `match`
/// arms key on (`--min-score`, never `--min-score <n>`) — scans for `--`
/// then stops at the first character that isn't alphanumeric or `-`.
fn flags_named_in(text: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let mut rest = text;
    while let Some(pos) = rest.find("--") {
        let candidate = &rest[pos..];
        let end = candidate
            .char_indices()
            .skip(2)
            .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '-'))
            .map(|(idx, _)| idx)
            .unwrap_or(candidate.len());
        out.insert(candidate[..end].to_string());
        rest = &candidate[end.max(1)..];
    }
    out
}

/// Round 3 fix (B3-r3-F10) — `found-jobs`' flag list is now hand-maintained
/// in five places (`VerbHelp.args`, `parse_found_jobs`'s match arms, its
/// unknown-argument message, the MCP `inputSchema`, and `tool_argv`) with no
/// test tying any two together; `every_verb_in_the_table_is_parseable_with_its_minimal_args`
/// only ever parses the verb's MINIMAL args, never touching a single flag.
/// Closes the loop between the two surfaces this file can see without an
/// `AppHandle`: `VerbHelp.args` (what `--help` advertises) and each verb's
/// own `parse_*` function (what it actually accepts), both directions —
/// FORWARD: every flag `args` documents must be recognized (a benign dummy
/// value may still fail ITS OWN type check, but never the catch-all "unknown
/// argument"); REVERSE: triggering the catch-all with a definitely-bogus
/// flag must list ONLY flags `args` already documents, so a flag added to a
/// `match` arm without updating `args` shows up here, not just silently in
/// production.
#[test]
fn every_advertised_found_jobs_and_best_matches_flag_round_trips_with_the_parser() {
    let cases: &[(&str, &[&str])] = &[("best-matches", &[]), ("found-jobs", &["ap-1"])];
    for &(verb, minimal) in cases {
        let help = VERB_TABLE.iter().find(|v| v.name == verb).unwrap();
        let documented = flags_named_in(help.args);
        assert!(
            !documented.is_empty(),
            "verb `{verb}` has no documented flags to check"
        );

        for flag in &documented {
            let mut args = s(&[verb]);
            args.extend(minimal.iter().map(|s| s.to_string()));
            args.push(flag.clone());
            if flag.as_str() != "--include-description" {
                args.push("true".to_string());
            }
            if let Err(e) = parse_verb(&args) {
                assert!(
                    !e.to_string().starts_with("unknown argument"),
                    "verb `{verb}` documents `{flag}` in VERB_TABLE but the parser doesn't \
                     recognize it: {e}"
                );
            }
        }

        // T4 hardening: this must FAIL, not merely permit failure — a
        // vacuous `if let Err(e) = ...` with no `else` would pass even if
        // `parse_verb` stopped rejecting an unknown flag entirely, and no
        // other test in this file covers that direction.
        let mut bogus = s(&[verb]);
        bogus.extend(minimal.iter().map(|s| s.to_string()));
        bogus.push("--definitely-not-a-real-flag".to_string());
        let err = parse_verb(&bogus).expect_err("an undocumented flag must be refused");
        let msg = err.to_string();
        assert!(
            msg.starts_with("unknown argument"),
            "verb `{verb}` refused `--definitely-not-a-real-flag` for the wrong reason: {msg}"
        );
        for flag in flags_named_in(&msg) {
            assert!(
                documented.contains(&flag),
                "verb `{verb}`'s unknown-argument message lists `{flag}` but \
                 VERB_TABLE's args string doesn't document it: {msg}"
            );
        }
    }
}
#[test]
fn payload_carries_the_wire_resource_name() {
    assert_eq!(Verb::Schema.payload()["resource"], "schema");
    assert_eq!(
        Verb::Job {
            url: "https://x.example.com".to_string()
        }
        .payload()["url"],
        "https://x.example.com"
    );
    let with_limit = Verb::BestMatches {
        limit: Some(7),
        cursor: None,
        query: None,
    }
    .payload();
    assert_eq!(with_limit["limit"], 7);
    let without_limit = Verb::BestMatches {
        limit: None,
        cursor: None,
        query: None,
    }
    .payload();
    assert!(without_limit.get("limit").is_none());

    let found_jobs_scoped = found_jobs(
        Some("ap-1"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
    )
    .payload();
    assert_eq!(found_jobs_scoped["autopilotId"], "ap-1");
    let found_jobs_spanning =
        found_jobs(None, None, None, None, None, None, None, None, false).payload();
    assert!(
        found_jobs_spanning.get("autopilotId").is_none(),
        "an omitted autopilotId must be absent from the payload, not null"
    );
}
