use super::{support::*, *};

// ── Migration: drop_unused_job_preferences_columns ────────────────────────────
//
// The v2 migration recreates `job_preferences` with only `id`, `location`, and
// `tech_stack`. Simulate a v1 database (the original 6-column table with data in
// the now-removed columns), run the store's migrations, and assert the dropped
// columns are gone while `location` + `tech_stack` survive.

#[test]
fn test_migration_drops_unused_columns_and_preserves_kept_fields() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("job_preferences.db");

    // Build a legacy v1 schema by hand and seed every column.
    {
        let conn = crate::db::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE job_preferences (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                location TEXT,
                remote TEXT,
                seniority TEXT,
                salary_min INTEGER,
                salary_max INTEGER,
                tech_stack TEXT
            );
            INSERT INTO job_preferences
                (id, location, remote, seniority, salary_min, salary_max, tech_stack)
                VALUES
                (1, 'Berlin', 'remote', 'senior', 80000, 120000,
                 '[{\"name\":\"Rust\",\"category\":\"language\"}]');
            PRAGMA user_version = 1;",
        )
        .unwrap();
    }

    // Re-open through the store, which runs the pending v2..v5 migrations.
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Kept fields round-trip.
    let prefs = store.get();
    assert_eq!(prefs.location, Some("Berlin".to_string()));
    let ts = prefs
        .tech_stack
        .expect("tech_stack must survive the migration");
    assert_eq!(ts.len(), 1);
    assert_eq!(ts[0].name, "Rust");
    // v3 (`add_job_preferences_country_code`) adds a brand-new column to a v1 DB
    // that never had one — must default to None, not error on the missing column.
    assert_eq!(
        prefs.country_code, None,
        "country_code must default to None on a legacy DB with no such column"
    );
    // v4 (`add_job_preferences_salary_expectation`) — same defaults-to-None
    // discipline for a second brand-new column on the same legacy v1 DB.
    assert_eq!(
        prefs.salary_expectation, None,
        "salary_expectation must default to None on a legacy DB with no such column"
    );
    // v5 (`add_job_preferences_extra_agency_companies`) — same defaults-to-None
    // discipline for a third brand-new column on the same legacy v1 DB.
    assert_eq!(
        prefs.extra_agency_companies, None,
        "extra_agency_companies must default to None on a legacy DB with no such column"
    );

    // Dropped columns are gone from the schema.
    let conn = store.conn.lock();
    assert!(
        !crate::db::column_exists(&conn, "job_preferences", "remote"),
        "remote column must be dropped"
    );
    assert!(
        !crate::db::column_exists(&conn, "job_preferences", "seniority"),
        "seniority column must be dropped"
    );
    assert!(
        !crate::db::column_exists(&conn, "job_preferences", "salary_min"),
        "salary_min column must be dropped"
    );
    assert!(
        !crate::db::column_exists(&conn, "job_preferences", "salary_max"),
        "salary_max column must be dropped"
    );
    // Kept columns remain.
    assert!(crate::db::column_exists(
        &conn,
        "job_preferences",
        "location"
    ));
    assert!(crate::db::column_exists(
        &conn,
        "job_preferences",
        "tech_stack"
    ));
    // v3 column added on top of the legacy v1 → v2 chain.
    assert!(crate::db::column_exists(
        &conn,
        "job_preferences",
        "country_code"
    ));
    // v4 column added on top of the same chain.
    assert!(crate::db::column_exists(
        &conn,
        "job_preferences",
        "salary_expectation"
    ));
    // v5 column added on top of the same chain.
    assert!(crate::db::column_exists(
        &conn,
        "job_preferences",
        "extra_agency_companies"
    ));
}

#[test]
fn semantic_scoring_column_exists_after_the_migration_chain() {
    let (_dir, store) = open_store();
    let conn = store.conn.lock();
    // v6 column, APPENDED to the chain (migrations are position-indexed).
    assert!(crate::db::column_exists(
        &conn,
        "job_preferences",
        "semantic_scoring"
    ));
}

/// …and the UPGRADE path, which the fresh-install test above cannot see: every
/// existing user opens a v5 database, where the column does not exist yet. A
/// fresh install runs the whole chain from zero and would still pass if
/// migration 6 were mis-numbered, mis-ordered, or wrong about the starting
/// schema — this seeds the real v5 shape and reopens through the store.
#[test]
fn a_version_5_database_gains_the_semantic_scoring_column_on_open() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    // The schema as of migration 5 (`add_job_preferences_extra_agency_companies`):
    // the dropped-column recreate has already run and the three ADD COLUMNs with
    // it. No `semantic_scoring`.
    {
        let conn = crate::db::open(&data_dir.join("job_preferences.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE job_preferences (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 location TEXT,
                 tech_stack TEXT,
                 country_code TEXT,
                 salary_expectation TEXT,
                 extra_agency_companies TEXT
             );
             INSERT OR IGNORE INTO job_preferences (id, location) VALUES (1, 'Berlin');
             PRAGMA user_version = 5;",
        )
        .unwrap();
        assert!(
            !crate::db::column_exists(&conn, "job_preferences", "semantic_scoring"),
            "fixture precondition: a v5 database has no semantic_scoring column"
        );
    }

    let store = JobPreferencesStore::open(&data_dir).expect("the v5 → v6 upgrade must succeed");

    assert!(
        crate::db::column_exists(&store.conn.lock(), "job_preferences", "semantic_scoring"),
        "opening a v5 database must run migration 6"
    );
    assert!(
        !store.semantic_scoring(),
        "the added column is NULL for every upgraded install, and NULL must read \
         false — the app-wide default that keeps a scheduled run embedding-free \
         until the user opts in"
    );
    assert_eq!(
        store.get().location,
        Some("Berlin".to_string()),
        "…and the upgrade is an ADD COLUMN, so the user's existing row survives it"
    );
}
