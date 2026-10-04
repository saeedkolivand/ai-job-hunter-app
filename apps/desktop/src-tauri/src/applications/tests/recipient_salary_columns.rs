use super::{support::*, *};

/// Migration round-trip: seed a DB at user_version=4 (has job_summary column,
/// no recipient columns), open the store, verify migration 5 adds them, and
/// confirm pre-existing rows survive intact with DEFAULT '' values.
#[test]
fn recipient_columns_migrate_from_pre_recipient_schema() {
    let dir = TempDir::new().unwrap();
    let legacy_id = "app-legacy-recip-001";
    seed_legacy_row(dir.path(), 4, legacy_id);

    // Opening the store runs migration 5 (ADD COLUMN recipient_name/email).
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app = store
        .get(legacy_id)
        .expect("legacy row must be readable after migration");
    assert_eq!(
        app.recipient_name, "",
        "legacy row must get DEFAULT '' for recipient_name after migration"
    );
    assert_eq!(
        app.recipient_email, "",
        "legacy row must get DEFAULT '' for recipient_email after migration"
    );
    assert_eq!(app.id, legacy_id, "row id must be unchanged");

    // Write recipient fields and confirm they round-trip.
    edit(&store, legacy_id, |p| {
        p.recipient_name = Some("Jane Smith".into());
        p.recipient_email = Some("jane@acme.com".into());
    });
    let updated = store.get(legacy_id).unwrap();
    assert_eq!(updated.recipient_name, "Jane Smith");
    assert_eq!(updated.recipient_email, "jane@acme.com");
}

/// Recipient fields persist and round-trip through update_fields and export/import.
#[test]
fn recipient_fields_persist_and_export_import_round_trip() {
    let (_dir, store) = open_store();
    let id = track(&store, "Acme", "Engineer");

    // Set both fields.
    edit(&store, &id, |p| {
        p.recipient_name = Some("Jane Smith".into());
        p.recipient_email = Some("jane@acme.com".into());
    });
    let app = store.get(&id).unwrap();
    assert_eq!(app.recipient_name, "Jane Smith");
    assert_eq!(app.recipient_email, "jane@acme.com");

    // Export + import round-trips the fields.
    let bundle = store.export();
    let (_dir2, store2) = open_store();
    store2.import(&bundle).unwrap();
    let imported = store2.get(&id).unwrap();
    assert_eq!(imported.recipient_name, "Jane Smith");
    assert_eq!(imported.recipient_email, "jane@acme.com");

    // Clearing via empty string leaves the fields empty.
    edit(&store, &id, |p| {
        p.recipient_name = Some(String::new());
        p.recipient_email = Some(String::new());
    });
    let cleared = store.get(&id).unwrap();
    assert_eq!(cleared.recipient_name, "");
    assert_eq!(cleared.recipient_email, "");
}

/// Migration round-trip: seed a DB at user_version=5 (has recipient columns, no
/// salary columns), open the store, verify migration 6 adds them, and confirm
/// a pre-existing row survives with `None` salary (NULL, never 0).
#[test]
fn salary_columns_migrate_from_pre_salary_schema() {
    let dir = TempDir::new().unwrap();
    let legacy_id = "app-legacy-salary-001";
    seed_legacy_row(dir.path(), 5, legacy_id);

    // Opening the store runs migration 6 (ADD COLUMN salary_min/max/currency).
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app = store
        .get(legacy_id)
        .expect("legacy row must be readable after migration");
    assert_eq!(
        app.salary_min, None,
        "legacy row must get NULL (None), never 0, for salary_min after migration"
    );
    assert_eq!(app.salary_max, None);
    assert_eq!(app.salary_currency, None);
    assert_eq!(app.id, legacy_id, "row id must be unchanged");
}

/// Salary persists and round-trips through `upsert_for_origin` and
/// export/import, and a second upsert with unknown salary (`None`) never
/// clobbers an already-known value (COALESCE(new, old) merge).
#[test]
fn salary_fields_persist_merge_and_export_import_round_trip() {
    let (_dir, store) = open_store();

    let with_salary = ApplicationMeta {
        salary_min: Some(70_000.0),
        salary_max: Some(90_000.0),
        salary_currency: Some("EUR".into()),
        ..meta("Acme", "Engineer")
    };
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/salary/1",
            "aggregator",
            &with_salary,
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.salary_min, Some(70_000.0));
    assert_eq!(app.salary_max, Some(90_000.0));
    assert_eq!(app.salary_currency, Some("EUR".to_string()));

    // A later re-track with unknown salary must NOT clobber the known values.
    let unknown_salary = meta("Acme", "Engineer");
    store
        .upsert_for_origin(
            "https://acme.com/job/salary/1",
            "aggregator",
            &unknown_salary,
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();
    let unchanged = store.get(&id).unwrap();
    assert_eq!(
        unchanged.salary_min,
        Some(70_000.0),
        "an unknown incoming salary must not clobber an already-known value"
    );
    assert_eq!(unchanged.salary_max, Some(90_000.0));
    assert_eq!(unchanged.salary_currency, Some("EUR".to_string()));

    // Export + import round-trips the fields.
    let bundle = store.export();
    let (_dir2, store2) = open_store();
    store2.import(&bundle).unwrap();
    let imported = store2.get(&id).unwrap();
    assert_eq!(imported.salary_min, Some(70_000.0));
    assert_eq!(imported.salary_max, Some(90_000.0));
    assert_eq!(imported.salary_currency, Some("EUR".to_string()));
}
