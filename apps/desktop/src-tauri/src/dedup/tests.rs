use super::*;
use tempfile::TempDir;

fn open() -> (TempDir, DedupStore) {
    let dir = TempDir::new().unwrap();
    let store = DedupStore::open(dir.path()).unwrap();
    (dir, store)
}

#[test]
fn pair_orders_keys_ascending() {
    assert_eq!(DedupStore::pair("b", "a"), ("a".into(), "b".into()));
    assert_eq!(DedupStore::pair("a", "b"), ("a".into(), "b".into()));
}

#[test]
fn insert_is_order_independent_and_idempotent() {
    let (_dir, store) = open();
    store.insert_pairs(&[("z".into(), "a".into())]).unwrap();
    // Re-insert the SAME pair in the opposite order — must not duplicate.
    store.insert_pairs(&[("a".into(), "z".into())]).unwrap();
    let pairs = store.all_pairs();
    assert_eq!(pairs.len(), 1);
    assert!(pairs.contains(&("a".into(), "z".into())));
}

#[test]
fn self_pairs_are_ignored() {
    let (_dir, store) = open();
    store.insert_pairs(&[("a".into(), "a".into())]).unwrap();
    assert!(
        store.all_pairs().is_empty(),
        "a key can't be a dup of itself"
    );
}

#[test]
fn export_import_round_trips() {
    let (_dir, store) = open();
    store
        .insert_pairs(&[("k1".into(), "k2".into()), ("k3".into(), "k2".into())])
        .unwrap();
    let bundle = store.export();

    let (_dir2, store2) = open();
    let restored = store2.import(&bundle).unwrap();
    assert_eq!(restored, 2);
    assert_eq!(store2.all_pairs(), store.all_pairs());
    // Invariant preserved through the round-trip.
    assert!(store2.all_pairs().contains(&("k2".into(), "k3".into())));
}

#[test]
fn import_with_a_malformed_row_errors_and_preserves_existing_rows() {
    let (_dir, store) = open();
    store.insert_pairs(&[("keep".into(), "me".into())]).unwrap();

    // One well-formed row + one malformed (missing `keyB`). The store
    // deserializes EVERY row before mutating, so the malformed row must fail
    // the whole import BEFORE any DELETE/insert runs.
    let bundle = serde_json::json!([
        { "keyA": "a", "keyB": "b", "createdAt": 1 },
        { "keyA": "x", "createdAt": 2 }
    ]);
    let result = store.import(&bundle);
    assert!(
        result.is_err(),
        "a malformed row must fail the whole import"
    );

    // The pre-existing verdict survives untouched, and the well-formed row
    // from the failed bundle was NOT partially inserted.
    let pairs = store.all_pairs();
    assert!(
        pairs.contains(&("keep".into(), "me".into())),
        "existing rows must survive a failed import (deserialize-all-before-mutate)"
    );
    assert!(
        !pairs.contains(&("a".into(), "b".into())),
        "no partial insert from the failed import"
    );
    assert_eq!(pairs.len(), 1, "the table is exactly the pre-import state");
}

#[test]
fn import_replaces_existing_rows() {
    let (_dir, store) = open();
    store.insert_pairs(&[("old".into(), "row".into())]).unwrap();
    // A bundle with a different single pair replaces, not merges.
    let bundle = serde_json::json!([
        { "keyA": "a", "keyB": "b", "createdAt": 123 }
    ]);
    store.import(&bundle).unwrap();
    let pairs = store.all_pairs();
    assert_eq!(pairs.len(), 1);
    assert!(pairs.contains(&("a".into(), "b".into())));
}

#[test]
fn clear_all_empties_the_store() {
    let (_dir, store) = open();
    store.insert_pairs(&[("a".into(), "b".into())]).unwrap();
    assert!(!store.all_pairs().is_empty());
    store.clear_all();
    assert!(store.all_pairs().is_empty());
}

#[test]
fn reopening_the_same_db_is_migration_idempotent() {
    let dir = TempDir::new().unwrap();
    {
        let store = DedupStore::open(dir.path()).unwrap();
        store.insert_pairs(&[("a".into(), "b".into())]).unwrap();
    }
    // Second open re-runs run_migrations (no-op) and keeps the data.
    let store = DedupStore::open(dir.path()).unwrap();
    assert!(store.all_pairs().contains(&("a".into(), "b".into())));
}
