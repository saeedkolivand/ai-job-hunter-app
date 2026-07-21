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
            country_code: None,
            tech_stack: Some(vec![TechStackItem {
                name: "Rust".to_string(),
                category: "language".to_string(),
            }]),
            salary_expectation: Some("€75,000".to_string()),
            extra_agency_companies: Some(vec!["Hays".to_string()]),
        })
        .unwrap();
    assert!(store.get().location.is_some());

    store.clear().unwrap();
    let prefs = store.get();
    assert_eq!(prefs.location, None);
    assert_eq!(prefs.tech_stack, None);
    assert_eq!(prefs.salary_expectation, None);
    assert_eq!(prefs.extra_agency_companies, None);
}

#[test]
fn test_set_and_get() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let prefs = JobPreferences {
        location: Some("Berlin".to_string()),
        country_code: Some("de".to_string()),
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
        salary_expectation: Some("€75,000".to_string()),
        extra_agency_companies: None,
    };

    store.set(&prefs).unwrap();
    let retrieved = store.get();

    assert_eq!(retrieved.location, Some("Berlin".to_string()));
    assert_eq!(retrieved.country_code, Some("de".to_string()));
    assert_eq!(retrieved.tech_stack.as_ref().unwrap().len(), 2);
    assert_eq!(retrieved.salary_expectation, Some("€75,000".to_string()));
}

#[test]
fn test_tech_stack_serialization() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let prefs = JobPreferences {
        location: None,
        country_code: None,
        tech_stack: Some(vec![TechStackItem {
            name: "TypeScript".to_string(),
            category: "language".to_string(),
        }]),
        salary_expectation: None,
        extra_agency_companies: None,
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
        country_code: Some("de".to_string()),
        tech_stack: Some(vec![TechStackItem {
            name: "Rust".to_string(),
            category: "language".to_string(),
        }]),
        salary_expectation: Some("€75,000".to_string()),
        extra_agency_companies: None,
    };
    store.set(&prefs1).unwrap();

    // Overwrite with a sparser shape.
    let prefs2 = JobPreferences {
        location: Some("Munich".to_string()),
        country_code: None,
        tech_stack: None,
        salary_expectation: None,
        extra_agency_companies: None,
    };
    store.set(&prefs2).unwrap();

    let retrieved = store.get();
    assert_eq!(retrieved.location, Some("Munich".to_string()));
    // A field set to None overwrites the prior value (full-row UPDATE semantics).
    assert_eq!(retrieved.tech_stack, None);
    assert_eq!(
        retrieved.country_code, None,
        "country_code must also be overwritten to None (full-row UPDATE semantics)"
    );
    assert_eq!(
        retrieved.salary_expectation, None,
        "salary_expectation must also be overwritten to None (full-row UPDATE semantics)"
    );
}

// ── salary_expectation (Task #30) ─────────────────────────────────────────────

#[test]
fn test_salary_expectation_round_trips() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    store
        .set(&JobPreferences {
            location: None,
            country_code: None,
            tech_stack: None,
            salary_expectation: Some("80k DOE".to_string()),
            extra_agency_companies: None,
        })
        .unwrap();

    assert_eq!(store.get().salary_expectation, Some("80k DOE".to_string()));
}

#[test]
fn test_salary_expectation_defaults_to_none() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
    assert_eq!(store.get().salary_expectation, None);
}

/// A pathological/oversized value is clamped server-side, never trusted as
/// the only write path (mirrors the byte caps `extension_bridge`'s own verbs
/// enforce on untrusted strings).
#[test]
fn test_salary_expectation_is_byte_clamped() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // A multi-byte (UTF-8) string well over the 200-byte cap.
    let oversized: String = "€".repeat(150); // 150 * 2 bytes = 300 bytes
    store
        .set(&JobPreferences {
            location: None,
            country_code: None,
            tech_stack: None,
            salary_expectation: Some(oversized),
            extra_agency_companies: None,
        })
        .unwrap();

    let stored = store.get().salary_expectation.unwrap();
    assert!(
        stored.len() <= MAX_SALARY_EXPECTATION_BYTES,
        "stored value must be clamped to the byte cap, got {} bytes",
        stored.len()
    );
    // Never split a multi-byte char — the clamped string must stay valid UTF-8
    // (guaranteed by `String`'s invariant; this call would panic otherwise).
    assert!(stored.is_char_boundary(stored.len()));
}

// ── set_salary_expectation (review fix, PR #695 — single-column write) ───────

/// The whole point of `set_salary_expectation`: unlike `set()`'s full-row
/// write, it must NEVER touch location/tech_stack/country_code — proven here
/// by seeding all three, then calling ONLY `set_salary_expectation` (as if a
/// caller's `useJobPreferences` query hadn't loaded yet, so it has no fresh
/// copy of those fields to spread) and asserting they all survive untouched.
#[test]
fn test_set_salary_expectation_never_clears_other_fields() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    store
        .set(&JobPreferences {
            location: Some("Berlin".to_string()),
            country_code: Some("de".to_string()),
            tech_stack: Some(vec![TechStackItem {
                name: "Rust".to_string(),
                category: "language".to_string(),
            }]),
            salary_expectation: None,
            extra_agency_companies: None,
        })
        .unwrap();

    store
        .set_salary_expectation(Some("€75,000".to_string()))
        .unwrap();

    let retrieved = store.get();
    assert_eq!(
        retrieved.location,
        Some("Berlin".to_string()),
        "location must survive a salary-only set"
    );
    assert_eq!(
        retrieved.country_code,
        Some("de".to_string()),
        "country_code must survive a salary-only set"
    );
    assert_eq!(
        retrieved.tech_stack.as_ref().map(Vec::len),
        Some(1),
        "tech_stack must survive a salary-only set"
    );
    assert_eq!(retrieved.salary_expectation, Some("€75,000".to_string()));
}

#[test]
fn test_set_salary_expectation_can_clear_to_none_without_touching_other_fields() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    store
        .set(&JobPreferences {
            location: Some("Munich".to_string()),
            country_code: None,
            tech_stack: None,
            salary_expectation: Some("€75,000".to_string()),
            extra_agency_companies: None,
        })
        .unwrap();

    store.set_salary_expectation(None).unwrap();

    let retrieved = store.get();
    assert_eq!(retrieved.location, Some("Munich".to_string()));
    assert_eq!(retrieved.salary_expectation, None);
}

#[test]
fn test_set_salary_expectation_is_byte_clamped() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let oversized: String = "€".repeat(150); // 300 bytes
    store.set_salary_expectation(Some(oversized)).unwrap();

    let stored = store.get().salary_expectation.unwrap();
    assert!(stored.len() <= MAX_SALARY_EXPECTATION_BYTES);
    assert!(stored.is_char_boundary(stored.len()));
}

// ── extra_agency_companies (ADR-029 §i) ───────────────────────────────────────

#[test]
fn test_extra_agency_companies_round_trip() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    store
        .set(&JobPreferences {
            location: None,
            country_code: None,
            tech_stack: None,
            salary_expectation: None,
            extra_agency_companies: Some(vec![
                "Talent Partners".to_string(),
                "Local Recruiters".to_string(),
            ]),
        })
        .unwrap();

    assert_eq!(
        store.get().extra_agency_companies,
        Some(vec![
            "Talent Partners".to_string(),
            "Local Recruiters".to_string()
        ])
    );
}

#[test]
fn test_extra_agency_companies_clamps_and_drops_blanks() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let oversized = "€".repeat(150); // 300 bytes, over the per-entry cap
    store
        .set(&JobPreferences {
            location: None,
            country_code: None,
            tech_stack: None,
            salary_expectation: None,
            extra_agency_companies: Some(vec![
                "  Padded Agency  ".to_string(), // trimmed
                "   ".to_string(),               // blank → dropped
                oversized,                       // byte-clamped
            ]),
        })
        .unwrap();

    let stored = store.get().extra_agency_companies.unwrap();
    assert_eq!(stored.len(), 2, "blank entries must be dropped");
    assert_eq!(stored[0], "Padded Agency", "entries are trimmed");
    assert!(
        stored[1].len() <= MAX_AGENCY_COMPANY_BYTES,
        "oversized entry must be byte-clamped"
    );
}

#[test]
fn test_extra_agency_companies_list_length_is_capped() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // A list well over the length cap → truncated to MAX_EXTRA_AGENCY_COMPANIES,
    // bounding the single JSON column against a looping/XSS'd renderer.
    let many: Vec<String> = (0..MAX_EXTRA_AGENCY_COMPANIES + 25)
        .map(|i| format!("agency{i}"))
        .collect();
    store.set_extra_agency_companies(Some(many)).unwrap();

    let stored = store.get().extra_agency_companies.unwrap();
    assert_eq!(
        stored.len(),
        MAX_EXTRA_AGENCY_COMPANIES,
        "the extra-agency list must be capped at the server limit"
    );
}

#[test]
fn test_extra_agency_companies_empty_list_stores_as_none() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    store
        .set(&JobPreferences {
            location: None,
            country_code: None,
            tech_stack: None,
            salary_expectation: None,
            extra_agency_companies: Some(vec!["   ".to_string()]),
        })
        .unwrap();

    assert_eq!(
        store.get().extra_agency_companies,
        None,
        "an all-blank list collapses to None (SQL NULL), not an empty array"
    );
}

/// Single-column write must never clobber the other fields (PR #695 pattern).
#[test]
fn test_set_extra_agency_companies_never_clears_other_fields() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    store
        .set(&JobPreferences {
            location: Some("Berlin".to_string()),
            country_code: Some("de".to_string()),
            tech_stack: None,
            salary_expectation: Some("€75,000".to_string()),
            extra_agency_companies: None,
        })
        .unwrap();

    store
        .set_extra_agency_companies(Some(vec!["Hays".to_string()]))
        .unwrap();

    let retrieved = store.get();
    assert_eq!(retrieved.location, Some("Berlin".to_string()));
    assert_eq!(retrieved.country_code, Some("de".to_string()));
    assert_eq!(retrieved.salary_expectation, Some("€75,000".to_string()));
    assert_eq!(
        retrieved.extra_agency_companies,
        Some(vec!["Hays".to_string()])
    );
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
