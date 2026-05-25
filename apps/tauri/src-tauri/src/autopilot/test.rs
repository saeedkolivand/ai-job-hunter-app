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
