use super::{support::*, *};

#[test]
fn test_open_store() {
    let (_dir, store) = open_store();
    let prefs = store.get();
    assert!(prefs.location.is_none());
    assert!(prefs.tech_stack.is_none());
}

#[test]
fn test_get_default() {
    let (_dir, store) = open_store();
    let prefs = store.get();

    assert_eq!(prefs.location, None);
    assert_eq!(prefs.tech_stack, None);
}

#[test]
fn test_clear_resets_to_empty() {
    let (_dir, store) = open_store();
    store
        .set(&JobPreferences {
            location: Some("Berlin".to_string()),
            tech_stack: Some(vec![TechStackItem {
                name: "Rust".to_string(),
                category: "language".to_string(),
            }]),
            salary_expectation: Some("€75,000".to_string()),
            extra_agency_companies: Some(vec!["Hays".to_string()]),
            ..blank()
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
    let (_dir, store) = open_store();

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
        ..blank()
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
    let (_dir, store) = open_store();

    let prefs = JobPreferences {
        tech_stack: Some(vec![TechStackItem {
            name: "TypeScript".to_string(),
            category: "language".to_string(),
        }]),
        ..blank()
    };

    store.set(&prefs).unwrap();
    let retrieved = store.get();

    assert_eq!(retrieved.tech_stack.unwrap()[0].name, "TypeScript");
}

#[test]
fn test_partial_update() {
    let (_dir, store) = open_store();

    // Set initial preferences.
    let prefs1 = JobPreferences {
        location: Some("Berlin".to_string()),
        country_code: Some("de".to_string()),
        tech_stack: Some(vec![TechStackItem {
            name: "Rust".to_string(),
            category: "language".to_string(),
        }]),
        salary_expectation: Some("€75,000".to_string()),
        ..blank()
    };
    store.set(&prefs1).unwrap();

    // Overwrite with a sparser shape.
    let prefs2 = JobPreferences {
        location: Some("Munich".to_string()),
        ..blank()
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

// ── semantic_scoring mirror (ADR-020 addendum) ────────────────────────────────

/// The default is the load-bearing half: every install that predates this
/// column (and every user who never touched the toggle) must read `false`, or
/// the headless Autopilot would silently start embedding.
#[test]
fn semantic_scoring_defaults_off_and_round_trips() {
    let (_dir, store) = open_store();

    assert!(
        !store.semantic_scoring(),
        "an unset (NULL) mirror must read false — the app-wide semanticScoring default"
    );

    store.set_semantic_scoring(true).unwrap();
    assert!(store.semantic_scoring());

    store.set_semantic_scoring(false).unwrap();
    assert!(
        !store.semantic_scoring(),
        "turning the setting back off must stick"
    );
}

/// Single-column discipline, both directions: the semantic write must not
/// disturb the other columns, and — the easier regression to introduce — a
/// full-row `set()` (a Settings edit elsewhere, or a backup restore) must not
/// silently clear the user's semantic-scoring choice.
#[test]
fn semantic_scoring_and_the_other_columns_cannot_clobber_each_other() {
    let (_dir, store) = open_store();

    store
        .set(&JobPreferences {
            location: Some("Berlin".to_string()),
            country_code: Some("DE".to_string()),
            salary_expectation: Some("€75,000".to_string()),
            ..blank()
        })
        .unwrap();
    store.set_semantic_scoring(true).unwrap();

    // The semantic write left every other column alone.
    let prefs = store.get();
    assert_eq!(prefs.location, Some("Berlin".to_string()));
    assert_eq!(prefs.salary_expectation, Some("€75,000".to_string()));

    // …and a later full-row write (which knows nothing about this column) left
    // the semantic bit alone.
    store
        .set(&JobPreferences {
            location: Some("Hamburg".to_string()),
            ..blank()
        })
        .unwrap();
    assert!(
        store.semantic_scoring(),
        "a full-row set() must not reset the semantic-scoring mirror"
    );

    // Factory reset DOES clear it (it is user data like everything else here).
    store.clear().unwrap();
    assert!(!store.semantic_scoring());
}
