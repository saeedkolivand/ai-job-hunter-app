//! ADR-038 §3/§4 -- the exhaustive counterpart to `dispatch_classify`'s 4 hand-picked `gate`
//! cases -- redistributed from the crate-level `test.rs` (R8 relief).

use super::super::super::agent_cli::policy::{Effect, POLICY};
use super::super::*;

/// Walks every ONE of the 168 real `POLICY` rows (not a representative sample) and asserts
/// `dispatch`'s own gate (`gate` -- called directly by `dispatch`, never a parallel copy) agrees
/// with what that row's declared `Effect` promises: `Read`/`Reversible` dispatchable
/// unconditionally, `Irreversible` dispatchable ONLY with a confirm, `NotExposed` never
/// dispatchable. This is what stops a future phase widening the gate for one class (e.g.
/// loosening `Reversible`) from silently widening it for another -- a test that only checked 2-3
/// representative rows could pass while missing a class the sample didn't happen to cover.
///
/// Mutation-checked by hand, and the negative result matters as much as the positive one:
/// flipping a single row's OWN `Effect` in `policy.rs` (e.g. `documents_remove` from
/// `Irreversible` to `Read`) does NOT fail this test -- the assertions below are keyed off
/// `entry.effect` itself, so a mis-classified row just moves to a different (still
/// self-consistent) branch. That is a real limit of what a per-row walk can prove: it is not a
/// check that any INDIVIDUAL classification is correct (the row's own comment + review is what
/// defends that). What DOES fail this test -- verified by hand, then reverted -- is mutating
/// `gate`'s OWN match arms: changing `Effect::Irreversible(source) => match confirm { .. }` to
/// always return `Ok(Dispatch::Confirmed { .. })` regardless of `confirm` (the exact "silently
/// widened the gate for one class" shape this guards against) fails on the FIRST Irreversible row
/// this walks (`system_open_external`), because that row's `Effect` still correctly says
/// `Irreversible` while the (mutated) gate now claims it is dispatchable with no confirm. Walking
/// all 168 real rows -- not 2-3 representative ones -- is what makes that failure immediate
/// rather than dependent on which rows a smaller hand-picked sample happened to include.
#[test]
fn agent_call_gate_matches_every_policy_rows_declared_effect() {
    let dispatchable = |effect: Effect, confirm: Option<&str>| {
        matches!(
            gate(effect, confirm),
            Ok(Dispatch::Direct | Dispatch::Confirmed { .. })
        )
    };

    let mut checked = 0usize;
    for entry in POLICY {
        checked += 1;
        match entry.effect {
            Effect::Read | Effect::Reversible => {
                assert!(
                    dispatchable(entry.effect, None),
                    "{} is Read/Reversible — must be dispatchable with no confirm",
                    entry.path
                );
                assert!(
                    dispatchable(entry.effect, Some("x")),
                    "{} is Read/Reversible — must stay dispatchable even WITH a confirm",
                    entry.path
                );
            }
            Effect::Irreversible(_) => {
                assert!(
                    !dispatchable(entry.effect, None),
                    "{} is Irreversible — must NOT be dispatchable without --confirm",
                    entry.path
                );
                assert!(
                    dispatchable(entry.effect, Some("x")),
                    "{} is Irreversible — must be dispatchable once --confirm is supplied",
                    entry.path
                );
            }
            Effect::NotExposed(_) => {
                assert!(
                    !dispatchable(entry.effect, None),
                    "{} is NotExposed — must never be dispatchable",
                    entry.path
                );
                assert!(
                    !dispatchable(entry.effect, Some("x")),
                    "{} is NotExposed — a confirm value must not change that",
                    entry.path
                );
            }
        }
    }
    // Hand-written literal (not derived from `POLICY.len()` itself -- same "pair a loop with a
    // literal" discipline `policy.rs`'s own tests use): every one of the 168 rows must actually
    // have been walked. 167 + 1 (round 5, `B1-r1-ACLI-R5-1`): `updater::updater_status`, the
    // read-only counterpart added when `updater_check` was reverted from `Read` back to
    // `Reversible`.
    assert_eq!(checked, 168);
}
