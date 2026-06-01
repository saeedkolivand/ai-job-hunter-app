use super::*;

#[test]
fn test_default_top_n() {
    assert_eq!(default_top_n(), 3);
}

#[test]
fn test_autopilot_status_partial_eq() {
    assert_eq!(AutopilotStatus::Active, AutopilotStatus::Active);
    assert_ne!(AutopilotStatus::Active, AutopilotStatus::Paused);
}

#[test]
fn test_autopilot_target_serialization() {
    let target = AutopilotTarget {
        board: "linkedin".to_string(),
        query: "software engineer".to_string(),
        location: Some("Berlin".to_string()),
        work_type: None,
        pages: 5,
        date_filter: None,
        top_n: 3,
    };
    let json = serde_json::to_string(&target);
    assert!(json.is_ok());
}

#[test]
fn test_autopilot_filter_serialization() {
    let filter = AutopilotFilter {
        min_match_score: 75.0,
        keywords: Some(vec!["rust".to_string(), "typescript".to_string()]),
        exclude_keywords: None,
    };
    let json = serde_json::to_string(&filter);
    assert!(json.is_ok());
}

#[test]
fn test_str_field() {
    let value = serde_json::json!({ "name": "Test", "other": "Value" });
    assert_eq!(str_field(&value, "name"), "Test");
    assert_eq!(str_field(&value, "missing"), "");
}

#[test]
fn test_now_ms() {
    let now = now_ms();
    assert!(now > 0);
}

#[test]
fn test_clear_all_removes_every_autopilot() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    for name in ["AP1", "AP2"] {
        store.create(serde_json::json!({
            "name": name,
            "target": { "board": "linkedin", "query": "rust", "pages": 1 },
            "filter": { "minMatchScore": 50.0 },
            "action": "save",
            "schedule": "manual",
        }));
    }
    assert_eq!(store.list().len(), 2);

    store.clear_all();
    assert!(store.list().is_empty());
}

#[test]
fn test_data_store_export_import_preserves_id() {
    use crate::data_store::DataStore;
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    let created = store.create(serde_json::json!({
        "name": "Test AP",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "action": "save",
        "schedule": "manual",
    }));
    let id = created.id.clone();

    let bundle = store.export();

    let temp2 = TempDir::new().unwrap();
    let restored = AutopilotStore::new(&temp2.path().to_path_buf());
    let n = restored.import(&bundle).unwrap();

    assert_eq!(n, 1);
    let list = restored.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id); // id preserved across restore
    assert_eq!(list[0].name, "Test AP");
}

fn found_job(url: &str, found_at: u64) -> FoundJob {
    FoundJob {
        title: "Engineer".into(),
        company: "Acme".into(),
        url: url.into(),
        location: None,
        description: None,
        score: None,
        found_at,
        is_new: false,
        applied: false,
    }
}

#[test]
fn merge_dedups_by_url_preserving_first_seen_and_flagging_new() {
    let existing = vec![found_job("https://a.com/1", 100)];
    let incoming = vec![
        found_job("https://a.com/1", 999), // re-surfaced — keep found_at=100
        found_job("https://a.com/2", 200), // genuinely new
    ];

    let merged = merge_found_jobs(&existing, incoming);

    assert_eq!(merged.len(), 2, "no duplicate row for the same url");
    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(a1.found_at, 100, "first-seen time preserved");
    assert!(!a1.is_new, "an existing job is not new");
    let a2 = merged.iter().find(|j| j.url == "https://a.com/2").unwrap();
    assert!(a2.is_new, "a never-seen url is flagged new");
}

#[test]
fn merge_is_idempotent_on_a_repeated_run() {
    let first = merge_found_jobs(&[], vec![found_job("u1", 1), found_job("u2", 2)]);
    assert!(first.iter().all(|j| j.is_new));

    // Re-running with the same postings yields the same set; only is_new clears.
    let second = merge_found_jobs(&first, vec![found_job("u1", 9), found_job("u2", 9)]);
    assert_eq!(second.len(), 2);
    assert!(second.iter().all(|j| !j.is_new));
}

#[test]
fn merge_keeps_prior_jobs_not_in_the_new_run() {
    let existing = vec![found_job("old", 1)];
    let merged = merge_found_jobs(&existing, vec![found_job("fresh", 2)]);
    assert_eq!(
        merged.len(),
        2,
        "prior finds are retained, new ones appended"
    );
    assert!(merged.iter().any(|j| j.url == "old"));
}
