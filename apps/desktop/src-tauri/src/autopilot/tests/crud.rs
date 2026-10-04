//! The store's create/update/remove surface and the input hardening at its boundary.

use super::super::*;
use super::support::*;

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
fn test_u32_field_in_range_rejects_out_of_range_and_non_numeric() {
    let v = serde_json::json!({
        "good": 23,
        "tooBig": 25,
        "minOk": 59,
        "minBad": 60,
        "negative": -1,
        "text": "9",
    });
    // In-range values pass through.
    assert_eq!(u32_field_in_range(&v, "good", 23), Some(23));
    assert_eq!(u32_field_in_range(&v, "minOk", 59), Some(59));
    // Out-of-range / non-numeric / absent → None (falls back to scheduler default).
    assert_eq!(u32_field_in_range(&v, "tooBig", 23), None);
    assert_eq!(u32_field_in_range(&v, "minBad", 59), None);
    assert_eq!(u32_field_in_range(&v, "negative", 23), None);
    assert_eq!(u32_field_in_range(&v, "text", 23), None);
    assert_eq!(u32_field_in_range(&v, "missing", 23), None);
}

#[test]
fn create_drops_out_of_range_schedule_time_so_scheduler_falls_back() {
    let (_temp, store) = temp_store();

    // A client that bypassed the Zod range check sends scheduleHour: 25 /
    // scheduleMinute: 60. Persisting those verbatim would make `local_at`
    // return None forever → the autopilot is silently never due. Instead the
    // storage boundary stores None, so the scheduler uses its safe default.
    let ap = store.create(serde_json::json!({
        "name": "Out of range",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "daily",
        "scheduleHour": 25,
        "scheduleMinute": 60,
    }));
    assert_eq!(ap.schedule_hour, None, "out-of-range hour is not persisted");
    assert_eq!(
        ap.schedule_minute, None,
        "out-of-range minute is not persisted"
    );

    // A valid time is kept as-is.
    let ok = store.create(serde_json::json!({
        "name": "Valid",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "daily",
        "scheduleHour": 18,
        "scheduleMinute": 30,
    }));
    assert_eq!(ok.schedule_hour, Some(18));
    assert_eq!(ok.schedule_minute, Some(30));
}

#[test]
fn update_rejects_out_of_range_time_while_keeping_null_clear() {
    let (_temp, store) = temp_store();
    let ap = store.create(serde_json::json!({
        "name": "AP",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "daily",
        "scheduleHour": 10,
        "scheduleMinute": 15,
    }));

    // Patching with an out-of-range hour clears it to None rather than poisoning.
    let patched = store
        .update(&ap.id, serde_json::json!({ "scheduleHour": 99 }))
        .unwrap();
    assert_eq!(patched.schedule_hour, None, "out-of-range patch → None");
    assert_eq!(patched.schedule_minute, Some(15), "untouched field kept");

    // Explicit null still clears (existing behavior preserved).
    let cleared = store
        .update(&ap.id, serde_json::json!({ "scheduleMinute": null }))
        .unwrap();
    assert_eq!(cleared.schedule_minute, None, "explicit null clears");
}

#[test]
fn update_toggling_assistant_off_clears_the_provider_snapshot() {
    let (_temp, store) = temp_store();
    let ap = store.create(serde_json::json!({
        "name": "AP",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "daily",
        "assistant": true,
        "assistantProvider": "openai",
        "assistantModel": "gpt-4o",
        "assistantBaseUrl": "https://api.openai.com",
    }));
    assert!(ap.assistant);
    assert_eq!(ap.assistant_provider.as_deref(), Some("openai"));
    assert_eq!(ap.assistant_model.as_deref(), Some("gpt-4o"));
    assert_eq!(
        ap.assistant_base_url.as_deref(),
        Some("https://api.openai.com")
    );

    // The renderer omits assistantProvider/Model/BaseUrl when toggling off, so a
    // patch with only `assistant: false` must clear all three itself.
    let updated = store
        .update(&ap.id, serde_json::json!({ "assistant": false }))
        .unwrap();
    assert!(!updated.assistant);
    assert!(
        updated.assistant_provider.is_none(),
        "stale provider snapshot must be cleared on toggle-off"
    );
    assert!(
        updated.assistant_model.is_none(),
        "stale model snapshot must be cleared on toggle-off"
    );
    assert!(
        updated.assistant_base_url.is_none(),
        "stale base-url snapshot must be cleared on toggle-off"
    );

    // Re-enabling with a fresh snapshot still sets it (the enable path is intact).
    let reenabled = store
        .update(
            &ap.id,
            serde_json::json!({
                "assistant": true,
                "assistantProvider": "anthropic",
                "assistantModel": "claude-3-5-sonnet",
            }),
        )
        .unwrap();
    assert!(reenabled.assistant);
    assert_eq!(reenabled.assistant_provider.as_deref(), Some("anthropic"));
    assert_eq!(
        reenabled.assistant_model.as_deref(),
        Some("claude-3-5-sonnet")
    );
}

#[test]
fn test_clear_all_removes_every_autopilot() {
    let (_temp, store) = temp_store();
    for name in ["AP1", "AP2"] {
        create_ap(&store, name, "linkedin", 50.0, "manual");
    }
    assert_eq!(store.list().len(), 2);

    store.clear_all();
    assert!(store.list().is_empty());
}

#[test]
fn test_data_store_export_import_preserves_id() {
    use crate::data_store::DataStore;

    let (_temp, store) = temp_store();
    let created = create_ap(&store, "Test AP", "linkedin", 50.0, "manual");
    let id = created.id.clone();

    let bundle = store.export();

    let (_temp2, restored) = temp_store();
    let n = restored.import(&bundle).unwrap();

    assert_eq!(n, 1);
    let list = restored.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id); // id preserved across restore
    assert_eq!(list[0].name, "Test AP");
}

// ── AutopilotStore::create filter fallback ────────────────────────────────────

#[test]
fn create_with_missing_filter_defaults_min_match_score_to_zero() {
    // When `filter` is absent (or null) the store must default min_match_score
    // to 0.0 — NOT 50.0. A 50.0 default silently drops most scraped jobs.
    let (_temp, store) = temp_store();

    let ap = store.create(serde_json::json!({
        "name": "No filter",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        // `filter` key completely omitted
        "schedule": "daily",
    }));
    assert_eq!(
        ap.filter.min_match_score, 0.0,
        "absent filter must default to min_match_score 0.0, not 50.0"
    );
    assert!(ap.filter.keywords.is_none());
    assert!(ap.filter.exclude_keywords.is_none());
}

#[test]
fn create_with_null_filter_defaults_min_match_score_to_zero() {
    let (_temp, store) = temp_store();

    let ap = store.create(serde_json::json!({
        "name": "Null filter",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": null,
        "schedule": "daily",
    }));
    assert_eq!(
        ap.filter.min_match_score, 0.0,
        "null filter must default to min_match_score 0.0"
    );
}
