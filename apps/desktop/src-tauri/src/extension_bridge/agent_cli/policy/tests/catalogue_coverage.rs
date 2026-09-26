//! Coverage of the GENERATED catalogue against the table: every dispatchable row is
//! catalogued or explicitly allowlisted, and those hand-written allowlists stay real.

use super::super::*;

// ── generated catalogue coverage (issues #1163, #1158, #1160) ────────────

/// A POLICY row with ZERO renderer `invoke()` references at all — this module's own doc names
/// exactly five such rows; two (`extract_resume`, `support_get_system_info`) are `NotExposed` and
/// so outside this coverage test's scope, and `dialog_open_files` (also `NotExposed`) has a real
/// call site the generator itself records in `catalogue::UNCATALOGUED`. The remaining three are the
/// ONLY Read/Reversible/Irreversible rows the generator has no signal for at all — hand-written,
/// never derived from `catalogue::UNCATALOGUED` (this repo's own standing lesson: a guard driven
/// off its own generated data cannot catch the generator silently dropping a row it used to emit).
/// `updater_status` (issue #1165's read-only counterpart to `updater_check`) has no renderer call
/// site at all — its ONLY consumer today is this agent surface's own `updater_install` confirm
/// proof, dispatched through `agent_call`'s `Webview::on_message` path, never the renderer's
/// `AppClient` — so it is uncatalogued by the same "zero renderer references" reasoning as
/// `boards_list`/`privacy_clear_data`, not an oversight.
const ALLOWLISTED_UNCATALOGUED: &[&str] = &["boards_list", "privacy_clear_data", "updater_status"];

/// Issue #1183 F4 — the ONE row of [`ALLOWLISTED_UNCATALOGUED`] whose `POLICY` `Effect` is
/// [`Effect::Irreversible`] (`privacy_clear_data`: disconnects every board and unconditionally
/// wipes the postings + interactions cache, per that row's own comment). This module's doc frames
/// every `ALLOWLISTED_UNCATALOGUED` entry purely as "zero renderer references", which is true but
/// incomplete for this one: it is also the single dispatchable row `agent_call::validate`'s
/// generated-catalogue key checking cannot reach at all (no `CatalogueArg` list to validate
/// against). Hand-written and checked by `allowlisted_uncatalogued_irreversible_entries_are_
/// exactly_the_irreversible_ones` below so a FUTURE destructive command added to
/// `ALLOWLISTED_UNCATALOGUED` — or `privacy_clear_data` being downgraded off `Irreversible` — must
/// be a deliberate, reviewed edit to this list, never a silent side effect of editing `POLICY` or
/// the allowlist alone.
const ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE: &[&str] = &["privacy_clear_data"];

/// Every dispatchable (`Read`/`Reversible`/`Irreversible`) POLICY row is reachable through
/// `agent_call::validate`'s dispatch-time key checking (issues #1163, #1158, #1160): either the
/// generated `catalogue::CATALOGUE` has an entry for it, the generator itself flagged it in
/// `catalogue::UNCATALOGUED` (a real call site it could not parse with confidence), or it is on
/// this file's own hand-written [`ALLOWLISTED_UNCATALOGUED`] (zero call site at all). A command
/// landing in none of the three is silently UNVALIDATED input with nobody accounting for why.
#[test]
fn every_dispatchable_row_is_catalogued_or_explicitly_accounted_for() {
    let mut uncovered = Vec::new();
    for entry in POLICY {
        if matches!(entry.effect, Effect::NotExposed(_)) {
            continue;
        }
        let command = entry.path.rsplit("::").next().unwrap_or(entry.path);
        let catalogued = super::super::super::catalogue::CATALOGUE
            .iter()
            .any(|e| e.command == command);
        let accounted_for = catalogued
            || super::super::super::catalogue::UNCATALOGUED.contains(&command)
            || ALLOWLISTED_UNCATALOGUED.contains(&command);
        if !accounted_for {
            uncovered.push(entry.path);
        }
    }
    assert!(
        uncovered.is_empty(),
        "these Read/Reversible/Irreversible POLICY rows are absent from CATALOGUE, \
         catalogue::UNCATALOGUED, AND this test's own ALLOWLISTED_UNCATALOGUED — a caller can \
         send them any input with none of it checked: {uncovered:?}"
    );
}

/// Allowlists are debt, not absolution (`docs/architecture-rules.md`'s own framing for R2/R7's
/// exception lists): each entry here must still be a real POLICY row, and must still be genuinely
/// absent from the generated catalogue — a future `invoke()` call site added for either command
/// makes the generator catalogue it, and this test then fails until the stale entry is removed.
#[test]
fn allowlisted_uncatalogued_entries_are_still_real_and_still_uncatalogued() {
    for command in ALLOWLISTED_UNCATALOGUED {
        assert!(
            POLICY
                .iter()
                .any(|e| e.path.rsplit("::").next() == Some(*command)),
            "{command} is not a real POLICY row — remove it from ALLOWLISTED_UNCATALOGUED"
        );
        assert!(
            !super::super::super::catalogue::CATALOGUE
                .iter()
                .any(|e| e.command == *command),
            "{command} is now catalogued — remove it from ALLOWLISTED_UNCATALOGUED"
        );
    }
}

/// Issue #1183 F4: every [`ALLOWLISTED_UNCATALOGUED`] row whose `POLICY` `Effect` is
/// [`Effect::Irreversible`] must appear in [`ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE`], AND every
/// entry of that second list must actually be `Irreversible` — both directions, so the list can
/// neither miss a real destructive uncatalogued row nor carry a stale one. Mutating either list
/// alone, or reclassifying `privacy_clear_data`'s `POLICY` effect, fails this test.
#[test]
fn allowlisted_uncatalogued_irreversible_entries_are_exactly_the_irreversible_ones() {
    for command in ALLOWLISTED_UNCATALOGUED {
        let entry = POLICY
            .iter()
            .find(|e| e.path.rsplit("::").next() == Some(*command))
            .expect("pinned by allowlisted_uncatalogued_entries_are_still_real_and_still_uncatalogued above");
        let is_irreversible = matches!(entry.effect, Effect::Irreversible(_));
        assert_eq!(
            is_irreversible,
            ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE.contains(command),
            "{command} is {}Irreversible in POLICY but {}listed in \
             ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE — keep the two in sync",
            if is_irreversible { "" } else { "NOT " },
            if ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE.contains(command) {
                ""
            } else {
                "not "
            }
        );
    }
    for command in ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE {
        assert!(
            ALLOWLISTED_UNCATALOGUED.contains(command),
            "{command} is in ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE but not in \
             ALLOWLISTED_UNCATALOGUED itself"
        );
    }
}

/// Hand-written mirror of `catalogue::UNCATALOGUED` (MEDIUM — CLI review round 1). The coverage
/// test above pairs `catalogued` with the GENERATED `UNCATALOGUED` array and nothing bounds it —
/// this repo's own standing lesson (see [`ALLOWLISTED_UNCATALOGUED`]'s doc) applies to
/// `UNCATALOGUED` itself, not only to the zero-call-site allowlist: a generator regression that
/// silently reclassified N real commands as uncatalogued would leave
/// `every_dispatchable_row_is_catalogued_or_explicitly_accounted_for` green while un-validating
/// all of them, because that test treats `UNCATALOGUED` membership as accounted-for by
/// construction. Pinned against a literal, exactly like `ALLOWLISTED_UNCATALOGUED` is.
const EXPECTED_UNCATALOGUED: &[&str] = &["dialog_open_files"];

#[test]
fn uncatalogued_matches_the_hand_written_list() {
    assert_eq!(
        super::super::super::catalogue::UNCATALOGUED,
        EXPECTED_UNCATALOGUED,
        "catalogue::UNCATALOGUED drifted from this test's own hand-written list — if a NEW \
         command legitimately can't be parsed by the generator (a computed key, a spread, a \
         non-literal command name, or a non-object invoke() argument), add it here \
         deliberately; if a command DROPPED OUT of UNCATALOGUED, nothing to do beyond updating \
         this list. A command that appears here without ever having been added on purpose is \
         the regression this test exists to catch."
    );
}
