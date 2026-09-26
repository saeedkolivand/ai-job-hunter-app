//! `<namespace>:<command>` ⇄ [`super::super::agent_cli::policy::PolicyEntry`] lookup, and the two
//! catalogue-validation re-exports MCP's local refusal path needs — split out of `agent_call.rs`
//! under the R8 LOC cap.

use serde_json::Value;

use super::super::agent_cli::policy::{Effect, PolicyEntry, ProofSource, POLICY};
use super::{proof, validate, Refusal};

/// Split a [`PolicyEntry::path`] (e.g. `"commands::jobs::jobs_list"`, always `module::fn`) into
/// `(namespace, command)`. `command` is the bare trailing segment — the wire `cmd` Tauri actually
/// registers, never the qualified path; `namespace` is the segment immediately before it. Uniform
/// across every row's shape with no per-module special-casing — the SAME derivation parses a CLI
/// token's expected shape and looks a row up.
///
/// `pub(super)` — `agent_cli::mcp` (a sibling, not a descendant) needs the identical split to
/// route a `call-*` tool locally against its own bundled `POLICY` copy, and to build `commands`'
/// rows, never a second hand-typed `rsplit("::")`.
pub(in crate::extension_bridge) fn split_path(path: &str) -> (&str, &str) {
    let mut segments = path.rsplit("::");
    let command = segments.next().unwrap_or(path);
    let namespace = segments.next().unwrap_or("");
    (namespace, command)
}

/// The one [`PolicyEntry`] whose derived `(namespace, command)` matches
/// EXACTLY — never a fuzzy/partial match (a typo'd namespace on an
/// otherwise-real command name refuses rather than silently dispatching:
/// `command` alone already uniquely identifies a row, since
/// `generate_handler!` requires globally-unique command names, so a
/// namespace mismatch can only mean the caller typed the wrong one).
pub(super) fn find_policy(namespace: &str, command: &str) -> Option<&'static PolicyEntry> {
    POLICY
        .iter()
        .find(|entry| split_path(entry.path) == (namespace, command))
}

/// The real namespace for `command`, when EXACTLY ONE [`POLICY`] row's own bare command name
/// matches it — never a fuzzy match on a mistyped COMMAND name: an EXACT string match on the
/// trailing segment, same equality [`find_policy`] uses, not a distance/prefix heuristic. `None`
/// when zero rows match, or — defensively, since command names are globally unique — more than
/// one does; guessing between two would be exactly the "typo to a destructive neighbour" path this
/// surface never takes. `pub(super)` — the MCP server's own LOCAL `unknown_command` refusal needs
/// the identical suggestion, never a second hand-typed scan of [`POLICY`].
pub(in crate::extension_bridge) fn namespace_suggestion(command: &str) -> Option<&'static str> {
    let mut matches = POLICY
        .iter()
        .filter(|entry| split_path(entry.path).1 == command)
        .map(|entry| split_path(entry.path).0);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

/// [`Refusal::UnknownCommand`]'s own detail text — built from [`namespace_suggestion`]'s output,
/// `pub(super)` so [`super::agent_cli::mcp::local_call_refusal`] can build the IDENTICAL wording
/// for its own local (never-dispatched, no round trip) refusal rather than a second hand-typed
/// copy that could drift.
pub(in crate::extension_bridge) fn unknown_command_detail(suggestion: Option<&str>) -> String {
    match suggestion {
        Some(namespace) => format!(
            "no policy row matches this <namespace>:<command> — this command name IS real, but \
             registered under namespace `{namespace}`; run `agent schema` or the MCP `commands` \
             tool to enumerate targets, or see policy.rs for the full table"
        ),
        None => "no policy row matches this <namespace>:<command> — run `agent schema` or the \
                  MCP `commands` tool to enumerate targets, or see policy.rs for the full table"
            .to_string(),
    }
}

/// Re-exports of [`proof::proof_field`]/[`proof::proof_kind`] for `commands`' `proofField`/
/// `proofKind` rows (issues #1160, #1160 round 2), without widening `proof`'s own module privacy.
pub(in crate::extension_bridge) fn proof_field_for(source: ProofSource) -> Option<String> {
    proof::proof_field(source)
}

pub(in crate::extension_bridge) fn proof_kind_for(source: ProofSource) -> &'static str {
    proof::proof_kind(source)
}

/// Re-export of [`validate::check_input`] + [`validate::check_no_empty_required_wrapper`] for
/// MCP's `local_call_refusal` (A1-r1-SEC-1 HIGH, widened round 2): `local_call_refusal` used to
/// forward every uncaught body straight to the PEER app process for catalogue validation — a
/// SEPARATE, possibly OLDER process, so a mis-keyed `call-*` body could dispatch silently on an
/// older running app even though `initialize`'s own instructions promise `invalid_input` is
/// refused before dispatch. Both checks run here, in [`dispatch_plan::plan`]'s own order, so the
/// local mirror matches the app-side gate exactly. Returns the detail string (never the full
/// [`Refusal`], to keep `validate`'s enum-construction private to this module).
pub(in crate::extension_bridge) fn invalid_input_detail(
    command: &str,
    effect: Effect,
    input: &Value,
) -> Option<String> {
    fn detail_of(result: Result<(), Refusal>) -> Option<String> {
        match result {
            Ok(()) => None,
            Err(Refusal::InvalidInput(detail)) => Some(detail),
            Err(_) => None, // both checked fns' only Err variant is InvalidInput
        }
    }
    detail_of(validate::check_input(command, input)).or_else(|| {
        detail_of(validate::check_no_empty_required_wrapper(
            command, effect, input,
        ))
    })
}
