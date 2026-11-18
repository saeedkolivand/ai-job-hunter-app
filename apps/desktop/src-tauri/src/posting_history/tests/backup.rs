//! Backup, restore and sync of history rows, and hostile peer rows.

use super::*;

#[test]
fn import_clears_the_device_local_markers() {
    let (_dir, store) = open();
    see(&store, T0);
    store.import(&serde_json::json!([])).unwrap();
    assert_eq!(marker_count(&store), 0);
}

#[test]
fn backup_round_trip_preserves_every_field_including_the_source() {
    let (_dir, store) = open();
    see(&store, T0);
    without_acme(&store, T0 + DAY, A);
    see(&store, T0 + 10 * DAY);
    let bundle = store.export();
    let (_dir2, other) = open();
    assert_eq!(other.import(&bundle).unwrap(), 2);
    assert_eq!(other.export(), bundle);
    let row = acme_row(&other);
    assert_eq!(
        (row.first_seen_at, row.seen_count, row.repost_count),
        (T0, 2, 1)
    );
    assert_eq!(row.last_source, A);
    assert_eq!(bundle[0]["ghostKey"], "acme|rust engineer");
    assert_eq!(bundle[0]["lastSource"], A);
}

#[test]
fn import_replaces_and_a_malformed_row_aborts_without_clearing() {
    let (_dir, store) = open();
    see(&store, T0);
    let bad = serde_json::json!([{"ghostKey": "x", "firstSeenAt": "nope"}]);
    assert!(store.import(&bad).is_err());
    assert_eq!(store.rows().len(), 1, "failed import left the table alone");
    assert_eq!(marker_count(&store), 1, "and left the markers alone");
    assert!(store.import(&serde_json::json!({})).is_err());
    assert_eq!(store.import(&serde_json::json!([])).unwrap(), 0);
    assert!(store.rows().is_empty());
}

#[test]
fn sync_record_round_trip_keys_on_the_ghost_key() {
    let (_dir, a) = open();
    see(&a, T0);
    let records = a.sync_records().unwrap();
    assert_eq!(records[0].id, "acme|rust engineer");
    let (_dir2, b) = open();
    b.upsert_record(&records[0].id, &records[0].payload)
        .unwrap();
    assert_eq!(b.export(), a.export());
    b.delete_record(&records[0].id).unwrap();
    assert!(b.rows().is_empty());
}

fn hostile_row() -> serde_json::Value {
    serde_json::json!([{
        "ghostKey": "evil|role", "firstSeenAt": 0, "lastSeenAt": u64::MAX / 2,
        "seenCount": i64::MAX, "repostCount": u32::MAX, "lastUrl": "u"
    }])
}

fn assert_clamped(store: &PostingHistoryStore) {
    let row = store.rows().into_iter().next().unwrap();
    assert!(
        row.last_seen_at <= now_ms(),
        "future last_seen_at bent to now"
    );
    assert!(row.first_seen_at <= row.last_seen_at);
    assert_eq!(row.seen_count, MAX_COUNT);
    assert_eq!(row.repost_count, MAX_COUNT);
}

#[test]
fn a_hostile_backup_row_is_clamped_and_a_following_scrape_does_not_overflow() {
    let (_dir, store) = open();
    store.import(&hostile_row()).unwrap();
    assert_clamped(&store);
    // The hostile row has no source, so its return counts nothing; the counters
    // advance by exactly one scrape from the clamped value.
    let out = run(&store, now_ms() + 30 * DAY, A, &[("Evil", "Role", "u")]);
    assert_eq!(
        out["evil|role"].repost_count,
        u32::try_from(MAX_COUNT).unwrap()
    );
    assert_eq!(store.rows()[0].seen_count, MAX_COUNT + 1);
}

#[test]
fn a_hostile_sync_record_is_clamped_the_same_way() {
    let (_dir, store) = open();
    let payload = hostile_row()[0].clone();
    store.upsert_record("evil|role", &payload).unwrap();
    assert_clamped(&store);
}

#[test]
fn saturating_counters_survive_a_row_at_i64_max() {
    let (_dir, store) = open();
    // Bypass the import clamp to prove record_scrape itself cannot overflow.
    store
        .conn
        .lock()
        .execute(
            "INSERT INTO posting_history VALUES ('acme|rust engineer', 1, 2, ?1, ?1, '', ?2)",
            params![i64::MAX, A],
        )
        .unwrap();
    without_acme(&store, 3, A);
    see(&store, 3 + 30 * DAY);
    assert_eq!(
        acme_row(&store).seen_count,
        u64::try_from(i64::MAX).unwrap()
    );
    assert_eq!(
        acme_row(&store).repost_count,
        u64::try_from(i64::MAX).unwrap()
    );
}
