//! Rows pinned by their PROOF SHAPE or their effect class rather than by name alone:
//! the `notifications` mark rows, both `updater` rows, and the two `commands::help` rows.

use super::super::*;

/// Round 5 (`B1-r1-ACLI-R5-3`): direct pin for both rows issue #1164 reclassified — the aggregate
/// row count (`policy_table_row_count_is_pinned`) and the Irreversible tally
/// (`every_proof_source_read_command_is_a_read_row`'s trailing `checked == 35`) are BOTH blind to
/// a revert of one row paired with an unrelated +1/-1 elsewhere in the table: `checked` stays 35
/// either way. `notifications_mark_read` had no direct pin anywhere (it appeared in this file only
/// inside a comment); `notifications_mark_all_read` was pinned only for its comment's WORDING
/// (`mark_all_read_proof_comment_calls_the_count_a_superset_not_exact` above), never its `Effect`
/// or `ProofSource` shape. This asserts both rows' actual classification and proof shape directly.
#[test]
fn notifications_mark_rows_stay_irreversible_with_their_proof_shapes() {
    let mark_read = POLICY
        .iter()
        .find(|e| e.path == "commands::notifications::notifications_mark_read")
        .expect("commands::notifications::notifications_mark_read is a real POLICY row");
    let Effect::Irreversible(ProofSource::ListMatch {
        read_command,
        id_field,
        match_field,
        value_field,
    }) = mark_read.effect
    else {
        panic!(
            "notifications_mark_read must be Irreversible(ProofSource::ListMatch), got {:?}",
            mark_read.effect
        );
    };
    assert_eq!(read_command, "notifications_list");
    assert_eq!(id_field, &["id"]);
    assert_eq!(match_field, "id");
    assert_eq!(value_field, "title");

    let mark_all_read = POLICY
        .iter()
        .find(|e| e.path == "commands::notifications::notifications_mark_all_read")
        .expect("commands::notifications::notifications_mark_all_read is a real POLICY row");
    let Effect::Irreversible(ProofSource::Count { read_command }) = mark_all_read.effect else {
        panic!(
            "notifications_mark_all_read must be Irreversible(ProofSource::Count), got {:?}",
            mark_all_read.effect
        );
    };
    assert_eq!(read_command, "notifications_list");
}

/// Round 5 (`B1-r1-ACLI-R5-1`): `updater_install`'s proof must read the PENDING version off
/// `updater_status` — the read-only counterpart added when `updater_check` was reverted from
/// `Read` back to `Reversible` (it writes `UpdaterState`/emits an event, so it cannot be the proof
/// source for another `Irreversible` row: `every_proof_source_read_command_is_a_read_row` requires
/// the target to be `Effect::Read`) — rather than `system_get_version` (the CURRENTLY RUNNING
/// version — a vacuous proof, since it never changes as a result of confirming).
#[test]
fn updater_install_proof_reads_the_pending_version_off_updater_status() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_install")
        .expect("updater::updater_install is a real POLICY row");
    let Effect::Irreversible(ProofSource::Scalar { read_command, path }) = entry.effect else {
        panic!(
            "updater_install must stay Irreversible(ProofSource::Scalar), got {:?}",
            entry.effect
        );
    };
    assert_eq!(
        read_command, "updater_status",
        "updater_install's proof must read updater_status, not system_get_version's vacuous \
         running-version echo, and not updater_check (Reversible, not a valid Read proof source)"
    );
    assert_eq!(
        path,
        &["version"],
        "updater_install's proof must walk to updater_status's `version` field"
    );
}

/// Round 5 (`B1-r1-ACLI-R5-1`): `updater_check` must stay `Effect::Reversible` — it writes
/// `UpdaterState` and emits `updater:status`, and reclassifying it `Read` (issue #1165) forced
/// `call-read`'s `readOnlyHint` to `false` for every one of this table's 63 other `Read` rows,
/// since the hint is a per-TOOL promise, not per-row (see `mcp::tests::
/// call_read_annotations_claim_read_only`). `updater::updater_status` is the read-only
/// alternative this row's proof now points at.
#[test]
fn updater_check_stays_reversible_not_read() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_check")
        .expect("updater::updater_check is a real POLICY row");
    assert_eq!(
        entry.effect,
        Effect::Reversible,
        "updater_check must stay Reversible, not Read — got {:?}",
        entry.effect
    );
    let status = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_status")
        .expect("updater::updater_status is a real POLICY row");
    assert_eq!(
        status.effect,
        Effect::Read,
        "updater_status must be the genuinely read-only alternative — got {:?}",
        status.effect
    );
}

/// Issue #1169: `commands::help::help_search` must actually BE the `NotExposed` row the doc
/// guard below and `every_proof_source_read_command_is_a_read_row`'s comment both describe — the
/// prior version of this file asserted the PROSE said so without ever reading `POLICY` itself
/// (round-3 finding `B1-r3-ACLI-2`), so reverting the row back to `Irreversible`/`Read` left every
/// other test in this file green.
#[test]
fn help_search_stays_not_exposed() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::help::help_search")
        .expect("commands::help::help_search is a real POLICY row");
    assert!(
        matches!(entry.effect, Effect::NotExposed(_)),
        "commands::help::help_search must stay NotExposed (issue #1169) — got {:?}",
        entry.effect
    );
}

/// Issue #1164 round 2 (`B1-r2-B2-r2-ACLI-6`): `commands::help::help_search` is `NotExposed`
/// (issue #1169), but its own module doc claimed unqualified reachability from the agent CLI /
/// extension bridge in four spans — falsifying the RATIONALE those spans give for re-checking
/// every Zod cap in Rust. Every span that mentions the agent CLI or extension bridge reaching
/// `help_search` must also name its current `NotExposed` status (issue #1169), so a future
/// reclassification of the POLICY row is the only thing that can make the doc true again without
/// a human re-reading it — enforced together with `help_search_stays_not_exposed` above, which
/// pins the ROW itself (round-3 finding `B1-r3-ACLI-2`: this test alone never read `POLICY`).
#[test]
fn help_module_doc_reachability_claims_stay_paired_with_its_not_exposed_status() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::help::help_search")
        .expect("commands::help::help_search is a real POLICY row");
    assert!(
        matches!(entry.effect, Effect::NotExposed(_)),
        "this doc-reachability guard only makes sense while the row is NotExposed — got {:?}",
        entry.effect
    );
    const HELP_RS: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/help.rs"));
    let lines: Vec<&str> = HELP_RS.lines().collect();
    let mut checked = 0usize;
    for (n, line) in lines.iter().enumerate() {
        if !(line.contains("agent CLI") || line.contains("agent-CLI")) {
            continue; // not a span naming the agent CLI at all
        }
        // A window of comment lines around the mention, so "reach"/"NotExposed"/"#1169" can
        // be on a neighbouring line within the same prose block, not literally the same line.
        let start = n.saturating_sub(6);
        let end = (n + 6).min(lines.len());
        let window = lines[start..end].join("\n");
        if !window.contains("reach") {
            continue; // an "agent CLI" mention unrelated to reachability
        }
        checked += 1;
        assert!(
            window.contains("NotExposed") && window.contains("#1169"),
            "a reachability claim at help.rs line {} must name help_search's current \
             NotExposed status (issue #1169): {window}",
            n + 1
        );
    }
    assert!(
        checked >= 4,
        "expected at least the 4 reachability spans issue #1164 round 2 flagged, found \
         {checked}"
    );
}
