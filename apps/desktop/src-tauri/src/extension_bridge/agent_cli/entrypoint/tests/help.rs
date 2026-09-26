//! The `--help` text itself, and the second half of the owner's anti-drift
//! requirement (`verb/tests.rs` pins the table side of the same loop).

use super::super::*;
use crate::extension_bridge::agent_cli::tests::support::s;

/// Every parseable verb must appear in the help text — the SECOND half
/// of the owner's anti-drift requirement (together with the test above,
/// this pins BOTH directions: help ⊆ parseable AND parseable ⊆ help).
#[test]
fn help_text_names_every_verb_in_the_table() {
    let text = help_text();
    for v in VERB_TABLE {
        assert!(
            text.contains(v.name),
            "help text is missing verb `{}`: {text}",
            v.name
        );
    }
    assert!(text.contains("--help"));
    assert!(text.contains("EXIT CODES"));
}

#[test]
fn help_text_lists_every_error_sentinel_this_cli_can_emit() {
    let text = help_text();
    for (sentinel, _) in ERROR_SENTINELS {
        assert!(
            text.contains(sentinel),
            "help text is missing sentinel `{sentinel}`: {text}"
        );
    }
}

/// The exit-2 entry has to stay true for the app-side refusals that ALSO
/// exit 2 (issue #1135). It used to say the round trip never completed or
/// the usage was invalid — both false for a `result_too_large`, which is
/// raised after the command RAN, so an agent reading it would conclude a
/// mutating call was safe to re-send. Pins the two claims that make it
/// honest, plus the pointer that keeps the app-side names OUT of this CLI's
/// own [`ERROR_SENTINELS`] table (a second hand-typed copy is the drift the
/// table exists to prevent).
#[test]
fn help_texts_exit_2_entry_covers_an_app_side_refusal_and_warns_the_command_may_have_run() {
    let text = help_text();
    let exit_2 = text
        .lines()
        .find(|l| l.trim_start().starts_with("2   "))
        .expect("the EXIT CODES block has a `2` row");
    assert!(
        exit_2.contains("refused") && exit_2.contains("may already have run"),
        "the exit-2 row must cover an app-side refusal AND warn the command may have run: {exit_2:?}"
    );
    assert!(
        exit_2.contains(agent_call::ERR_RESULT_TOO_LARGE),
        "the exit-2 row must name the refusal that warning is about: {exit_2:?}"
    );
    assert!(
        !ERROR_SENTINELS
            .iter()
            .any(|(sentinel, _)| *sentinel == agent_call::ERR_RESULT_TOO_LARGE),
        "an app-side refusal name must never be added to this CLI's own sentinel table"
    );
    assert!(
        text.contains("agent_call::Refusal"),
        "help must point at where the app-side refusal names are defined: {text}"
    );
}

/// Issue #1132's two-totals distinction is written on TWO surfaces — this
/// table (which `--help` and the `automations` MCP tool description derive
/// from) and `agent_read::RESOURCES` (which `agent schema` serves) — and
/// nothing tied them together, so one could drop a field the other still
/// explained (MEDIUM fix, review round 4). Both field names, on both
/// surfaces, in one assertion.
#[test]
fn both_automations_descriptions_name_both_totals() {
    let cli = VERB_TABLE
        .iter()
        .find(|v| v.name == "automations")
        .expect("the automations verb")
        .returns;
    let (_, schema) = crate::extension_bridge::agent_read::RESOURCES
        .iter()
        .find(|(name, _)| *name == "automations")
        .expect("the automations resource");
    for field in ["totalFound", "foundJobsTotal"] {
        assert!(
            cli.contains(field),
            "the --help/MCP description must name `{field}`: {cli}"
        );
        assert!(
            schema.contains(field),
            "`agent schema`'s own description must name `{field}`: {schema}"
        );
    }
}

/// P-r2-R2-F7 (round-2 review, issue #1180), reworded round 3 (P-r3-AC-R3-F2):
/// plain `agent call` (there is no `call-read` verb — that is the MCP tool
/// name) never sees an MCP tool description, so the `call` verb's own
/// `--help` text is one of the only two places such a caller can learn
/// `contact_profile_get`'s reply is projected — and it must name a form the
/// CLI dispatcher actually accepts (`parse_call`), not the MCP-only
/// `call-read`/`commands` surface a plain-CLI caller cannot invoke.
///
/// P-r1-AC-R4-F2 (round 4): the two assertions above are literal-vs-literal
/// — they'd stay green even if `contact_profile`/`contact_profile_get` were
/// renamed out from under the dispatcher, leaving `--help` naming an
/// invocation nothing accepts (the same drift class as P-r3-AC-R3-F2, just
/// re-encoded). This one instead resolves the named pair against the real
/// policy table, so a rename fails HERE instead of only making the help
/// text a silent lie.
#[test]
fn call_verb_help_names_the_contact_profile_get_projection() {
    let returns = VERB_TABLE
        .iter()
        .find(|v| v.name == "call")
        .expect("the call verb")
        .returns;
    assert!(returns.contains("agent call contact_profile:contact_profile_get"));
    assert!(returns.contains("photo"));
    assert!(
        !returns.contains("call-read contact_profile_get"),
        "must not name the MCP-only tool form as the CLI invocation"
    );
    assert!(
        policy::POLICY
            .iter()
            .any(|e| agent_call::split_path(e.path) == ("contact_profile", "contact_profile_get")),
        "the pair --help names must actually resolve to a real POLICY row"
    );
}

/// Round 2 fix (B3-r2-F3): `found-jobs`' cursor is bound to BOTH the
/// `autopilotId` scope AND the active filter arguments
/// (`found_jobs::found_jobs_cursor_issuer`) — the MCP schema
/// (`mcp::schemas`) already said so, but this table (`--help`) and
/// `agent_read::RESOURCES` (`agent schema`) only mentioned the id, so a
/// caller following either one had no way to know a cursor replayed under
/// changed filters would refuse. Same drift-guard shape as
/// `both_automations_descriptions_name_both_totals`, one hop over.
///
/// Asserts `text.contains("filter argument")`, not the bare word `"filter"`
/// (round 3 fix, B3-r3-F3 — both pre-fix strings already said "the filtered
/// row count"/"server-side filters", so the original `contains("filter")`
/// predicate was already true on the UNFIXED wording and passed on a revert;
/// `"filter argument"` only appears once the binding is actually stated).
#[test]
fn found_jobs_cursor_binding_is_named_on_both_surfaces() {
    let cli = VERB_TABLE
        .iter()
        .find(|v| v.name == "found-jobs")
        .expect("the found-jobs verb")
        .returns;
    let (_, schema) = crate::extension_bridge::agent_read::RESOURCES
        .iter()
        .find(|(name, _)| *name == "found-jobs")
        .expect("the found-jobs resource");
    for (surface, text) in [("--help", cli), ("agent schema", schema)] {
        assert!(
            text.contains("autopilotId") && text.contains("filter argument"),
            "{surface}'s found-jobs description must name both the autopilotId scope and the \
             filter-argument binding: {text}"
        );
    }
}

/// Round 3 fix (B3-r3-F4): `best-matches`' cursor is bound to `query` on the
/// `--help`/MCP-tool text (already stated) but NOT on `agent schema`'s own
/// `RES_BEST_MATCHES` row — the identical drift class
/// `found_jobs_cursor_binding_is_named_on_both_surfaces` catches one
/// resource over, now closed on its sibling.
#[test]
fn best_matches_cursor_binding_is_named_on_both_surfaces() {
    let cli = VERB_TABLE
        .iter()
        .find(|v| v.name == "best-matches")
        .expect("the best-matches verb")
        .returns;
    let (_, schema) = crate::extension_bridge::agent_read::RESOURCES
        .iter()
        .find(|(name, _)| *name == "best-matches")
        .expect("the best-matches resource");
    for (surface, text) in [("--help", cli), ("agent schema", schema)] {
        assert!(
            text.contains("query") && text.contains("cursor") && text.contains("issued it"),
            "{surface}'s best-matches description must name the cursor/query binding: {text}"
        );
    }
}

#[test]
fn is_help_request_recognizes_help_h_and_bare_help_verb() {
    assert!(is_help_request(&s(&["--help"])));
    assert!(is_help_request(&s(&["-h"])));
    assert!(is_help_request(&s(&["help"])));
    assert!(is_help_request(&s(&["--help", "job"])));
    assert!(!is_help_request(&s(&["job", "--help"])));
    assert!(!is_help_request(&s(&["best-matches"])));
    assert!(!is_help_request(&s(&[])));
}
