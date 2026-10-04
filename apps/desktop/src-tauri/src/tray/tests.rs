use tempfile::TempDir;

use super::*;

/// `n` freshly-created autopilots (default status `Active`) in a
/// temp-file-backed store, returning the store, their ids in creation
/// order, and the `TempDir` guard (must outlive the store's file access).
fn seeded_store(n: usize) -> (AutopilotStore, Vec<String>, TempDir) {
    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    let ids = (0..n)
        .map(|i| {
            store
                .create(serde_json::json!({ "name": format!("ap-{i}") }))
                .id
        })
        .collect();
    (store, ids, temp)
}

fn statuses(store: &AutopilotStore, ids: &[String]) -> Vec<AutopilotStatus> {
    ids.iter().map(|id| store.get(id).unwrap().status).collect()
}

fn pause_all_active(store: &AutopilotStore) {
    set_all_status(store, AutopilotStatus::Active, AutopilotStatus::Paused);
}

fn resume_all_paused(store: &AutopilotStore) {
    set_all_status(store, AutopilotStatus::Paused, AutopilotStatus::Active);
}

#[test]
fn pause_all_active_pauses_every_active_one_and_leaves_others_alone() {
    let (store, ids, _temp) = seeded_store(3);
    store.set_status(&ids[1], AutopilotStatus::Paused);
    store.set_status(&ids[2], AutopilotStatus::Archived);

    pause_all_active(&store);

    assert_eq!(
        statuses(&store, &ids),
        vec![
            AutopilotStatus::Paused,   // was Active, got paused
            AutopilotStatus::Paused,   // was already Paused, untouched
            AutopilotStatus::Archived  // never touched
        ]
    );
}

#[test]
fn pause_all_active_pauses_a_single_active_one() {
    // The positive single-Active case: exactly one autopilot, Active, gets
    // paused by a `pause_all` click.
    let (store, ids, _temp) = seeded_store(1);

    pause_all_active(&store);

    assert_eq!(statuses(&store, &ids), vec![AutopilotStatus::Paused]);
}

#[test]
fn pause_all_active_is_a_no_op_when_all_are_already_paused() {
    let (store, ids, _temp) = seeded_store(2);
    for id in &ids {
        store.set_status(id, AutopilotStatus::Paused);
    }

    pause_all_active(&store);

    assert_eq!(statuses(&store, &ids), vec![AutopilotStatus::Paused; 2]);
}

#[test]
fn pause_all_active_is_a_no_op_when_all_are_archived() {
    let (store, ids, _temp) = seeded_store(2);
    for id in &ids {
        store.set_status(id, AutopilotStatus::Archived);
    }

    pause_all_active(&store);

    assert_eq!(statuses(&store, &ids), vec![AutopilotStatus::Archived; 2]);
}

#[test]
fn pause_all_active_is_a_no_op_on_an_empty_store() {
    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());

    // Must not panic on an empty list; nothing to assert beyond that.
    pause_all_active(&store);
}

#[test]
fn resume_all_paused_resumes_every_paused_one_and_leaves_others_alone() {
    let (store, ids, _temp) = seeded_store(3);
    store.set_status(&ids[0], AutopilotStatus::Paused);
    store.set_status(&ids[2], AutopilotStatus::Archived);

    resume_all_paused(&store);

    assert_eq!(
        statuses(&store, &ids),
        vec![
            AutopilotStatus::Active,   // was Paused, got resumed
            AutopilotStatus::Active,   // was already Active, untouched
            AutopilotStatus::Archived  // never touched
        ]
    );
}

#[test]
fn resume_all_paused_is_a_no_op_when_none_are_paused() {
    let (store, ids, _temp) = seeded_store(2); // all default to Active

    resume_all_paused(&store);

    assert_eq!(statuses(&store, &ids), vec![AutopilotStatus::Active; 2]);
}

#[test]
fn archived_autopilots_are_never_touched_by_either_action() {
    let (store, ids, _temp) = seeded_store(2);
    store.set_status(&ids[0], AutopilotStatus::Archived);
    store.set_status(&ids[1], AutopilotStatus::Paused);

    pause_all_active(&store); // nothing is Active, so this is a no-op
    assert_eq!(
        statuses(&store, &ids),
        vec![AutopilotStatus::Archived, AutopilotStatus::Paused]
    );

    resume_all_paused(&store); // resumes the Paused one only
    assert_eq!(
        statuses(&store, &ids),
        vec![AutopilotStatus::Archived, AutopilotStatus::Active]
    );
}

/// `resume_all` resumes literally every `Paused` autopilot, not just ones
/// this session's `pause_all` paused — there is no "which ones did the
/// tray pause" record, by design. Seed a mix of Active and already
/// individually-Paused (as if the user had paused it from Settings →
/// Autopilot before ever touching the tray), run `pause_all` then
/// `resume_all`, and confirm every non-archived autopilot round-trips
/// back to `Active` — including the one that was Paused before either
/// tray action ran.
#[test]
fn pause_then_resume_round_trip_resumes_every_non_archived_one_including_a_pre_existing_pause() {
    let (store, ids, _temp) = seeded_store(3);
    store.set_status(&ids[0], AutopilotStatus::Paused); // paused before either tray click
    store.set_status(&ids[2], AutopilotStatus::Archived);
    // ids[1] stays Active.

    pause_all_active(&store); // pauses ids[1]; ids[0] already Paused, ids[2] untouched
    assert_eq!(
        statuses(&store, &ids),
        vec![
            AutopilotStatus::Paused,
            AutopilotStatus::Paused,
            AutopilotStatus::Archived
        ]
    );

    resume_all_paused(&store); // resumes BOTH ids[0] and ids[1], including the pre-existing pause
    assert_eq!(
        statuses(&store, &ids),
        vec![
            AutopilotStatus::Active,
            AutopilotStatus::Active,
            AutopilotStatus::Archived
        ]
    );
}
