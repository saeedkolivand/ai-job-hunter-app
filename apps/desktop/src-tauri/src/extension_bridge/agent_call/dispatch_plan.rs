//! The pure "should `dispatch` attempt this at all" decision — split out of `agent_call.rs` (R8's
//! hard LOC cap, the same reason `proof.rs`/`validate.rs`/`reshape.rs` are siblings rather than
//! inline). Re-exported at `agent_call`'s own top level (`pub(super) use dispatch_plan::{...}`),
//! so every existing call site (`agent_call::gate`, `super::gate` from `agent_call::tests`) is
//! unchanged.

use serde_json::Value;

use super::super::agent_cli::policy::{Effect, PolicyEntry, ProofSource};
use super::{proof, validate, Refusal};

/// What [`gate`] clears `dispatch` to do for one `(effect, confirm)` pair —
/// carries whatever the cleared branch needs, so nothing downstream
/// re-derives a fact `gate` already established. `Confirmed`'s `confirm` is
/// a plain `&str`, not an `Option` — reaching that variant at all is already
/// proof one was supplied, so there is nothing left to unwrap.
pub(in crate::extension_bridge) enum Dispatch<'a> {
    /// `Read`/`Reversible` — invoke directly, no ceremony.
    Direct,
    /// `Irreversible`, `confirm` already known to be present. Carries the
    /// row's own [`ProofSource`] alongside it so `dispatch` never re-matches
    /// `entry.effect` a second time to recover it.
    Confirmed {
        source: ProofSource,
        confirm: &'a str,
    },
}

/// Pure gate: does `effect` permit `dispatch` to ATTEMPT a real command
/// invocation at all, given whether a `confirm` value was supplied — never
/// mind whether that attempt then succeeds. Replaces a former
/// boolean-returning `dispatchable`: a `bool` only told the caller "yes",
/// forcing `dispatch` to re-match `entry.effect` a second time to recover
/// the `ProofSource` AND `.expect()` a `confirm` this fn had already proved
/// `Some` — an `expect` on an externally reachable `agent.call` path, safe
/// only because of a separate call to this same gate rather than because
/// the type ruled out the `None` case. Returning [`Dispatch`] instead means
/// the confirmed branch carries its `&str` and `ProofSource` BY
/// CONSTRUCTION, so there is nothing left downstream to re-derive or
/// unwrap — a future refactor that changed this gate's logic could no
/// longer silently leave a stale, now-unsound `expect` behind it.
///
/// [`plan`] below calls this as its own LAST decision (never a
/// parallel/shadow copy of the same logic), so `extension_bridge::test`'s
/// exhaustive walk over every real `POLICY` row
/// (`agent_call_gate_matches_every_policy_rows_declared_effect`) proves
/// something about THIS production routing, not a second implementation
/// that could silently drift from it. `pub(in crate::extension_bridge)` (re-exported at
/// `agent_call`'s own top level) — reachable from `extension_bridge::test`, a sibling of
/// `agent_call`, for exactly that test; [`Dispatch`] shares that visibility for the same reason.
pub(in crate::extension_bridge) fn gate(
    effect: Effect,
    confirm: Option<&str>,
) -> Result<Dispatch<'_>, Refusal> {
    match effect {
        Effect::NotExposed(reason) => Err(Refusal::NotExposed(reason)),
        Effect::Read | Effect::Reversible => Ok(Dispatch::Direct),
        Effect::Irreversible(source) => match confirm {
            Some(confirm) => Ok(Dispatch::Confirmed { source, confirm }),
            None => Err(Refusal::ConfirmationRequired(proof::hint(source))),
        },
    }
}

/// Pure ordering decision `dispatch` calls as its own FIRST step, extracted so a test can drive it
/// directly rather than only trust it from reading `dispatch`'s own source (CLI review round 2 —
/// MEDIUM: a swapped-lines regression in `dispatch` used to leave every prior "ordering" test
/// green, since it only called `check_input`/`gate` separately). Order: `NotExposed` refuses
/// FIRST — a command that can never dispatch must refuse with its own cause, not a validation
/// error about arguments that were never going to matter (14/23 `NotExposed` rows carry declared
/// args) — THEN catalogue validation (#1160's ordering fix: a missing required key on an
/// `Irreversible` row must refuse here, never surface as `ConfirmationRequired` and die on the
/// deserializer after an approved confirm) — THEN the confirm ceremony itself.
pub(in crate::extension_bridge) fn plan<'a>(
    entry: &'a PolicyEntry,
    command: &str,
    input: &Value,
    confirm: Option<&'a str>,
) -> Result<Dispatch<'a>, Refusal> {
    if let Effect::NotExposed(reason) = entry.effect {
        return Err(Refusal::NotExposed(reason));
    }
    validate::check_input(command, input)?;
    // A1-r1-SEC-2 MEDIUM — same ordering slot as `check_input` above (before `gate`'s confirm
    // ceremony): an empty required wrapper on a mutation is refused here, never surfaced as
    // `ConfirmationRequired` and then dispatched as a no-op "success".
    validate::check_no_empty_required_wrapper(command, entry.effect, input)?;
    gate(entry.effect, confirm)
}
