//! Loading and saving `autopilots.json`: tolerant loading, dead-key stripping, and the
//! no-op-write skip.

use super::super::*;
use super::support::*;
use tempfile::TempDir;

#[test]
fn legacy_auto_apply_records_load_and_strip_dead_keys_on_save() {
    let (_temp, dir) = temp_dir();

    // A record persisted before the auto-apply engine was removed: it still
    // carries the now-dropped `action` / `autoSubmit` keys. Loading must not
    // fail (serde ignores unknown fields) — the silent find-&-save migration.
    let legacy = r#"[{
        "_id": "ap-legacy",
        "name": "Legacy AP",
        "status": "active",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "action": "auto_apply",
        "schedule": "daily",
        "autoSubmit": true,
        "coverLetter": "Dear team",
        "totalFound": 4,
        "totalApplied": 2,
        "foundJobs": [],
        "createdAt": 1,
        "updatedAt": 1
    }]"#;
    std::fs::write(dir.join("autopilots.json"), legacy).unwrap();

    let store = AutopilotStore::new(&dir);
    let list = store.list();
    assert_eq!(list.len(), 1, "legacy record loads despite dropped keys");
    let ap = &list[0];
    assert_eq!(ap.id, "ap-legacy");
    assert_eq!(ap.schedule, "daily");
    assert_eq!(ap.status, AutopilotStatus::Active);
    assert_eq!(ap.cover_letter.as_deref(), Some("Dear team"));

    // Touching the record rewrites the file from the new struct — the dead
    // auto-apply keys are gone from disk going forward.
    store.stamp_last_run("ap-legacy");
    let on_disk = std::fs::read_to_string(dir.join("autopilots.json")).unwrap();
    assert!(!on_disk.contains("\"action\""), "action stripped on save");
    assert!(
        !on_disk.contains("autoSubmit"),
        "autoSubmit stripped on save"
    );
    assert!(
        on_disk.contains("Dear team"),
        "kept fields survive the rewrite"
    );
}

#[test]
fn load_drops_only_the_unparseable_record_not_the_whole_file() {
    let (_temp, dir) = temp_dir();

    // One good record + one record whose `runStatus` is a variant this build's
    // `RunStatus` enum doesn't know (e.g. a downgrade after a future release
    // added one) — before the per-record tolerant load, ANY record failing
    // `Vec<Autopilot>` deserialization failed the WHOLE file parse, producing
    // an empty map; a later `save()` would then silently overwrite the file
    // and lose every other (perfectly valid) record too.
    let mixed = r#"[
        {
            "_id": "ap-good",
            "name": "Good AP",
            "status": "active",
            "target": { "board": "linkedin", "query": "rust", "pages": 1 },
            "filter": { "minMatchScore": 50.0 },
            "schedule": "daily",
            "totalFound": 0,
            "totalApplied": 0,
            "createdAt": 1,
            "updatedAt": 1
        },
        {
            "_id": "ap-future",
            "name": "Future AP",
            "status": "active",
            "target": { "board": "linkedin", "query": "rust", "pages": 1 },
            "filter": { "minMatchScore": 50.0 },
            "schedule": "daily",
            "runStatus": "someFutureStatus",
            "totalFound": 0,
            "totalApplied": 0,
            "createdAt": 1,
            "updatedAt": 1
        }
    ]"#;
    std::fs::write(dir.join("autopilots.json"), mixed).unwrap();

    let store = AutopilotStore::new(&dir);
    let list = store.list();
    assert_eq!(
        list.len(),
        1,
        "the good record loads; only the unparseable one is dropped"
    );
    assert_eq!(list[0].id, "ap-good");

    // A post-tolerant-load save must not lose data on disk. `create()` reads
    // via the same `load()` (tolerant-parsed, already dropped "ap-future") and
    // then `save()`s the FULL in-memory map back — if that save ever wrote only
    // the newly-touched record instead of the whole map, "ap-good" would
    // silently vanish from disk the moment anything else was created.
    create_ap(&store, "New AP", "linkedin", 50.0, "manual");

    // A FRESH store over the same dir has an empty cache, so its `list()` call
    // re-reads and re-parses what actually landed on disk — not the in-memory
    // cache the original `store` still holds. This is the real proof that the
    // save after a tolerant load didn't drop data.
    let fresh_store = AutopilotStore::new(&dir);
    let fresh_list = fresh_store.list();
    assert_eq!(
        fresh_list.len(),
        2,
        "both the surviving original record and the newly created one must be on disk"
    );
    assert!(
        fresh_list.iter().any(|a| a.id == "ap-good"),
        "the original good record must survive a post-tolerant-load save, not just the in-memory read"
    );
    assert!(
        fresh_list.iter().any(|a| a.name == "New AP"),
        "the newly created record must also be present"
    );
}

#[test]
fn save_skips_disk_write_when_serialized_state_is_unchanged() {
    let (temp, store) = temp_store();
    create_ap(&store, "AP", "linkedin", 50.0, "manual");

    // After `create`, disk already holds exactly the serialized JSON, so the
    // dirty check compares equal and must skip the write. Probe with mtime: a
    // skipped write never touches the file (mtime frozen); a real rewrite would
    // bump it. Re-save the identical, unchanged map and assert mtime is stable.
    let file = temp.path().join("autopilots.json");
    let before = std::fs::metadata(&file).unwrap().modified().unwrap();

    let map = store.load();
    store.save(map);

    let after = std::fs::metadata(&file).unwrap().modified().unwrap();
    assert_eq!(
        before, after,
        "identical serialized state must skip the write (mtime unchanged)"
    );
    // And the state is preserved, not blanked.
    assert!(std::fs::read_to_string(&file).unwrap().contains("\"_id\""));
}

#[test]
fn save_writes_when_serialized_state_differs() {
    let (temp, store) = temp_store();
    create_ap(&store, "AP", "linkedin", 50.0, "manual");

    // Overwrite the file with content that does NOT match the serialized map,
    // then save the (unchanged) map: the bytes differ, so the write must proceed
    // and replace the sentinel content with the real serialized JSON.
    let file = temp.path().join("autopilots.json");
    std::fs::write(&file, "// stale sentinel content").unwrap();

    let map = store.load();
    store.save(map);

    let after = std::fs::read_to_string(&file).unwrap();
    assert!(
        !after.contains("stale sentinel"),
        "differing on-disk content must trigger a write"
    );
    assert!(
        after.contains("\"_id\""),
        "real serialized state was written"
    );
}

#[test]
fn save_writes_when_file_is_missing() {
    let (temp, store) = temp_store();
    create_ap(&store, "AP", "linkedin", 50.0, "manual");

    // A missing/unreadable file never matches the serialized bytes → the write
    // must proceed so state isn't lost on the first persist after deletion.
    let file = temp.path().join("autopilots.json");
    std::fs::remove_file(&file).unwrap();
    assert!(!file.exists());

    let map = store.load();
    store.save(map);

    assert!(file.exists(), "missing file is (re)written, not skipped");
    let after = std::fs::read_to_string(&file).unwrap();
    assert!(after.contains("\"_id\""), "serialized state was written");
}

#[test]
fn write_to_disk_surfaces_error_instead_of_swallowing_it() {
    // Point the store's "data dir" at a real FILE, so `data_file` resolves to a
    // path *under* a non-directory. Neither the create_dir_all in `new` (`.ok`'d)
    // nor the final `std::fs::write` can create a child of a file, so the write
    // fails deterministically on every OS. This is exactly the error `save` now
    // logs via `log::error` instead of `.ok()`-swallowing (quick win 9) — proving
    // the error path is real and detectable rather than silently lost.
    let temp = TempDir::new().unwrap();
    let file_as_dir = temp.path().join("not-a-directory");
    std::fs::write(&file_as_dir, b"x").unwrap();

    let store = AutopilotStore::new(&file_as_dir);
    let result = store.write_to_disk(&HashMap::new());
    assert!(
        result.is_err(),
        "writing under a non-directory path must surface an IO error, not be swallowed"
    );

    // `save` (which now logs that error) must not panic on the failure path and
    // still keeps the in-memory cache consistent for the running process.
    store.save(HashMap::new());
    assert!(
        store.list().is_empty(),
        "save tolerates a persist failure without panicking and reflects the intended state in-memory"
    );
}
