//! Unit tests for `save_answers_optin`.

use super::*;

#[test]
fn defaults_off_and_round_trips_through_the_setter() {
    let dir = tempfile::tempdir().unwrap();
    let state = BridgeState::load(dir.path());
    assert!(!state.save_answers_on_submit_enabled());

    assert!(state.set_save_answers_on_submit_enabled(true));
    assert!(state.save_answers_on_submit_enabled());
    assert!(load_save_answers_on_submit_optin(dir.path()));

    // Re-setting the same value reports no change, mirrors every sibling flag's setter.
    assert!(!state.set_save_answers_on_submit_enabled(true));
}

/// See `set_save_answers_on_submit_enabled`'s own doc for why a persist failure must never
/// leave memory MORE permissive than disk. Make the next write fail and disable — the flip
/// must be refused, memory left exactly where disk still is.
#[test]
fn a_persist_failure_leaves_memory_matching_what_is_still_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let state = BridgeState::load(dir.path());
    assert!(state.set_save_answers_on_submit_enabled(true));
    assert!(state.save_answers_on_submit_enabled());

    crate::platform::fs::fail_next_write_on_this_thread();

    assert!(
        !state.set_save_answers_on_submit_enabled(false),
        "a persist failure must report no change"
    );
    assert!(
        state.save_answers_on_submit_enabled(),
        "memory must stay in sync with the still-persisted (\"1\") value on a failed write"
    );
    assert!(
        load_save_answers_on_submit_optin(dir.path()),
        "the on-disk file must be untouched by the failed write"
    );
}
