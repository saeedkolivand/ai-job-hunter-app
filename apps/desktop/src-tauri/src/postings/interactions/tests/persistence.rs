use super::{support::*, *};

/// `save` writes a sibling temp file and renames it over the real file (see
/// `platform::fs::write_atomic`), so the on-disk copy is replaced atomically
/// instead of being truncated in place. No temp file is left on the happy path.
#[test]
fn save_replaces_the_file_atomically_and_leaves_no_temp_file() {
    let (_dir, data_dir, mut store) = open_store();

    store.upsert(interaction("job-1", "viewed"));
    store.upsert(interaction("job-2", "applied"));

    let data_file = data_dir.join("interactions.json");
    assert!(data_file.exists(), "the interactions file must be written");
    let leftovers: Vec<_> = std::fs::read_dir(&data_dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "the temp file must be renamed away, not left behind"
    );

    // The file is complete and parses — a truncated write would not.
    let on_disk: Vec<InteractionRecord> =
        serde_json::from_str(&std::fs::read_to_string(&data_file).unwrap()).unwrap();
    assert_eq!(on_disk.len(), 2);

    // A fresh store hydrating from that file sees both interactions.
    let mut reopened = InteractionStore::new(&data_dir);
    assert_eq!(reopened.list(None).len(), 2);
}

#[test]
fn corrupt_interactions_file_is_preserved_not_overwritten() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let data_file = data_dir.join("interactions.json");

    // Simulate a file that exists on disk but is malformed (truncated write,
    // disk corruption, manual edit). The old loader swallowed the parse error
    // and started from an empty map, so the next save wiped every interaction.
    std::fs::write(&data_file, b"{ this is not valid json ]").unwrap();

    let mut store = InteractionStore::new(&data_dir);
    // First access hydrates the cache; the corrupt file is moved aside.
    let records = store.list(None);
    assert!(records.is_empty(), "corrupt file loads as empty in-memory");

    // The original bytes are preserved in the backup, NOT silently discarded.
    let backup = data_dir.join("interactions.json.corrupt");
    assert!(backup.exists(), "corrupt file is backed up");
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        "{ this is not valid json ]",
        "backup keeps the original corrupt bytes"
    );

    // A subsequent mutation rewrites the primary file (now valid), but the
    // backup still holds the recoverable original.
    store.upsert(InteractionRecord {
        timestamp: 1,
        ..interaction("job-1", "viewed")
    });
    assert!(backup.exists(), "backup survives the next save");
}

#[test]
fn corrupt_file_with_failed_backup_blocks_save_keeping_original() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let data_file = data_dir.join("interactions.json");
    let backup = data_dir.join("interactions.json.corrupt");

    // Corrupt primary file on disk.
    let original = b"{ this is not valid json ]";
    std::fs::write(&data_file, original).unwrap();

    // Force the backup rename to FAIL cross-platform: the backup target already
    // exists as a NON-EMPTY directory, so renaming a file onto it errors on every
    // OS. This simulates the rename failing (file locked / cross-device / perms).
    std::fs::create_dir(&backup).unwrap();
    std::fs::write(backup.join("sentinel"), b"x").unwrap();

    let mut store = InteractionStore::new(&data_dir);
    // Hydrating sees the corrupt file, attempts the backup, and the rename fails.
    let records = store.list(None);
    assert!(records.is_empty(), "corrupt file loads as empty in-memory");
    assert!(store.block_save, "failed backup arms the save guard");

    // A mutation would normally rewrite the primary file. With the guard armed,
    // save MUST skip the write so the un-backed-up corrupt original is preserved.
    store.upsert(InteractionRecord {
        timestamp: 1,
        ..interaction("job-1", "viewed")
    });

    assert_eq!(
        std::fs::read(&data_file).unwrap(),
        original,
        "save did not overwrite the un-backed-up corrupt original"
    );
}

#[test]
fn successful_backup_leaves_save_unblocked() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let data_file = data_dir.join("interactions.json");

    // Corrupt primary file, no obstruction at the backup path → rename succeeds.
    std::fs::write(&data_file, b"{ not json ]").unwrap();

    let mut store = InteractionStore::new(&data_dir);
    store.list(None);
    assert!(
        !store.block_save,
        "a successful backup must NOT arm the save guard"
    );

    // save proceeds: the now-free primary path is rewritten with valid JSON.
    store.upsert(InteractionRecord {
        timestamp: 1,
        ..interaction("job-1", "viewed")
    });
    let written = std::fs::read_to_string(&data_file).unwrap();
    let parsed: Vec<InteractionRecord> = serde_json::from_str(&written).unwrap();
    assert_eq!(parsed.len(), 1, "save wrote fresh data to the freed path");
}

#[test]
fn missing_interactions_file_loads_empty_without_backup() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    // No interactions.json written — first run.
    let mut store = InteractionStore::new(&data_dir);
    assert!(store.list(None).is_empty());
    // A missing file is normal, not corruption: no .corrupt backup is created.
    assert!(
        !data_dir.join("interactions.json.corrupt").exists(),
        "missing file must not be treated as corrupt"
    );
}
