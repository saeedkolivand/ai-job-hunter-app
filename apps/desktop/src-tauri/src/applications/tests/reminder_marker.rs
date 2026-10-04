use super::{support::*, *};

// ── Migration 8 backfill (no banner storm on the first post-upgrade sweep) ────

#[test]
fn migration_8_pre_marks_already_due_reminders_so_the_first_sweep_is_quiet() {
    // Without the backfill every pre-existing overdue reminder is NULL after the
    // upgrade, so the first sweep treats the user's whole backlog as brand new
    // and announces it. MAX_PER_SWEEP paces that; it does not suppress it.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("applications.db");
    let conn = crate::db::open(&path).unwrap();

    // Build the schema up to — but NOT including — the marker migration.
    let all = super::migrations::MIGRATIONS;
    for m in &all[..7] {
        (m.up)(&conn).unwrap();
    }
    let seed = |id: &str, next: Option<i64>| {
        conn.execute(
            "INSERT INTO applications (id, status, created_at, updated_at, next_action_at)
             VALUES (?1, 'applied', 1000, 1000, ?2)",
            params![id, next],
        )
        .unwrap();
    };
    let before = now_ms();
    seed("long-overdue", Some(1_000));
    seed("due-a-minute-ago", Some(ts_to_db(before - 60_000)));
    seed("due-later-today", Some(ts_to_db(before + 3_600_000)));
    seed("no-reminder", None);

    let m8 = &all[7];
    assert_eq!(
        m8.name, "add_applications_next_action_notified_at",
        "migration order is pinned — entries are append-only"
    );
    (m8.up)(&conn).unwrap();
    let after = now_ms();

    let marker = |id: &str| -> Option<i64> {
        conn.query_row(
            "SELECT next_action_notified_at FROM applications WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    };
    for id in ["long-overdue", "due-a-minute-ago"] {
        let stamped = marker(id).unwrap_or_else(|| panic!("{id} must be pre-marked as announced"));
        assert!(
            stamped >= ts_to_db(before) && stamped <= ts_to_db(after),
            "{id} must carry the migration's own timestamp, got {stamped}"
        );
    }
    assert_eq!(
        marker("due-later-today"),
        None,
        "a reminder that has NOT come due yet must still be announceable"
    );
    assert_eq!(
        marker("no-reminder"),
        None,
        "a row with no reminder is untouched"
    );

    // End to end: the sweep's own read now reports the backlog as delivered and
    // the future one as pending — exactly what `should_notify` filters on.
    // (`user_version` is set by hand because the bodies were replayed directly;
    // `ApplicationStore::open` would otherwise try to re-run all eight.)
    conn.execute_batch("PRAGMA user_version = 8").unwrap();
    drop(conn);
    let store = ApplicationStore::open(dir.path()).unwrap();
    let pending: Vec<String> = store
        .follow_up_candidates()
        .into_iter()
        .filter(|c| c.notified_at.is_none())
        .map(|c| c.id)
        .collect();
    assert_eq!(
        pending,
        vec!["due-later-today"],
        "only the not-yet-due reminder may still notify after the upgrade"
    );
}

#[test]
fn a_re_upsert_of_the_same_job_url_carries_the_notified_marker_forward() {
    // `write_row_conn` now WRITES this column, so every `Application` literal has
    // to carry the stored value forward. `update_fields` gets it free via
    // `..existing`; `upsert_internal`'s merge branch enumerates all fields, and
    // that is the one a re-scrape/re-track goes through. Getting it wrong there
    // re-arms an already-announced reminder on the next scrape.
    let (_dir, store) = open_store();
    let url = "https://acme.example/job/1";
    let id = store
        .track_manual(url, "b", &meta("Acme", "Engineer"))
        .unwrap();
    set_reminder(&store, &id, Some(5_000));
    assert!(store.mark_next_action_notified(&id, 5_000, 9_000).unwrap());

    // Same URL again — merges into the SAME row through `upsert_internal`.
    let same = store
        .track_manual(url, "b", &meta("Acme Corp", "Senior Engineer"))
        .unwrap();
    assert_eq!(
        same, id,
        "fixture sanity: the re-track must merge, not create"
    );
    assert_eq!(
        notified_at(&store),
        Some(9_000),
        "a re-upsert must not resurrect an already-announced reminder"
    );
}

#[test]
fn the_notified_marker_survives_an_export_import_round_trip() {
    // `export`/`import` is the backup path. Dropping the marker there meant
    // restoring a backup re-fired every reminder the user had already seen.
    let (_dir, store) = open_store();
    let id = track(&store, "Acme", "Engineer");
    set_reminder(&store, &id, Some(5_000));
    assert!(store.mark_next_action_notified(&id, 5_000, 9_000).unwrap());

    let bundle = store.export();
    assert_eq!(
        bundle[0]["nextActionNotifiedAt"],
        serde_json::json!(9_000),
        "the marker must be on the wire, under the camelCase name the TS type declares"
    );

    let (_restore_dir, restored) = open_store();
    restored.import(&bundle).unwrap();
    assert_eq!(
        notified_at(&restored),
        Some(9_000),
        "a restored backup must not re-announce an already-delivered reminder"
    );

    // A bundle exported by a build that predates the marker simply has no such
    // key — it must still import (serde default) and start un-notified.
    let mut legacy = bundle.clone();
    legacy[0]
        .as_object_mut()
        .unwrap()
        .remove("nextActionNotifiedAt")
        .expect("the field was present before removal");
    restored.import(&legacy).unwrap();
    assert_eq!(
        notified_at(&restored),
        None,
        "a pre-marker bundle deserializes and starts un-notified"
    );
}
