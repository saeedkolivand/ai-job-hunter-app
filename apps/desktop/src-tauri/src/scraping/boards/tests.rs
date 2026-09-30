//! Registry-level tests for `boards/mod.rs`: TS/Rust board-id parity and the
//! all-remote-board declaration list.

use crate::ipc_contracts::board_ids::BOARD_IDS;

use super::is_all_remote_board;

/// The renderer carries its own copy of this registry's ids
/// (`packages/shared/src/schemas/index.ts::BOARD_IDS`, emitted to Rust by
/// `pnpm gen:ipc`), because the TS side needs a `BoardId` type and a Zod
/// enum. Two hand-maintained lists of the same vocabulary is exactly the
/// shape that drifts, and it had: `jobicy` was registered here, listed in
/// the catalog and given en/de labels, while the shared list never learned
/// about it. Nothing on either side could see the gap, because nothing
/// compared them.
///
/// Asserted in BOTH directions on purpose. One direction catches a board
/// added to the registry and forgotten in TS (the `jobicy` case); the other
/// catches a board removed here while the TS list — and therefore the
/// `BoardId` type the renderer programs against — still promises it.
#[test]
fn the_shared_board_id_list_and_this_registry_name_the_same_boards() {
    let registry: Vec<&str> = super::SCRAPERS.iter().map(|s| s.id()).collect();

    let missing_from_shared: Vec<&str> = registry
        .iter()
        .copied()
        .filter(|id| !BOARD_IDS.contains(id))
        .collect();
    assert!(
        missing_from_shared.is_empty(),
        "registered here but absent from packages/shared BOARD_IDS: {missing_from_shared:?} \
         — add them there and run `pnpm gen:ipc`"
    );

    let missing_from_registry: Vec<&str> = BOARD_IDS
        .iter()
        .copied()
        .filter(|id| !registry.contains(id))
        .collect();
    assert!(
        missing_from_registry.is_empty(),
        "promised by packages/shared BOARD_IDS but not registered in SCRAPERS: \
         {missing_from_registry:?}"
    );
}

/// A vacuity guard for the test above. Both of its assertions are "this
/// difference is empty", which is trivially true of two empty lists — so if
/// a refactor ever left `SCRAPERS` or the generated const empty, the parity
/// test would pass while checking nothing at all.
#[test]
fn both_lists_are_actually_populated() {
    assert!(super::SCRAPERS.len() >= 20, "the registry lost boards");
    assert_eq!(
        BOARD_IDS.len(),
        super::SCRAPERS.len(),
        "the two lists must have the same length, not merely overlap"
    );
}

/// The exact four all-remote feeds (round-4 fix T1) — hand-written, not
/// derived by scanning `SCRAPERS` for the flag, so a board silently
/// losing its override still fails this list.
#[test]
fn every_all_remote_board_is_declared() {
    for id in ["wwr", "remoteok", "remotive", "jobicy"] {
        assert!(is_all_remote_board(id), "{id} must be declared all-remote");
    }
}

#[test]
fn a_registered_non_remote_board_and_an_unknown_id_are_both_false() {
    assert!(!is_all_remote_board("linkedin"));
    assert!(!is_all_remote_board("aggregator"));
    assert!(!is_all_remote_board("not-a-real-board"));
}
