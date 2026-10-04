//! Corrupt autopilots.json handling (issue #1274).
//!
//! These tests mirror `postings`' corrupt-interaction-file tests:
//! (a) a NUL-filled autopilots.json → load returns empty, autopilots.json.corrupt
//!     exists with the original bytes, and a following save writes a fresh valid
//!     autopilots.json without touching the .corrupt file;
//! (b) a missing file → empty, no .corrupt file created;
//! (c) a file with one bad record among good ones still loads the good ones
//!     (existing behavior), and no .corrupt file is created;
//! (d) if the backup rename fails, the corrupt original survives a save.

use super::super::*;
use super::support::*;

#[test]
fn corrupt_autopilots_file_is_preserved_not_overwritten() {
    let (_temp, data_dir) = temp_dir();
    let data_file = data_dir.join("autopilots.json");

    // Simulate a file that exists on disk but is malformed (e.g. all NUL bytes:
    // the incident that triggered this fix). The old loader swallowed the parse
    // error and started from an empty map, so the next save would wipe the file.
    std::fs::write(&data_file, b"\0\0\0\0").unwrap();

    let store = AutopilotStore::new(&data_dir);
    // First access hydrates the cache; the corrupt file is moved aside.
    let list = store.list();
    assert!(list.is_empty(), "corrupt file loads as empty in-memory");

    // The original bytes are preserved in the backup, NOT silently discarded.
    let backup = data_dir.join("autopilots.json.corrupt");
    assert!(backup.exists(), "corrupt file is backed up");
    assert_eq!(
        std::fs::read(&backup).unwrap(),
        b"\0\0\0\0",
        "backup keeps the original corrupt bytes"
    );

    // A subsequent mutation rewrites the primary file (now valid), but the
    // backup still holds the recoverable original.
    create_ap(&store, "New AP", "linkedin", 0.0, "manual");
    assert!(backup.exists(), "backup survives the next save");
    let on_disk = std::fs::read_to_string(&data_file).unwrap();
    assert!(on_disk.contains("New AP"), "fresh valid file was written");
}

#[test]
fn missing_autopilots_file_loads_empty_without_backup() {
    let (_temp, data_dir) = temp_dir();
    // No autopilots.json written — first run.
    let store = AutopilotStore::new(&data_dir);
    assert!(store.list().is_empty());
    // A missing file is normal, not corruption: no .corrupt backup is created.
    assert!(
        !data_dir.join("autopilots.json.corrupt").exists(),
        "missing file must not be treated as corrupt"
    );
}

#[test]
fn autopilots_file_with_one_bad_record_loads_the_good_ones() {
    let (_temp, data_dir) = temp_dir();
    let data_file = data_dir.join("autopilots.json");

    // One good record + one record with an unknown runStatus variant (simulating
    // a downgrade after a future release added one). The per-record tolerant
    // parse must drop only the bad record, not the whole file.
    let mixed = r#"[
        {
            "_id": "ap-good",
            "name": "Good AP",
            "status": "active",
            "target": { "board": "linkedin", "query": "rust", "pages": 1 },
            "filter": { "minMatchScore": 0.0 },
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
            "filter": { "minMatchScore": 0.0 },
            "schedule": "daily",
            "runStatus": "someFutureStatus",
            "totalFound": 0,
            "totalApplied": 0,
            "createdAt": 1,
            "updatedAt": 1
        }
    ]"#;
    std::fs::write(&data_file, mixed).unwrap();

    let store = AutopilotStore::new(&data_dir);
    let list = store.list();
    assert_eq!(
        list.len(),
        1,
        "the good record loads; only the unparseable one is dropped"
    );
    assert_eq!(list[0].id, "ap-good");

    // No .corrupt file should be created — this is a valid JSON array with one
    // bad record, not a corrupt file.
    assert!(
        !data_dir.join("autopilots.json.corrupt").exists(),
        "valid JSON array with one bad record must not create .corrupt backup"
    );
}

#[test]
fn corrupt_file_with_failed_backup_blocks_save_keeping_original() {
    let (_temp, data_dir) = temp_dir();
    let data_file = data_dir.join("autopilots.json");
    let backup = data_dir.join("autopilots.json.corrupt");

    // Corrupt primary file on disk (NUL-filled, like the incident).
    let original = b"\0\0\0\0";
    std::fs::write(&data_file, original).unwrap();

    // Every backup slot is already taken (an older backup is never
    // overwritten), so there is nowhere to move the corrupt file.
    let _ = &backup;
    for slot in std::iter::once("autopilots.json.corrupt".to_string())
        .chain((1..10).map(|n| format!("autopilots.json.corrupt.{n}")))
    {
        std::fs::write(data_dir.join(slot), b"older backup").unwrap();
    }

    let store = AutopilotStore::new(&data_dir);
    // Hydrating sees the corrupt file, attempts the backup, and the rename fails.
    let list = store.list();
    assert!(list.is_empty(), "corrupt file loads as empty in-memory");
    assert!(
        store.is_block_save(),
        "no free backup slot arms the save guard"
    );

    // A mutation would normally rewrite the primary file. With the guard armed,
    // save MUST skip the write so the un-backed-up corrupt original is preserved.
    create_ap(&store, "New AP", "linkedin", 0.0, "manual");

    assert_eq!(
        std::fs::read(&data_file).unwrap(),
        original,
        "save did not overwrite the un-backed-up corrupt original"
    );
}

/// A second corruption must not lock the store: the older backup is kept and
/// the new corrupt file goes to the next free slot, so saves carry on.
#[test]
fn a_second_corruption_uses_the_next_backup_slot_and_saves_resume() {
    let (_temp, data_dir) = temp_dir();
    let data_file = data_dir.join("autopilots.json");
    std::fs::write(data_dir.join("autopilots.json.corrupt"), b"first incident").unwrap();
    std::fs::write(&data_file, b"\0\0\0\0").unwrap();

    let store = AutopilotStore::new(&data_dir);
    assert!(store.list().is_empty());
    assert!(!store.is_block_save(), "a taken slot must not block saves");
    assert_eq!(
        std::fs::read(data_dir.join("autopilots.json.corrupt")).unwrap(),
        b"first incident",
        "the older backup is never overwritten"
    );
    assert_eq!(
        std::fs::read(data_dir.join("autopilots.json.corrupt.1")).unwrap(),
        b"\0\0\0\0"
    );
}

/// A file that merely can't be READ (here: it's a directory; in the wild, a
/// sharing violation while antivirus holds it) is not corrupt: it stays where it
/// is, nothing is renamed, and saves are blocked so it can't be overwritten.
#[test]
fn an_unreadable_file_is_left_in_place_and_blocks_saves() {
    let (_temp, data_dir) = temp_dir();
    let data_file = data_dir.join("autopilots.json");
    std::fs::create_dir(&data_file).unwrap();

    let store = AutopilotStore::new(&data_dir);
    assert!(store.list().is_empty());
    assert!(store.is_block_save());
    assert!(data_file.is_dir(), "left in place");
    assert!(
        !data_dir.join("autopilots.json.corrupt").exists(),
        "not treated as corrupt"
    );
}

/// Bytes that aren't UTF-8 are damaged content: backed up like bad JSON, not
/// mistaken for an unreadable file (which would block saves instead).
#[test]
fn a_non_utf8_file_is_backed_up_as_corrupt() {
    let (_temp, data_dir) = temp_dir();
    let data_file = data_dir.join("autopilots.json");
    let original = [b'[', 0xff, 0xfe, b']'];
    std::fs::write(&data_file, original).unwrap();

    let store = AutopilotStore::new(&data_dir);
    assert!(store.list().is_empty());
    assert!(
        !store.is_block_save(),
        "a backed-up corrupt file must not block saves"
    );
    assert_eq!(
        std::fs::read(data_dir.join("autopilots.json.corrupt")).unwrap(),
        original
    );
}

fn seed_autopilots_file(data_file: &std::path::Path) {
    let (seed, store) = temp_store();
    create_ap(&store, "Seeded AP", "linkedin", 0.0, "manual");
    std::fs::copy(seed.path().join("autopilots.json"), data_file).unwrap();
}

/// A momentarily unreadable file must not cost the user their autopilots for
/// the rest of the session: once it's readable, the next load sees it.
#[test]
fn an_unreadable_file_is_read_again_once_it_becomes_readable() {
    let (_temp, data_dir) = temp_dir();
    let data_file = data_dir.join("autopilots.json");
    std::fs::create_dir(&data_file).unwrap();

    let store = AutopilotStore::new(&data_dir);
    assert!(store.list().is_empty());

    std::fs::remove_dir(&data_file).unwrap();
    seed_autopilots_file(&data_file);

    let list = store.list();
    assert_eq!(
        list.len(),
        1,
        "the empty stand-in must not have been cached"
    );
    assert!(!store.is_block_save());
}

/// A change made while saves are blocked must not look saved: it isn't written
/// (the file stays as it was) and it isn't kept in memory either.
#[test]
fn a_blocked_save_is_not_shown_as_saved() {
    let (_temp, data_dir) = temp_dir();
    let data_file = data_dir.join("autopilots.json");
    std::fs::create_dir(&data_file).unwrap();

    let store = AutopilotStore::new(&data_dir);
    create_ap(&store, "Not persisted", "linkedin", 0.0, "manual");

    assert!(
        data_file.is_dir(),
        "nothing was written over the unreadable file"
    );
    assert!(
        store.list().is_empty(),
        "the unsaved change is not served from memory"
    );
}
