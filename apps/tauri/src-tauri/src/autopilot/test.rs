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
        boards: vec!["linkedin".to_string()],
        query: "software engineer".to_string(),
        location: Some("Berlin".to_string()),
        country_code: None,
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
fn legacy_auto_apply_records_load_and_strip_dead_keys_on_save() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let dir = temp.path().to_path_buf();

    // A record persisted before the auto-apply engine was removed: it still
    // carries the now-dropped `action` / `autoSubmit` keys. Loading must not
    // fail (serde ignores unknown fields) — the silent find-&-save migration.
    let legacy = r#"[{
        "_id": "ap-legacy",
        "name": "Legacy AP",
        "status": "active",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "action": "auto_apply",
        "schedule": "daily",
        "autoSubmit": true,
        "coverLetter": "Dear team",
        "totalFound": 4,
        "totalApplied": 2,
        "foundJobs": [],
        "createdAt": 1,
        "updatedAt": 1
    }]"#;
    std::fs::write(dir.join("autopilots.json"), legacy).unwrap();

    let store = AutopilotStore::new(&dir);
    let list = store.list();
    assert_eq!(list.len(), 1, "legacy record loads despite dropped keys");
    let ap = &list[0];
    assert_eq!(ap.id, "ap-legacy");
    assert_eq!(ap.schedule, "daily");
    assert_eq!(ap.status, AutopilotStatus::Active);
    assert_eq!(ap.cover_letter.as_deref(), Some("Dear team"));

    // Touching the record rewrites the file from the new struct — the dead
    // auto-apply keys are gone from disk going forward.
    store.stamp_last_run("ap-legacy");
    let on_disk = std::fs::read_to_string(dir.join("autopilots.json")).unwrap();
    assert!(!on_disk.contains("\"action\""), "action stripped on save");
    assert!(
        !on_disk.contains("autoSubmit"),
        "autoSubmit stripped on save"
    );
    assert!(
        on_disk.contains("Dear team"),
        "kept fields survive the rewrite"
    );
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
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());

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
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
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
fn test_clear_all_removes_every_autopilot() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    for name in ["AP1", "AP2"] {
        store.create(serde_json::json!({
            "name": name,
            "target": { "board": "linkedin", "query": "rust", "pages": 1 },
            "filter": { "minMatchScore": 50.0 },
            "schedule": "manual",
        }));
    }
    assert_eq!(store.list().len(), 2);

    store.clear_all();
    assert!(store.list().is_empty());
}

#[test]
fn mark_interrupted_runs_flips_only_in_progress() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());

    let make = |name: &str| {
        store.create(serde_json::json!({
            "name": name,
            "target": { "board": "linkedin", "query": "rust", "pages": 1 },
            "filter": { "minMatchScore": 50.0 },
            "schedule": "manual",
        }))
    };
    let running = make("running");
    let done = make("done");

    store.set_run_status(&running.id, RunStatus::InProgress);
    store.set_run_status(&done.id, RunStatus::Completed);

    let reconciled = store.mark_interrupted_runs();
    assert_eq!(reconciled, 1, "only the in-progress run is reconciled");

    let status = |id: &str| store.get(id).unwrap().run_status;
    assert_eq!(status(&running.id), Some(RunStatus::Interrupted));
    assert_eq!(status(&done.id), Some(RunStatus::Completed));

    // Idempotent: a second startup sweep finds nothing to reconcile.
    assert_eq!(store.mark_interrupted_runs(), 0);
}

#[test]
fn record_run_marks_the_run_completed() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    let ap = store.create(serde_json::json!({
        "name": "ap",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "manual",
    }));
    store.set_run_status(&ap.id, RunStatus::InProgress);

    store.record_run(&ap.id, 3, 0, Vec::new());
    assert_eq!(
        store.get(&ap.id).unwrap().run_status,
        Some(RunStatus::Completed)
    );
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

#[test]
fn save_skips_disk_write_when_serialized_state_is_unchanged() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    store.create(serde_json::json!({
        "name": "AP",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "manual",
    }));

    // After `create`, disk already holds exactly the serialized JSON, so the
    // dirty check compares equal and must skip the write. Probe with mtime: a
    // skipped write never touches the file (mtime frozen); a real rewrite would
    // bump it. Re-save the identical, unchanged map and assert mtime is stable.
    let file = temp.path().join("autopilots.json");
    let before = std::fs::metadata(&file).unwrap().modified().unwrap();

    let map = store.load();
    store.save(map);

    let after = std::fs::metadata(&file).unwrap().modified().unwrap();
    assert_eq!(
        before, after,
        "identical serialized state must skip the write (mtime unchanged)"
    );
    // And the state is preserved, not blanked.
    assert!(std::fs::read_to_string(&file).unwrap().contains("\"_id\""));
}

#[test]
fn save_writes_when_serialized_state_differs() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    store.create(serde_json::json!({
        "name": "AP",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "manual",
    }));

    // Overwrite the file with content that does NOT match the serialized map,
    // then save the (unchanged) map: the bytes differ, so the write must proceed
    // and replace the sentinel content with the real serialized JSON.
    let file = temp.path().join("autopilots.json");
    std::fs::write(&file, "// stale sentinel content").unwrap();

    let map = store.load();
    store.save(map);

    let after = std::fs::read_to_string(&file).unwrap();
    assert!(
        !after.contains("stale sentinel"),
        "differing on-disk content must trigger a write"
    );
    assert!(
        after.contains("\"_id\""),
        "real serialized state was written"
    );
}

#[test]
fn save_writes_when_file_is_missing() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    store.create(serde_json::json!({
        "name": "AP",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "manual",
    }));

    // A missing/unreadable file never matches the serialized bytes → the write
    // must proceed so state isn't lost on the first persist after deletion.
    let file = temp.path().join("autopilots.json");
    std::fs::remove_file(&file).unwrap();
    assert!(!file.exists());

    let map = store.load();
    store.save(map);

    assert!(file.exists(), "missing file is (re)written, not skipped");
    let after = std::fs::read_to_string(&file).unwrap();
    assert!(after.contains("\"_id\""), "serialized state was written");
}

// ── AutopilotTarget boards back-compat deserialization ────────────────────────

#[test]
fn target_deserializes_legacy_board_string() {
    // Old on-disk format: `"board": "linkedin"` (singular string field).
    // The `#[serde(alias = "board", deserialize_with = "string_or_vec")]` must
    // normalise this to `boards: vec!["linkedin"]`.
    let json = r#"{"board": "linkedin", "query": "rust", "pages": 2}"#;
    let target: AutopilotTarget =
        serde_json::from_str(json).expect("legacy format must deserialize");
    assert_eq!(target.boards, vec!["linkedin"]);
}

#[test]
fn target_deserializes_new_boards_array() {
    // New format: `"boards": ["linkedin","remotive"]`.
    let json = r#"{"boards": ["linkedin","remotive"], "query": "rust", "pages": 2}"#;
    let target: AutopilotTarget = serde_json::from_str(json).expect("new format must deserialize");
    assert_eq!(target.boards, vec!["linkedin", "remotive"]);
}

#[test]
fn target_round_trips_as_boards_array() {
    // Serializing always writes `boards` (the canonical field name), so a
    // re-loaded record uses the new format — no legacy drift.
    let target = AutopilotTarget {
        boards: vec!["linkedin".to_string(), "remotive".to_string()],
        query: "rust".to_string(),
        location: None,
        country_code: None,
        work_type: None,
        pages: 2,
        date_filter: None,
        top_n: 3,
    };
    let serialized = serde_json::to_string(&target).unwrap();
    assert!(
        serialized.contains("\"boards\""),
        "must serialize as boards array"
    );
    assert!(
        !serialized.contains("\"board\""),
        "must not serialize as legacy singular"
    );

    let restored: AutopilotTarget = serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored.boards, vec!["linkedin", "remotive"]);
}

// ── AutopilotTarget country_code serde ───────────────────────────────────────

#[test]
fn target_country_code_absent_deserializes_to_none() {
    // Backward-compat: a persisted autopilot that pre-dates the country_code field
    // (i.e. the JSON simply omits "countryCode") must still deserialize cleanly and
    // yield country_code: None. This guarantees old autopilots continue to load.
    let json = r#"{
        "boards": ["aggregator"],
        "query": "rust developer",
        "location": "London",
        "pages": 2,
        "topN": 3
    }"#;
    let target: AutopilotTarget =
        serde_json::from_str(json).expect("missing countryCode must not fail deserialization");
    assert!(
        target.country_code.is_none(),
        "absent countryCode field must deserialize to None"
    );
}

#[test]
fn target_country_code_round_trips_and_none_is_omitted() {
    // Round-trip: Some("us") survives serialize → deserialize.
    // Absence (None) must be omitted from JSON entirely (skip_serializing_if).
    let with_code = AutopilotTarget {
        boards: vec!["aggregator".to_string()],
        query: "frontend engineer".to_string(),
        location: None,
        country_code: Some("us".to_string()),
        work_type: None,
        pages: 1,
        date_filter: None,
        top_n: 3,
    };
    let json = serde_json::to_string(&with_code).unwrap();
    // camelCase rename_all means the field is "countryCode" on the wire.
    assert!(
        json.contains("\"countryCode\":\"us\""),
        "country_code Some(\"us\") must serialize as camelCase countryCode"
    );
    let restored: AutopilotTarget = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.country_code, Some("us".to_string()));

    // None must be omitted — not written as null or empty string.
    let without_code = AutopilotTarget {
        boards: vec!["aggregator".to_string()],
        query: "frontend engineer".to_string(),
        location: None,
        country_code: None,
        work_type: None,
        pages: 1,
        date_filter: None,
        top_n: 3,
    };
    let json_none = serde_json::to_string(&without_code).unwrap();
    assert!(
        !json_none.contains("countryCode"),
        "country_code None must be omitted from serialized JSON (skip_serializing_if)"
    );
}

// ── found_job helper ──────────────────────────────────────────────────────────

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

#[test]
fn record_run_reports_only_newly_surfaced_jobs() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    let ap = store.create(serde_json::json!({
        "name": "AP",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 50.0 },
        "schedule": "manual",
    }));
    let id = ap.id;

    // First run — both URLs are brand new → drives a "2 new jobs" notification.
    assert_eq!(
        store.record_run(&id, 2, 0, vec![found_job("u1", 1), found_job("u2", 2)]),
        2
    );
    // Re-run with the two seen URLs + one unseen → only the unseen counts.
    assert_eq!(
        store.record_run(
            &id,
            3,
            0,
            vec![found_job("u1", 9), found_job("u2", 9), found_job("u3", 9)]
        ),
        1
    );
    // Nothing unseen → no notification.
    assert_eq!(store.record_run(&id, 3, 0, vec![found_job("u1", 9)]), 0);
    // Unknown autopilot → 0 (no panic).
    assert_eq!(
        store.record_run("missing", 5, 0, vec![found_job("x", 1)]),
        0
    );
}
