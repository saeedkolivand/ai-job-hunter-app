use super::*;
use tempfile::TempDir;

#[test]
fn test_open_store() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let prefs = store.get();
    assert!(prefs.location.is_none());
    assert!(prefs.remote.is_none());
}

#[test]
fn test_get_default() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let prefs = store.get();

    assert_eq!(prefs.location, None);
    assert_eq!(prefs.remote, None);
    assert_eq!(prefs.seniority, None);
    assert_eq!(prefs.salary_min, None);
    assert_eq!(prefs.salary_max, None);
    assert_eq!(prefs.tech_stack, None);
}

#[test]
fn test_set_and_get() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let prefs = JobPreferences {
        location: Some("Berlin".to_string()),
        remote: Some("hybrid".to_string()),
        seniority: Some("senior".to_string()),
        salary_min: Some(80000),
        salary_max: Some(120000),
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
    assert_eq!(retrieved.remote, Some("hybrid".to_string()));
    assert_eq!(retrieved.seniority, Some("senior".to_string()));
    assert_eq!(retrieved.salary_min, Some(80000));
    assert_eq!(retrieved.salary_max, Some(120000));
    assert_eq!(retrieved.tech_stack.as_ref().unwrap().len(), 2);
}

#[test]
fn test_tech_stack_serialization() {
    let temp_dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let prefs = JobPreferences {
        location: None,
        remote: None,
        seniority: None,
        salary_min: None,
        salary_max: None,
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

    // Set initial preferences
    let prefs1 = JobPreferences {
        location: Some("Berlin".to_string()),
        remote: Some("remote".to_string()),
        seniority: Some("senior".to_string()),
        salary_min: Some(80000),
        salary_max: Some(120000),
        tech_stack: None,
    };
    store.set(&prefs1).unwrap();

    // Update only some fields
    let prefs2 = JobPreferences {
        location: Some("Munich".to_string()),
        remote: Some("hybrid".to_string()),
        seniority: None,
        salary_min: None,
        salary_max: None,
        tech_stack: None,
    };
    store.set(&prefs2).unwrap();

    let retrieved = store.get();
    assert_eq!(retrieved.location, Some("Munich".to_string()));
    assert_eq!(retrieved.remote, Some("hybrid".to_string()));
    // Unchanged fields should be None since we set them to None
    assert_eq!(retrieved.seniority, None);
}
