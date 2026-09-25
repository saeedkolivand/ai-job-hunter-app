//! The generated catalogue's TSDoc coverage: an upper bound on rows shipped without a
//! description, and the hand-written list of irreversible rows still missing one.

use super::super::*;

/// Upper bound on catalogued rows with an empty `description` (MEDIUM — CLI review round 1,
/// issue #1163's own headline ask: "every command row should carry a one-line description").
/// 58 of 162 carried none at the time this guard was added, including `applications_delete` —
/// backfilling TSDoc across ~160 IPC contract members is real, ongoing documentation debt this
/// generator cannot force by itself, so the cap is not zero. What it DOES catch: this count
/// growing — a newly added command shipping with no TSDoc on its contract member, silently
/// leaving the agent-CLI surface with less self-description than it had before. Lower this
/// constant (never raise it) as descriptions are backfilled.
const MAX_NO_DESCRIPTION_ROWS: usize = 48;

#[test]
fn catalogued_no_description_count_does_not_regress() {
    let no_description = super::super::super::catalogue::CATALOGUE
        .iter()
        .filter(|e| e.description.is_empty())
        .count();
    assert!(
        no_description <= MAX_NO_DESCRIPTION_ROWS,
        "{no_description} catalogued commands have no description (cap \
         {MAX_NO_DESCRIPTION_ROWS}) — a new command shipped with no TSDoc on its IPC contract \
         member; add one. If this failed after backfilling docs elsewhere and the count is now \
         LOWER, lower MAX_NO_DESCRIPTION_ROWS to match (never raise it)."
    );
}

/// Hand-written allowlist of `Effect::Irreversible` commands that STILL carry no catalogue
/// description (CLI review round 2 — MEDIUM: [`MAX_NO_DESCRIPTION_ROWS`] above is tier-blind, so
/// it could not stop a NEW destructive command shipping with no TSDoc as long as some unrelated
/// read command gained one elsewhere). Every name here is app-data-wipe/bulk-delete territory —
/// exactly where a caller needs the description most. Mirrors
/// `EXPECTED_UNRESOLVED_WRAPPER_ARGS`'s own "shrink only" discipline: remove an entry the moment
/// its TSDoc is backfilled; a PR that adds a NEW `Irreversible` row here without also adding a
/// description is the regression this guards against.
/// A1-r1-AC-6 MEDIUM backfilled TSDoc on 10 of the original 11 rows here (the smallest remaining
/// gap on #1163's stated Expected) — `privacy_clear_data` is the one deliberate holdout: it has
/// ZERO renderer call sites (see its own `POLICY` entry's doc), so there is no `invoke()` call
/// site for the generator to attach a description to, and inventing an unused TS contract member
/// just to carry TSDoc would be dead code. Reclassifying it `NotExposed` to sidestep this was
/// considered and rejected — that would gate a working destructive verb away, which this repo's
/// own rule set forbids regardless of review pressure.
const EXPECTED_IRREVERSIBLE_NO_DESCRIPTION: &[&str] = &["privacy_clear_data"];

#[test]
fn every_irreversible_row_without_a_description_is_on_the_hand_written_list() {
    let mut actual: Vec<&str> = POLICY
        .iter()
        .filter_map(|entry| {
            if !matches!(entry.effect, Effect::Irreversible(_)) {
                return None;
            }
            let (_, command) = crate::extension_bridge::agent_call::split_path(entry.path);
            let has_description = super::super::super::catalogue::CATALOGUE
                .iter()
                .any(|e| e.command == command && !e.description.is_empty());
            (!has_description).then_some(command)
        })
        .collect();
    actual.sort_unstable();
    let mut expected = EXPECTED_IRREVERSIBLE_NO_DESCRIPTION.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "the set of Irreversible commands with no catalogue description drifted from this \
         test's own hand-written list — if a NEW Irreversible command legitimately has no \
         description yet, add TSDoc to its IPC contract member instead of adding it here; if \
         one DROPPED OUT (now described), remove it from EXPECTED_IRREVERSIBLE_NO_DESCRIPTION"
    );
}
