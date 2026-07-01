use super::*;
use tempfile::TempDir;

#[test]
fn test_open_store() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let prefs = store.get();
    assert!(prefs.location.is_none());
    assert!(prefs.tech_stack.is_none());
}

#[test]
fn test_get_default() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let prefs = store.get();

    assert_eq!(prefs.location, None);
    assert_eq!(prefs.tech_stack, None);
}

#[test]
fn test_clear_resets_to_empty() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
    store
        .set(&JobPreferences {
            location: Some("Berlin".to_string()),
            tech_stack: Some(vec![TechStackItem {
                name: "Rust".to_string(),
                category: "language".to_string(),
            }]),
        })
        .unwrap();
    assert!(store.get().location.is_some());

    store.clear().unwrap();
    let prefs = store.get();
    assert_eq!(prefs.location, None);
    assert_eq!(prefs.tech_stack, None);
}

#[test]
fn test_set_and_get() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let prefs = JobPreferences {
        location: Some("Berlin".to_string()),
        tech_stack: Some(vec![
            TechStackItem {
                name: "Rust".to_string(),
                category: "language".to_string(),
            },
            TechStackItem {
                name: "React".to_string(),
                category: "frontend".to_string(),
            },
        ]),
    };

    store.set(&prefs).unwrap();
    let retrieved = store.get();

    assert_eq!(retrieved.location, Some("Berlin".to_string()));
    assert_eq!(retrieved.tech_stack.as_ref().unwrap().len(), 2);
}

#[test]
fn test_tech_stack_serialization() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let prefs = JobPreferences {
        location: None,
        tech_stack: Some(vec![TechStackItem {
            name: "TypeScript".to_string(),
            category: "language".to_string(),
        }]),
    };

    store.set(&prefs).unwrap();
    let retrieved = store.get();

    assert_eq!(retrieved.tech_stack.unwrap()[0].name, "TypeScript");
}

#[test]
fn test_partial_update() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Set initial preferences.
    let prefs1 = JobPreferences {
        location: Some("Berlin".to_string()),
        tech_stack: Some(vec![TechStackItem {
            name: "Rust".to_string(),
            category: "language".to_string(),
        }]),
    };
    store.set(&prefs1).unwrap();

    // Overwrite with a sparser shape.
    let prefs2 = JobPreferences {
        location: Some("Munich".to_string()),
        tech_stack: None,
    };
    store.set(&prefs2).unwrap();

    let retrieved = store.get();
    assert_eq!(retrieved.location, Some("Munich".to_string()));
    // A field set to None overwrites the prior value (full-row UPDATE semantics).
    assert_eq!(retrieved.tech_stack, None);
}

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

    // Re-open through the store, which runs the pending v2 migration.
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Kept fields round-trip.
    let prefs = store.get();
    assert_eq!(prefs.location, Some("Berlin".to_string()));
    let ts = prefs
        .tech_stack
        .expect("tech_stack must survive the migration");
    assert_eq!(ts.len(), 1);
    assert_eq!(ts[0].name, "Rust");

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
}
