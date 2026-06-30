use super::*;
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_contact(id: &str, job_url: &str) -> ReferralContact {
    ReferralContact {
        id: id.into(),
        job_url: job_url.into(),
        company_name: "Acme".into(),
        person_name: "Jane Smith".into(),
        person_role: "Engineering Manager".into(),
        linkedin_url: "".into(),
        email_draft: "".into(),
        message_draft: "Hi Jane".into(),
        invite_note_draft: "".into(),
        channel: "linkedin_message".into(),
        status: "draft".into(),
        notes: "".into(),
        created_at: 1_000,
        updated_at: 1_000,
    }
}

// ── upsert / list / list_by_job ───────────────────────────────────────────────

#[test]
fn upsert_inserts_new_row_and_list_returns_it() {
    let dir = TempDir::new().unwrap();
    let store = ReferralStore::open(dir.path()).unwrap();

    store
        .upsert(&make_contact("ref-1", "https://acme.com/jobs/1"))
        .unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "ref-1");
    assert_eq!(list[0].company_name, "Acme");
    assert_eq!(list[0].person_name, "Jane Smith");
    assert_eq!(list[0].job_url, "https://acme.com/jobs/1");
}

#[test]
fn list_by_job_returns_only_matching_job_url() {
    let dir = TempDir::new().unwrap();
    let store = ReferralStore::open(dir.path()).unwrap();

    store
        .upsert(&make_contact("ref-1", "https://acme.com/jobs/1"))
        .unwrap();
    store
        .upsert(&make_contact("ref-2", "https://beta.com/jobs/9"))
        .unwrap();
    store
        .upsert(&make_contact("ref-3", "https://acme.com/jobs/1"))
        .unwrap();

    let by_job = store.list_by_job("https://acme.com/jobs/1");
    assert_eq!(by_job.len(), 2);
    assert!(by_job
        .iter()
        .all(|r| r.job_url == "https://acme.com/jobs/1"));

    // The beta job row must NOT appear.
    assert!(!by_job.iter().any(|r| r.id == "ref-2"));
}

#[test]
fn list_by_job_returns_empty_for_unknown_url() {
    let dir = TempDir::new().unwrap();
    let store = ReferralStore::open(dir.path()).unwrap();
    store
        .upsert(&make_contact("ref-1", "https://acme.com/jobs/1"))
        .unwrap();

    let result = store.list_by_job("https://no-such-company.io/jobs/99");
    assert!(result.is_empty());
}

// ── created_at immutability ───────────────────────────────────────────────────

#[test]
fn created_at_is_immutable_after_first_insert() {
    let dir = TempDir::new().unwrap();
    let store = ReferralStore::open(dir.path()).unwrap();

    // Insert with created_at = 1000.
    let original = make_contact("ref-1", "https://acme.com/jobs/1");
    store.upsert(&original).unwrap();

    // Upsert the SAME id with a different created_at (99999) and changed fields.
    let mut updated = original.clone();
    updated.created_at = 99_999;
    updated.updated_at = 2_000;
    updated.person_role = "VP Engineering".into();
    updated.message_draft = "Updated message".into();
    store.upsert(&updated).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1, "upsert must not duplicate the row");

    let row = &list[0];
    // created_at must be the ORIGINAL value — COALESCE keeps it.
    assert_eq!(
        row.created_at, 1_000,
        "created_at must remain 1_000; got {}",
        row.created_at
    );
    // Other mutable fields MUST be updated.
    assert_eq!(row.updated_at, 2_000);
    assert_eq!(row.person_role, "VP Engineering");
    assert_eq!(row.message_draft, "Updated message");
}

// ── remove ────────────────────────────────────────────────────────────────────

#[test]
fn remove_deletes_the_row_by_id() {
    let dir = TempDir::new().unwrap();
    let store = ReferralStore::open(dir.path()).unwrap();

    store
        .upsert(&make_contact("ref-1", "https://acme.com/jobs/1"))
        .unwrap();
    store
        .upsert(&make_contact("ref-2", "https://acme.com/jobs/1"))
        .unwrap();

    store.remove("ref-1").unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "ref-2");
}

#[test]
fn remove_unknown_id_is_a_noop_and_returns_ok() {
    let dir = TempDir::new().unwrap();
    let store = ReferralStore::open(dir.path()).unwrap();
    store
        .upsert(&make_contact("ref-1", "https://acme.com/jobs/1"))
        .unwrap();

    // Removing a non-existent id should not fail.
    store.remove("does-not-exist").unwrap();

    assert_eq!(store.list().len(), 1, "existing row must be untouched");
}

// ── clear_all ─────────────────────────────────────────────────────────────────

#[test]
fn clear_all_empties_the_table() {
    let dir = TempDir::new().unwrap();
    let store = ReferralStore::open(dir.path()).unwrap();

    store
        .upsert(&make_contact("ref-1", "https://acme.com/jobs/1"))
        .unwrap();
    store
        .upsert(&make_contact("ref-2", "https://acme.com/jobs/2"))
        .unwrap();

    store.clear_all();

    assert!(
        store.list().is_empty(),
        "table must be empty after clear_all"
    );
}

// ── command-layer allowlist (store-level contract) ────────────────────────────
// NOTE: The `referrals_upsert` and `referrals_remove` commands require an
// `AppHandle` (Tauri runtime state), which cannot be constructed in a plain unit
// test without a full Tauri app harness. The allowlist validation is therefore
// NOT covered here at the command level.
//
// The ALLOWED_CHANNELS / ALLOWED_STATUSES guard in commands/referrals.rs is still
// exercised end-to-end by the renderer's Zod schema test
// (`packages/shared/src/schemas/…`) + the integration test suite. If you add a
// pure-function extraction of the allowlist logic (a fn that takes a &str and
// returns a bool, independent of AppHandle), add a unit test here alongside it.
