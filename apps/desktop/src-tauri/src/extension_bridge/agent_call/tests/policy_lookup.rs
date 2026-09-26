//! Tests for `<namespace>:<command>` policy-row lookup (`policy_lookup.rs`).

use super::super::super::agent_cli::policy::Effect;
use super::super::policy_lookup::find_policy;
use super::super::*;

#[test]
fn split_path_takes_the_last_segment_as_command_and_the_one_before_as_namespace() {
    assert_eq!(
        split_path("commands::jobs::jobs_list"),
        ("jobs", "jobs_list")
    );
    // A 2-segment path (no `commands::` prefix) works identically —
    // `updater::updater_check` is the real POLICY row this covers.
    assert_eq!(
        split_path("updater::updater_check"),
        ("updater", "updater_check")
    );
    // A module path with its OWN `commands` segment in the middle
    // (`export::commands::...`) still resolves to the segment
    // IMMEDIATELY before the command, not the first one.
    assert_eq!(
        split_path("export::commands::documents_export_document"),
        ("commands", "documents_export_document")
    );
}

#[test]
fn find_policy_matches_a_real_row_by_its_derived_namespace_and_command() {
    let entry = find_policy("jobs", "jobs_list").expect("jobs_list is a real POLICY row");
    assert_eq!(entry.path, "commands::jobs::jobs_list");
    assert_eq!(entry.effect, Effect::Read);
}

#[test]
fn find_policy_refuses_a_command_name_under_the_wrong_namespace() {
    // `jobs_list` is real, but `jobs_list`'s OWN namespace is `jobs`, not
    // `wrongns` — a typo'd namespace must not fall back to matching on
    // the command name alone (see `find_policy`'s own doc).
    assert!(find_policy("wrongns", "jobs_list").is_none());
}

#[test]
fn find_policy_refuses_a_command_that_does_not_exist_at_all() {
    assert!(find_policy("jobs", "delete_everything").is_none());
}

// ── namespace_suggestion / unknown_command_detail (issue #1163) ──────────

/// The exact repro shape a caller hits: the real command name typed under
/// the wrong namespace — `namespace_suggestion` must name `jobs`, the ONE
/// real namespace `jobs_list` is registered under, never a guess among
/// several.
#[test]
fn namespace_suggestion_names_the_one_real_namespace_for_a_real_command_typed_wrong() {
    assert_eq!(namespace_suggestion("jobs_list"), Some("jobs"));
}

#[test]
fn namespace_suggestion_is_none_for_a_command_name_that_does_not_exist_at_all() {
    // Not just the wrong namespace — the COMMAND itself is fictional, so
    // there is nothing real to suggest.
    assert_eq!(namespace_suggestion("delete_everything"), None);
}

#[test]
fn unknown_command_detail_names_the_suggested_namespace_when_one_exists() {
    let detail = unknown_command_detail(Some("jobs"));
    assert!(detail.contains('`') && detail.contains("jobs"), "{detail}");
}

#[test]
fn unknown_command_detail_falls_back_to_the_generic_wording_with_no_suggestion() {
    let detail = unknown_command_detail(None);
    assert!(!detail.contains("registered under namespace"), "{detail}");
    assert!(
        detail.contains("agent schema") || detail.contains("commands"),
        "{detail}"
    );
}

/// End-to-end (of the pure parts): `dispatch`'s own `Refusal::UnknownCommand`
/// construction calls `namespace_suggestion` with the CALLER's bare command
/// name — mirrored here via `find_policy`'s failure path, the same
/// derivation `dispatch` uses, so this fails if that call site is ever
/// dropped or reordered.
#[test]
fn a_real_command_under_the_wrong_namespace_produces_a_refusal_naming_the_right_one() {
    assert!(find_policy("wrongns", "jobs_list").is_none());
    let refusal = Refusal::UnknownCommand(namespace_suggestion("jobs_list"));
    assert_eq!(refusal.sentinel(), "unknown_command");
    assert!(refusal.detail().contains("jobs"));
}

/// Pulls the REAL `extension_bridge_status` row and drives it through the
/// real production [`gate`] — not a hand-typed `Effect::NotExposed`
/// literal — so a future revert of that row back to `Read` fails HERE,
/// against the actual dispatch decision `handle_agent_call` makes, not only
/// against `policy::tests`' own shape check. MCP security critique: this is
/// the bridge's plaintext pairing token; the generic tier (and every MCP
/// `call-read` client one hop further out) must never dispatch it.
#[test]
fn the_real_extension_bridge_status_row_refuses_through_the_real_gate() {
    let entry = find_policy("extension_bridge", "extension_bridge_status")
        .expect("extension_bridge_status is a real POLICY row");
    assert!(
        matches!(
            super::super::gate(entry.effect, None),
            Err(Refusal::NotExposed(_))
        ),
        "extension_bridge_status must refuse through gate() with no confirm"
    );
    assert!(
        matches!(
            super::super::gate(entry.effect, Some("anything")),
            Err(Refusal::NotExposed(_))
        ),
        "extension_bridge_status must refuse through gate() even WITH a confirm"
    );
}

/// Same shape as the test above, for the same reason on a value that is not a
/// secret: `system_agent_cli_info` returns this binary's absolute path, which
/// on Windows and macOS lives under the user's home directory. Flipping that
/// row back to `Effect::Read` in `policy.rs` — the exact mutation this pins,
/// verified by hand — makes BOTH assertions below fail, because `gate` then
/// answers `Ok(Dispatch::Direct)` and `call-read` would ship a user path into
/// an MCP client's persisted transcript. No other test catches that flip: it
/// changes no row COUNT (`policy_table_has_exactly_167_rows`, the 34
/// Irreversible tally, `extension_bridge::test`'s 168-row walk are all blind
/// to an `Effect` swap), `not_exposed_rows_carry_a_real_reason` only inspects
/// rows that ARE already `NotExposed`, and the per-row walk in
/// `extension_bridge::test` keys its assertions off `entry.effect` itself, so
/// a reverted row just moves to a different self-consistent branch.
#[test]
fn the_real_system_agent_cli_info_row_refuses_through_the_real_gate() {
    let entry = find_policy("system", "system_agent_cli_info")
        .expect("system_agent_cli_info is a real POLICY row");
    assert!(
        matches!(
            super::super::gate(entry.effect, None),
            Err(Refusal::NotExposed(_))
        ),
        "system_agent_cli_info must refuse through gate() with no confirm"
    );
    assert!(
        matches!(
            super::super::gate(entry.effect, Some("anything")),
            Err(Refusal::NotExposed(_))
        ),
        "system_agent_cli_info must refuse through gate() even WITH a confirm"
    );
}

// ── Refusal sentinels/details (pure) ────────────────────────────────────
