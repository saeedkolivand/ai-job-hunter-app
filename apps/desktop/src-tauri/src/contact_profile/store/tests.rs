use super::*;

// ── `get()` / `try_get()` error handling (agent-cli review, P-r1-AC-R4-F3,
// issue #1180) ──────────────────────────────────────────────────────────

/// `get()` keeps its old "always returns something" contract for its many
/// read-only callers, even when the stored row is corrupt — it degrades to
/// `ContactProfile::default()` exactly as before. Corrupts the row directly
/// through the private `conn` field, which only this module and its
/// descendants can reach; there is no public way to land invalid JSON
/// (`set`/`import` only ever write a value that parses).
#[test]
fn get_degrades_to_default_on_a_corrupt_stored_row() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = ContactProfileStore::open(&dir.path().to_path_buf()).expect("open store");
    store
        .conn
        .lock()
        .execute(
            "UPDATE contact_profile SET data = ?1 WHERE id = 1",
            rusqlite::params!["not valid json"],
        )
        .expect("corrupt the row");

    assert_eq!(store.get(), ContactProfile::default());
}

/// `try_get()` is the fallible half `get()` is built on: the SAME corrupt
/// row that `get()` silently degrades from must surface as `Err` here,
/// never as `Ok(ContactProfile::default())` — a caller that would otherwise
/// treat "couldn't read" as "nothing stored" (the `contact_profile_set`
/// photo-restore in `extension_bridge/agent_call.rs`) needs to be able to
/// tell the two apart.
#[test]
fn try_get_reports_a_corrupt_stored_row_as_an_error() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = ContactProfileStore::open(&dir.path().to_path_buf()).expect("open store");
    store
        .conn
        .lock()
        .execute(
            "UPDATE contact_profile SET data = ?1 WHERE id = 1",
            rusqlite::params!["not valid json"],
        )
        .expect("corrupt the row");

    assert!(
        store.try_get().is_err(),
        "a corrupt stored row must be reported, not read back as an empty profile"
    );
}

/// The happy path: an empty (never-written) row is `Ok(default)`, not an
/// error — only an actual read/parse failure should refuse.
#[test]
fn try_get_is_ok_with_the_default_profile_when_nothing_is_stored() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = ContactProfileStore::open(&dir.path().to_path_buf()).expect("open store");
    assert_eq!(store.try_get().unwrap(), ContactProfile::default());
}

/// `try_get()` has TWO independent failure sources mapped through two
/// separate `.map_err(...)?` calls — the SQL read itself (a locked/busy row
/// in production; here, a dropped table) and the JSON parse of what it
/// returns. The corrupt-row tests above only exercise the parse one (the
/// query itself still succeeds there, returning a row with bad `data`);
/// this drops the table so `query_row` itself errors, proving that failure
/// is propagated too rather than silently degrading the way the old
/// `.ok().flatten()` implementation did for every failure source alike.
#[test]
fn try_get_reports_a_query_failure_as_an_error() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = ContactProfileStore::open(&dir.path().to_path_buf()).expect("open store");
    store
        .conn
        .lock()
        .execute("DROP TABLE contact_profile", [])
        .expect("drop the table");

    assert!(
        store.try_get().is_err(),
        "a query failure (e.g. a locked/busy row) must be reported, not read back as an empty profile"
    );
}
