use super::{support::*, *};

#[test]
fn export_import_round_trips() {
    let (_dir, store) = open_store();
    upsert(
        &store,
        "https://x.com/1",
        "b",
        &meta("X", "T"),
        ApplicationOrigin::Generate,
    );
    let bundle = store.export();

    let (_dir2, store2) = open_store();
    let n = store2.import(&bundle).unwrap();
    assert_eq!(n, 1);
    assert_eq!(store2.list().len(), 1);
    assert_eq!(store2.list()[0].company, "X");
}

/// HIGH blocker fix: `DataStore::import` must return `Err(AppError::Parse(…))` when
/// the supplied JSON value is not a JSON array.  The production path at mod.rs
/// line ~825 calls `.as_array().ok_or_else(|| AppError::Parse(…))`.
#[test]
fn import_non_array_returns_parse_error() {
    let (_dir, store) = open_store();

    // Passing an object instead of an array must be rejected.
    let result = store.import(&serde_json::json!({"key": "value"}));
    assert!(result.is_err(), "non-array input must return Err");

    // Check it is specifically the Parse variant.
    match result.unwrap_err() {
        AppError::Parse(msg) => {
            assert!(
                msg.contains("applications"),
                "error message should mention 'applications', got: {msg}"
            );
        }
        other => panic!("expected AppError::Parse, got: {other:?}"),
    }

    // Sanity: the store is still empty — the failed import must not have written anything.
    assert!(
        store.list().is_empty(),
        "store must be empty after a failed import"
    );
}

/// Happy-path companion: import a valid array after the error-path test to
/// confirm the store is still operational.
#[test]
fn import_non_array_does_not_corrupt_subsequent_happy_path() {
    let (_dir, store) = open_store();

    // First call fails.
    assert!(store.import(&serde_json::json!(42)).is_err());

    // Subsequent valid import still works.
    upsert(
        &store,
        "https://x.com/1",
        "b",
        &meta("X", "T"),
        ApplicationOrigin::Generate,
    );
    let bundle = store.export();

    let (_dir2, store2) = open_store();
    let n = store2.import(&bundle).unwrap();
    assert_eq!(n, 1, "valid import after failed import must succeed");
    assert_eq!(store2.list()[0].company, "X");
}

// ── R1 — ApplicationStore::import rollback regression guard ──────────────────
//
// `DataStore::import` for ApplicationStore runs clear+repopulate in ONE
// transaction. These tests pin that contract: a malformed LATER record must
// abort the import and leave PRIOR data fully intact.

/// Minimal valid Application JSON, compatible with the `Application` serde shape.
fn valid_application_json(id: &str, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "status": status,
        "appliedAt": null,
        "createdAt": 1_000_000u64,
        "updatedAt": 1_000_000u64,
        "jobUrl": "",
        "board": "linkedin",
        "company": "Acme",
        "title": "Engineer",
        "candidate": "Jane",
        "answers": [],
        "brief": "",
        "jobDescription": "",
        "notes": "",
        "nextActionAt": null,
        "comp": "",
        "contactName": "",
        "contactEmail": "",
        "jobSummary": ""
    })
}

#[test]
fn application_import_malformed_later_record_rolls_back_prior_data() {
    // R1 — Seed store with prior data, then import a bundle whose LAST element
    // has `status` as a number (must be a string). Import must fail and prior
    // data must be fully intact.
    let (_dir, store) = open_store();

    // Seed with a known application.
    let prior_id = track(&store, "Prior Corp", "Prior Role");
    let prior_count = store.list().len();
    assert_eq!(prior_count, 1, "precondition: one prior record");

    // Bundle: first element is valid, second has a numeric status (invalid type).
    let bundle = serde_json::json!([
        valid_application_json("new-1", "applied"),
        {
            "id": "bad-2",
            "status": 42,           // ← wrong type: must be string
            "appliedAt": null,
            "createdAt": 2_000_000u64,
            "updatedAt": 2_000_000u64,
            "jobUrl": "",
            "board": "",
            "company": "Bad Corp",
            "title": "Bad Role",
            "candidate": "",
            "answers": [],
            "brief": "",
            "jobDescription": "",
            "notes": "",
            "nextActionAt": null,
            "comp": "",
            "contactName": "",
            "contactEmail": "",
            "jobSummary": ""
        }
    ]);

    let result = crate::data_store::DataStore::import(&store, &bundle);
    assert!(
        result.is_err(),
        "import of a bundle with a malformed record must return Err; got Ok"
    );

    // PRIOR data must be fully intact — the transaction must have rolled back.
    let remaining = store.list();
    assert_eq!(
        remaining.len(),
        1,
        "import rollback must leave prior records intact; got {} records (expected 1)",
        remaining.len()
    );
    assert_eq!(
        remaining[0].id, prior_id,
        "the surviving record must be the original prior application, not a partial import"
    );
    // Status events for the prior record must also still be present.
    assert!(
        !store.events(&prior_id).is_empty(),
        "status events for the prior application must survive a rolled-back import"
    );
}

#[test]
fn application_import_all_valid_records_replaces_prior_data() {
    // R1 happy-path: confirms the import transaction commits when the bundle is
    // fully valid — prior data is replaced with the imported records.
    let (_dir, store) = open_store();

    track(&store, "Old Corp", "Old Role");
    assert_eq!(store.list().len(), 1, "precondition: one prior record");

    let bundle = serde_json::json!([
        valid_application_json("new-1", "applied"),
        valid_application_json("new-2", "saved"),
    ]);

    let n = crate::data_store::DataStore::import(&store, &bundle).unwrap();
    assert_eq!(n, 2, "import must report 2 records restored");

    let list = store.list();
    assert_eq!(list.len(), 2, "store must hold the 2 imported records");
    let ids: Vec<&str> = list.iter().map(|a| a.id.as_str()).collect();
    assert!(
        ids.contains(&"new-1") && ids.contains(&"new-2"),
        "both imported ids must be present; got {ids:?}"
    );
    // Prior record must be gone.
    assert!(
        list.iter().all(|a| a.company != "Old Corp"),
        "prior record 'Old Corp' must not survive a successful import"
    );
}
