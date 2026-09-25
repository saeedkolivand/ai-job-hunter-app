//! Membership + shape of `POLICY` itself: the pinned row count, set-equality with
//! `lib.rs`'s `generate_handler!`, no duplicate paths, a real reason on every
//! `NotExposed` row, and the hand-pinned `match_resume`/regenerate-token rows — plus
//! the `lib.rs` extraction the two set-comparison tests share.

use super::super::*;
use std::collections::HashSet;

/// The `lib.rs` source, embedded at compile time — the SAME text
/// `cargo build` feeds to `tauri::generate_handler!`, so extraction from
/// it can never drift from what is actually wired up (mirrors
/// `commands::cli_agents::tests`' `include_str!` of the capability
/// allowlist for the identical reason).
const LIB_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));

/// Extract the fully-qualified command paths registered inside
/// `tauri::generate_handler![...]`, in source order. Comment-only and
/// blank lines inside the list are skipped; every other line is
/// trimmed of its trailing comma. Panics (test-only) if the markers
/// this depends on ever move — that failure itself is the signal this
/// extraction needs updating, not a silent empty result.
///
/// The closing `]` is located on the first NON-comment line that
/// carries one (LOW fix — security review): the naive `rest.find(']')`
/// this used to run against the RAW text would truncate the extraction
/// early if a `//` comment between the marker and the real terminator
/// ever contained a literal `]` — silent, since a truncated-but-still-
/// well-formed list still passes both anti-drift tests below with a
/// SMALLER `POLICY` and a smaller `registered` set, never surfacing the
/// mismatch. Comment lines are skipped when searching for `]`, not when
/// slicing — every real command line up to the true terminator is kept.
fn registered_command_paths() -> Vec<&'static str> {
    const START_MARKER: &str = "tauri::generate_handler![";
    let start = LIB_RS
        .find(START_MARKER)
        .expect("tauri::generate_handler![ marker present in lib.rs")
        + START_MARKER.len();
    let rest: &'static str = &LIB_RS[start..];

    let mut end = None;
    let mut offset = 0usize;
    for line in rest.lines() {
        let is_comment = line.trim_start().starts_with("//");
        if !is_comment {
            if let Some(pos) = line.find(']') {
                end = Some(offset + pos);
                break;
            }
        }
        // `lines()` strips the `\n` each line ended with — add it back
        // so `offset` stays a correct byte position into `rest`.
        offset += line.len() + 1;
    }
    let end = end.expect("generate_handler! list has a closing ] on a non-comment line in lib.rs");

    rest[..end]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .map(|line| line.trim_end_matches(','))
        .collect()
}

/// Hand-written literal — deliberately NOT derived from `POLICY.len()`
/// or from `registered_command_paths()`. A test that only loops over
/// the table it guards covers additions only (this repo's own standing
/// lesson: `feedback_a_guard_driven_off_its_own_data_cannot_catch_a_deletion`)
/// — this fails independently of either source's own content.
// Named without the count itself (issue #1170, MEDIUM — a prior name
// baked in "167", and two module docs elsewhere cited that name AS the
// authoritative row count, so both silently restated a stale number the
// moment this row grew to 168): the count lives ONLY in the `assert_eq!`
// below, never in a name or a doc pointer to this test.
#[test]
fn policy_table_row_count_is_pinned() {
    // 167 + 1 (round 5, `B1-r1-ACLI-R5-1`): `updater::updater_status`, the
    // read-only counterpart added when `updater_check` was reverted from
    // `Read` back to `Reversible`.
    assert_eq!(POLICY.len(), 168);
}

/// ADR-038 §1's core invariant: the policy table and `generate_handler!`
/// agree EXACTLY — no command reachable from the registry without a
/// classified row, and no stale/typo'd row claiming a command that
/// isn't actually registered. Both directions, with the offending
/// command named in the failure message.
#[test]
fn policy_table_matches_generate_handler_exactly() {
    let registered: HashSet<&str> = registered_command_paths().into_iter().collect();
    let policy: HashSet<&str> = POLICY.iter().map(|e| e.path).collect();

    let missing: Vec<&&str> = registered.difference(&policy).collect();
    assert!(
        missing.is_empty(),
        "commands registered in generate_handler! but missing a POLICY row \
         (unclassified — must be added): {missing:?}"
    );

    let extra: Vec<&&str> = policy.difference(&registered).collect();
    assert!(
        extra.is_empty(),
        "POLICY rows with no matching generate_handler! registration \
         (stale, or the path is typo'd): {extra:?}"
    );
}

/// Guards the set-equality test above against a duplicate masking a
/// missing row: two identical `path`s would satisfy both `difference`
/// checks while a third, genuinely-unclassified command silently has
/// no row at all.
#[test]
fn policy_table_has_no_duplicate_paths() {
    let mut seen = HashSet::new();
    for entry in POLICY {
        assert!(
            seen.insert(entry.path),
            "duplicate POLICY row for {}",
            entry.path
        );
    }
}

/// Every `NotExposed` row carries a real, specific reason — never a
/// bare placeholder like "unclear" or "todo" (rule 3 of the
/// classification pass this table was built under).
#[test]
fn not_exposed_rows_carry_a_real_reason() {
    for entry in POLICY {
        if let Effect::NotExposed(reason) = entry.effect {
            assert!(
                reason.trim().len() > 15,
                "{} is NotExposed with a too-short/placeholder reason: {reason:?}",
                entry.path
            );
        }
    }
}

/// LOW fix (pre-PR gate, round 3): a prior edit shortened this row's inline reason to one
/// over-broad claim — "every value extension_bridge_status could offer is one this exact
/// connection already had to possess" — which is true of `port`/`token` but NOT of `connected`,
/// a boolean that reads `true` only because THIS socket's own successful authentication is what
/// increments the counter it reports. The longer row comment above always had both clauses; the
/// short inline reason (what a caller/refusal actually sees) must too.
#[test]
fn extension_bridge_regenerate_token_reason_explains_why_connected_is_vacuous_too() {
    let entry = POLICY
        .iter()
        .find(|e| e.path.ends_with("extension_bridge_regenerate_token"))
        .expect("the row must still exist");
    let Effect::NotExposed(reason) = entry.effect else {
        panic!("must stay NotExposed");
    };
    assert!(
        reason.contains("connected"),
        "must name `connected` specifically, not just `port`/`token`: {reason}"
    );
    assert!(
        reason.contains("THIS socket") || reason.contains("this socket"),
        "must explain WHY connected is vacuous (self-authentication increments it), not just \
         assert it: {reason}"
    );
}

/// HIGH fix (security review round 2): `match_resume`/`match_resume_text`
/// reach a paid embedding provider (`score_one` → `embed_charged`) with
/// `budget: None` when `semanticScoringEnabled: true` — the SAME
/// uncapped-spend shape that forced `ai_embed` `NotExposed` before ITS
/// gate landed. Hand-written (not looped, mirroring
/// `policy_table_row_count_is_pinned`'s own discipline): a revert of
/// either row back to `Reversible` (freely dispatchable, no confirm, no
/// cap) would not be caught by any OTHER test in this file — the
/// Irreversible-row count is untouched by a `Reversible` change, and
/// `not_exposed_rows_carry_a_real_reason` only checks rows that ARE
/// `NotExposed`, never that a specific row remains one.
#[test]
fn match_resume_and_match_resume_text_stay_not_exposed_until_a_real_charge_lands() {
    for path in [
        "commands::match_resume::match_resume",
        "commands::match_resume::match_resume_text",
    ] {
        let entry = POLICY
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
        assert!(
            matches!(entry.effect, Effect::NotExposed(_)),
            "{path} must stay NotExposed (uncharged paid-embedding path) until a real \
             charge_provider_daily gate lands on it — got {:?}",
            entry.effect
        );
    }
}

/// `registered_command_paths` itself must find every command `lib.rs`
/// actually registers — sanity-checks the extraction against a handful
/// of paths spanning the start, middle and end of the list, so a
/// regression in the marker/parsing logic (not just a POLICY drift)
/// is caught here rather than surfacing as a confusing mismatch above.
#[test]
fn extraction_finds_known_paths_at_each_end_of_the_list() {
    let found = registered_command_paths();
    assert_eq!(
        found.first().copied(),
        Some("commands::cli_agents::cli_agents_status"),
        "extraction must find the FIRST registered command"
    );
    assert_eq!(
        found.last().copied(),
        Some("updater::updater_changelog"),
        "extraction must find the LAST registered command"
    );
    assert!(found.contains(&"commands::privacy::privacy_reset_app"));
    assert_eq!(found.len(), 168);
}
