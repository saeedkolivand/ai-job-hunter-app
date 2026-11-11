use super::*;

#[test]
fn date_filter_accepts_every_known_value_and_none() {
    assert!(check_date_filter(None).is_ok());
    for v in DATE_FILTER_OPTIONS {
        assert!(check_date_filter(Some(v)).is_ok(), "{v}");
    }
}

#[test]
fn date_filter_rejects_unknown_value_naming_allowed_ones() {
    let err = check_date_filter(Some("past_week"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("dateFilter") && err.contains("week") && err.contains("24h"));
}

#[test]
fn own_resume_wins_when_non_blank() {
    assert_eq!(
        effective_resume(Some("mine"), Some("doc")).as_deref(),
        Some("mine")
    );
}

#[test]
fn blank_or_absent_own_resume_falls_back_to_default_document() {
    assert_eq!(effective_resume(None, Some("doc")).as_deref(), Some("doc"));
    assert_eq!(
        effective_resume(Some("  \n"), Some("doc")).as_deref(),
        Some("doc")
    );
}

#[test]
fn no_resume_anywhere_is_none() {
    assert_eq!(effective_resume(Some(" "), Some("")), None);
    assert_eq!(effective_resume(None, None), None);
}

fn blank_autopilot() -> crate::autopilot::Autopilot {
    serde_json::from_value(serde_json::json!({
        "_id": "a1", "name": "n", "status": "active",
        "target": {"boards": ["linkedin"], "query": "rust", "pages": 1},
        "filter": {"minMatchScore": 0.0}, "schedule": "manual",
        "totalFound": 0, "totalApplied": 0, "foundJobs": [],
        "createdAt": 0, "updatedAt": 0
    }))
    .expect("minimal autopilot record")
}

#[test]
fn reply_carries_warning_only_when_given() {
    let with = super::super::reply_with_warning(blank_autopilot(), Some(NO_RESUME_WARNING));
    assert_eq!(with["warning"], NO_RESUME_WARNING);
    let without = super::super::reply_with_warning(blank_autopilot(), None);
    assert!(without.get("warning").is_none());
    assert_eq!(without["_id"], "a1");
}

/// No `tauri::test` harness exists, so pin the command wiring from source (the `plumbing.rs`
/// precedent): deleting a `check_date_filter` call or the unknown-id `Null` branch fails here.
#[test]
fn commands_validate_date_filter_and_return_null_for_unknown_id() {
    let src = include_str!("../mod.rs");
    let body = |name: &str| {
        let start = src.find(name).expect(name);
        let rest = &src[start..];
        rest[..rest.find("\n}").expect("fn end")].to_string()
    };
    let create = body("pub async fn autopilot_create");
    assert!(create.contains("guards::check_date_filter(req.target.date_filter"));
    let update = body("pub async fn autopilot_update");
    assert!(update.contains("guards::check_date_filter(req.target"));
    assert!(update.contains("Value::Null"));
}
