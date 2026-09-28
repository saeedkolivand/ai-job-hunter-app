use super::super::*;

use super::super::super::test_support::{app_meta, open_store, sample_posting};
use crate::applications::{ApplicationOrigin, ApplicationStatus};

// ── import-isolation contract ──────────────────────────────────────────────────
// `persist_import_application` is the WHOLE persistence side effect of an import.
// It takes only the `ApplicationStore` (no `PostingsCache`), so an import can
// never enter the Jobs/discovery feed — these lock that mapping + the Saved origin.

/// An import persists exactly one Saved Application carrying the posting's
/// company/title; the absence of a `PostingsCache` parameter is the structural
/// guarantee that nothing lands in the Jobs feed.
#[test]
fn import_persists_one_saved_application_from_posting() {
    let (_dir, store) = open_store();
    let posting = sample_posting("https://acme.example/jobs/77", "Acme", "Staff Engineer");

    let (id, status) =
        persist_import_application(&store, "https://acme.example/jobs/77", &posting, None).unwrap();

    assert_eq!(
        status, "saved",
        "an import with no applied flag stays saved"
    );
    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Saved);
    assert_eq!(app.company, "Acme");
    assert_eq!(app.title, "Staff Engineer");
    assert_eq!(
        store.list().len(),
        1,
        "import creates exactly one Application"
    );
}

/// `applied=Some(true)` from the extension advances the imported Application
/// straight to `applied`.
#[test]
fn import_applied_flag_advances_status() {
    let (_dir, store) = open_store();
    let posting = sample_posting("https://acme.example/jobs/78", "Beta", "SRE");

    let (_id, status) =
        persist_import_application(&store, "https://acme.example/jobs/78", &posting, Some(true))
            .unwrap();

    assert_eq!(status, "applied");
}

/// `applied=None` (or `applied=Some(false)`) with `ApplicationOrigin::Saved`
/// must create ONE Application with status `saved` and no `applied_at`.
#[test]
fn persistence_matrix_saved_no_applied_flag_yields_saved_status() {
    let (_dir, store) = open_store();

    let id = store
        .upsert_for_origin(
            "https://jobs.example.com/posting/100",
            "linkedin",
            &app_meta("Acme", "Frontend Engineer"),
            ApplicationOrigin::Saved,
            None, // applied flag absent
        )
        .unwrap();

    let apps = store.list();
    assert_eq!(apps.len(), 1, "exactly one Application created");
    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Saved,
        "absent applied flag with Saved origin must yield status=saved"
    );
    assert!(
        app.applied_at.is_none(),
        "applied_at must be None when status is saved"
    );
    assert_eq!(app.company, "Acme");
    assert_eq!(app.title, "Frontend Engineer");
}

/// `applied=Some(false)` is equivalent to absent — still `saved`.
#[test]
fn persistence_matrix_saved_applied_false_yields_saved_status() {
    let (_dir, store) = open_store();

    let id = store
        .upsert_for_origin(
            "https://jobs.example.com/posting/101",
            "indeed",
            &app_meta("Beta Inc", "DevOps Engineer"),
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Saved,
        "applied=Some(false) must yield status=saved"
    );
    assert!(app.applied_at.is_none());
}

/// `applied=Some(true)` must advance the status to `applied` immediately.
#[test]
fn persistence_matrix_applied_true_flag_yields_applied_status() {
    let (_dir, store) = open_store();

    let id = store
        .upsert_for_origin(
            "https://jobs.example.com/posting/102",
            "greenhouse",
            &app_meta("Globex", "Platform Engineer"),
            ApplicationOrigin::Saved,
            Some(true), // extension flagged this job as already applied
        )
        .unwrap();

    let apps = store.list();
    assert_eq!(apps.len(), 1, "exactly one Application created");
    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Applied,
        "applied=Some(true) must yield status=applied"
    );
    assert!(
        app.applied_at.is_some(),
        "applied_at must be set when applied=true"
    );
}

/// Re-importing the same URL (same normalized form, different raw variants)
/// must produce ONE Application, merge the fields, and not create a duplicate.
#[test]
fn persistence_matrix_dedup_same_url_merges_not_duplicates() {
    let (_dir, store) = open_store();

    // First import: raw URL with query param and trailing slash.
    let id_first = store
        .upsert_for_origin(
            "https://www.acmecorp.example/jobs/42/?utm_source=ext",
            "url",
            &app_meta("Acme Corp", "Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // Second import: canonical URL (www-stripped, no query, no trailing slash).
    let id_second = store
        .upsert_for_origin(
            "https://acmecorp.example/jobs/42",
            "url",
            &app_meta("Acme Corp", "Senior Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    assert_eq!(
        id_first, id_second,
        "same normalized URL must merge into the same Application row (no dup)"
    );

    let apps = store.list();
    assert_eq!(apps.len(), 1, "dedup: exactly one Application in the store");

    // The merged row should carry the latest-import title.
    let app = store.get(&id_first).unwrap();
    // Title updated to the second import value.
    assert_eq!(app.title, "Senior Backend Engineer");
    assert_eq!(app.company, "Acme Corp");
}

/// A `saved` → re-import with `applied=Some(true)` must advance the existing
/// row's status to `applied` (not create a second row).
#[test]
fn persistence_matrix_reimport_with_applied_true_advances_saved_to_applied() {
    let (_dir, store) = open_store();

    let url = "https://jobs.example.com/posting/200";

    // First import: saved.
    let id = store
        .upsert_for_origin(
            url,
            "url",
            &app_meta("Initech", "SRE"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    assert_eq!(store.get(&id).unwrap().status, ApplicationStatus::Saved);

    // Re-import: same URL, user ticked "applied" in the extension popup.
    let id2 = store
        .upsert_for_origin(
            url,
            "url",
            &app_meta("Initech", "SRE"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();

    assert_eq!(id, id2, "re-import must not create a second row");
    assert_eq!(
        store.list().len(),
        1,
        "still exactly one Application after re-import"
    );

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Applied,
        "re-import with applied=true must advance status from saved to applied"
    );
    assert!(app.applied_at.is_some());
}
