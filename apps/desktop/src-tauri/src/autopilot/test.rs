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
        board: None,
        description: None,
        score: None,
        found_at,
        is_new: false,
        applied: false,
        trust: None,
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
    assert_eq!(merged.len(), 2, "prior finds retained below the new one");
    assert!(merged.iter().any(|j| j.url == "old"));
}

#[test]
fn merge_puts_newly_found_jobs_on_top() {
    let merged = merge_found_jobs(&[found_job("old", 1)], vec![found_job("fresh", 2)]);
    assert_eq!(
        merged[0].url, "fresh",
        "newly found job is first (top of list)"
    );
    assert!(merged[0].is_new);
    assert_eq!(merged[1].url, "old", "prior finds fall below the new one");
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

#[test]
fn found_job_without_board_deserializes_to_none() {
    // Old persisted FoundJob records pre-date the `board` field. The
    // `#[serde(default)]` must let them load with `board: None` rather than failing.
    let json = r#"{
        "title": "Engineer",
        "company": "Acme",
        "url": "https://a.com/1",
        "foundAt": 100
    }"#;
    let job: FoundJob = serde_json::from_str(json).expect("legacy FoundJob must deserialize");
    assert_eq!(job.board, None, "absent board must default to None");
}

#[test]
fn found_job_with_board_round_trips() {
    // A FoundJob carrying a board serializes the camelCase key and round-trips.
    let mut job = found_job("https://a.com/1", 100);
    job.board = Some("aggregator".into());
    let json = serde_json::to_string(&job).unwrap();
    assert!(
        json.contains("\"board\":\"aggregator\""),
        "board must serialize as a camelCase string; got {json}"
    );
    let restored: FoundJob = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.board, Some("aggregator".to_string()));
}

#[test]
fn merge_preserves_and_refreshes_board_across_resurface() {
    // An existing row persisted before `board` existed (None) must pick up the
    // board when the same URL re-surfaces, and a never-seen URL keeps its board.
    let mut existing = found_job("https://a.com/1", 100);
    existing.board = None; // legacy row, no provenance yet

    let mut resurfaced = found_job("https://a.com/1", 999);
    resurfaced.board = Some("linkedin".into());
    let mut fresh = found_job("https://a.com/2", 200);
    fresh.board = Some("aggregator".into());

    let merged = merge_found_jobs(&[existing], vec![resurfaced, fresh]);

    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(
        a1.board,
        Some("linkedin".to_string()),
        "re-surfaced existing row picks up the incoming board"
    );
    let a2 = merged.iter().find(|j| j.url == "https://a.com/2").unwrap();
    assert_eq!(
        a2.board,
        Some("aggregator".to_string()),
        "appended new row keeps its board (via ..inc spread)"
    );
}

#[test]
fn merge_preserves_and_refreshes_trust_across_resurface() {
    // Same legacy-migration case as the board test above: an existing row
    // persisted before `trust` existed (None) must pick up the incoming trust
    // when the same URL re-surfaces, and a never-seen URL keeps its trust.
    let mut existing = found_job("https://a.com/1", 100);
    existing.trust = None; // legacy row, no trust assessment yet

    let resurfaced_trust =
        crate::scraping::trust::assess_trust("https://linkedin.com/jobs/view/1", "Acme");
    let mut resurfaced = found_job("https://a.com/1", 999);
    resurfaced.trust = Some(resurfaced_trust.clone());
    let fresh_trust =
        crate::scraping::trust::assess_trust("https://boards.greenhouse.io/acme/jobs/2", "Acme");
    let mut fresh = found_job("https://a.com/2", 200);
    fresh.trust = Some(fresh_trust.clone());

    let merged = merge_found_jobs(&[existing], vec![resurfaced, fresh]);

    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(
        a1.trust,
        Some(resurfaced_trust),
        "re-surfaced existing row picks up the incoming trust"
    );
    let a2 = merged.iter().find(|j| j.url == "https://a.com/2").unwrap();
    assert_eq!(
        a2.trust,
        Some(fresh_trust),
        "appended new row keeps its trust (via ..inc spread)"
    );
}

// ── AutopilotStore::create filter fallback ────────────────────────────────────

#[test]
fn create_with_missing_filter_defaults_min_match_score_to_zero() {
    use tempfile::TempDir;

    // When `filter` is absent (or null) the store must default min_match_score
    // to 0.0 — NOT 50.0. A 50.0 default silently drops most scraped jobs.
    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());

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
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());

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

// ── relax_legacy_filters ──────────────────────────────────────────────────────

/// Return a fully-populated `Autopilot` that each test can mutate in place.
/// Starts with the legacy restrictive defaults (the zero-jobs configuration)
/// so most tests only need to tweak the one field they care about.
/// Zero args → well under the 8-arg clippy limit.
fn base_autopilot() -> Autopilot {
    let now = 1_000_000u64;
    Autopilot {
        id: "test-id".into(),
        name: "Test AP".into(),
        status: AutopilotStatus::Active,
        target: AutopilotTarget {
            boards: vec!["linkedin".into()],
            query: "engineer".into(),
            location: None,
            country_code: None,
            work_type: None,
            pages: 1,
            date_filter: Some("24h".into()),
            top_n: 3,
        },
        filter: AutopilotFilter {
            min_match_score: 50.0,
            keywords: Some(vec!["rust".into(), "go".into()]),
            exclude_keywords: None,
        },
        schedule: "daily".into(),
        schedule_hour: None,
        schedule_minute: None,
        resume_text: None,
        cover_letter: None,
        total_found: 0,
        total_applied: 0,
        found_jobs: Vec::new(),
        run_status: None,
        last_run_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn relax_clears_keywords_for_legacy_record() {
    // base_autopilot() is legacy (score 50 + date "24h"), so the auto-prefilled
    // keyword list is cleared. The clear is gated on legacy-ness (see
    // `relax_is_noop_on_already_relaxed_record` for the non-legacy path).
    let mut ap = base_autopilot();
    ap.filter.keywords = Some(vec!["rust".into(), "go".into()]);
    relax_legacy_filters(&mut ap);
    assert!(
        ap.filter.keywords.is_none(),
        "legacy record's prefilled keywords must be cleared to None"
    );
}

#[test]
fn relax_is_noop_on_already_relaxed_record() {
    // A record already relaxed (score 0.0, date None) is NOT legacy, so re-running
    // the migration must NOT touch keywords the user added afterwards. This is the
    // idempotency property that makes a marker-write failure (→ rerun) safe.
    let mut ap = base_autopilot();
    ap.filter.min_match_score = 0.0;
    ap.target.date_filter = None;
    ap.filter.keywords = Some(vec!["python".into()]);

    relax_legacy_filters(&mut ap);

    assert_eq!(
        ap.filter.keywords.as_deref(),
        Some(["python".to_string()].as_ref()),
        "user-added keywords on an already-relaxed record must survive a rerun"
    );
    assert_eq!(ap.filter.min_match_score, 0.0);
    assert!(ap.target.date_filter.is_none());
}

#[test]
fn relax_keeps_keywords_when_not_legacy() {
    // Pins the documented narrow gap (autopilot/mod.rs `was_legacy`): a record
    // with prefilled keywords where the user ALSO changed BOTH the score (≠50.0)
    // AND the date (≠"24h") reads as non-legacy, so its keywords are KEPT. This
    // guards against a future change to the `was_legacy` predicate silently
    // regressing the "err toward keeping user data" direction.
    let mut ap = base_autopilot();
    ap.filter.min_match_score = 30.0; // ≠ 50.0
    ap.target.date_filter = Some("week".into()); // ≠ "24h"
    ap.filter.keywords = Some(vec!["python".into()]);

    relax_legacy_filters(&mut ap);

    assert_eq!(
        ap.filter.keywords.as_deref(),
        Some(["python".to_string()].as_ref()),
        "non-legacy record (score≠50 AND date≠24h) must keep its keywords"
    );
    assert_eq!(
        ap.filter.min_match_score, 30.0,
        "non-default score must be left untouched"
    );
    assert_eq!(
        ap.target.date_filter.as_deref(),
        Some("week"),
        "non-default date_filter must be left untouched"
    );
}

#[test]
fn relax_clears_none_keywords_remains_none() {
    // keywords already None → still None (no-op, no panic).
    let mut ap = base_autopilot();
    ap.filter.keywords = None;
    relax_legacy_filters(&mut ap);
    assert!(ap.filter.keywords.is_none());
}

#[test]
fn relax_resets_min_match_score_only_when_exactly_50() {
    // 50.0 → reset to 0.0.
    let mut ap = base_autopilot();
    ap.filter.min_match_score = 50.0;
    relax_legacy_filters(&mut ap);
    assert_eq!(
        ap.filter.min_match_score, 0.0,
        "default 50.0 must be reset to 0.0"
    );
}

#[test]
fn relax_leaves_custom_min_match_score_untouched() {
    // 75.0 (deliberate user setting) → unchanged.
    let mut ap = base_autopilot();
    ap.filter.min_match_score = 75.0;
    relax_legacy_filters(&mut ap);
    assert_eq!(
        ap.filter.min_match_score, 75.0,
        "custom 75.0 must not be touched"
    );
}

#[test]
fn relax_leaves_already_zero_min_match_score_at_zero() {
    // Already 0.0 → stays 0.0 (idempotent / already relaxed).
    let mut ap = base_autopilot();
    ap.filter.min_match_score = 0.0;
    relax_legacy_filters(&mut ap);
    assert_eq!(ap.filter.min_match_score, 0.0);
}

#[test]
fn relax_leaves_near_fifty_min_match_score_untouched() {
    // 49.9 is close to but NOT the magic 50.0 → unchanged.
    let mut ap = base_autopilot();
    ap.filter.min_match_score = 49.9;
    relax_legacy_filters(&mut ap);
    assert_eq!(
        ap.filter.min_match_score, 49.9,
        "49.9 is not the legacy default; must be left unchanged"
    );
}

#[test]
fn relax_clears_date_filter_only_for_24h() {
    // "24h" is the legacy auto-default → should become None.
    let mut ap = base_autopilot();
    ap.target.date_filter = Some("24h".into());
    relax_legacy_filters(&mut ap);
    assert!(
        ap.target.date_filter.is_none(),
        "\"24h\" legacy default must be cleared to None"
    );
}

#[test]
fn relax_leaves_week_date_filter_untouched() {
    let mut ap = base_autopilot();
    ap.target.date_filter = Some("week".into());
    relax_legacy_filters(&mut ap);
    assert_eq!(
        ap.target.date_filter.as_deref(),
        Some("week"),
        "user-picked \"week\" must be left alone"
    );
}

#[test]
fn relax_leaves_month_date_filter_untouched() {
    let mut ap = base_autopilot();
    ap.target.date_filter = Some("month".into());
    relax_legacy_filters(&mut ap);
    assert_eq!(
        ap.target.date_filter.as_deref(),
        Some("month"),
        "user-picked \"month\" must be left alone"
    );
}

#[test]
fn relax_leaves_none_date_filter_as_none() {
    let mut ap = base_autopilot();
    ap.target.date_filter = None;
    relax_legacy_filters(&mut ap);
    assert!(ap.target.date_filter.is_none());
}

#[test]
fn relax_preserves_all_unrelated_fields() {
    let mut ap = base_autopilot();
    // Set the fields relax touches (legacy defaults).
    ap.filter.keywords = Some(vec!["rust".into()]);
    ap.filter.min_match_score = 50.0;
    ap.target.date_filter = Some("24h".into());
    // Set non-relax fields to non-default values so we can assert they survive.
    ap.filter.exclude_keywords = Some(vec!["senior".into()]);
    ap.target.query = "backend engineer".into();
    ap.target.location = Some("Berlin".into());
    ap.target.country_code = Some("de".into());
    ap.target.boards = vec!["linkedin".into(), "indeed".into()];
    ap.target.pages = 3;
    ap.target.work_type = Some("remote".into());

    relax_legacy_filters(&mut ap);

    // The fix clears keywords + resets score + clears date_filter.
    assert!(ap.filter.keywords.is_none());
    assert_eq!(ap.filter.min_match_score, 0.0);
    assert!(ap.target.date_filter.is_none());

    // Everything else must be untouched.
    assert_eq!(
        ap.filter.exclude_keywords.as_deref(),
        Some(["senior".to_string()].as_ref()),
        "exclude_keywords must be preserved"
    );
    assert_eq!(ap.target.query, "backend engineer");
    assert_eq!(ap.target.location.as_deref(), Some("Berlin"));
    assert_eq!(ap.target.country_code.as_deref(), Some("de"));
    assert_eq!(ap.target.boards, vec!["linkedin", "indeed"]);
    assert_eq!(ap.target.pages, 3);
    assert_eq!(ap.target.work_type.as_deref(), Some("remote"));
}

#[test]
fn relax_is_idempotent() {
    // Calling relax_legacy_filters twice must equal calling it once — the
    // second call is a no-op on an already-relaxed autopilot.
    let mut ap = base_autopilot();
    // Start from the worst-case legacy state.
    ap.filter.keywords = Some(vec!["rust".into()]);
    ap.filter.exclude_keywords = Some(vec!["senior".into()]);
    ap.filter.min_match_score = 50.0;
    ap.target.date_filter = Some("24h".into());

    relax_legacy_filters(&mut ap);
    let after_first = (
        ap.filter.keywords.clone(),
        ap.filter.min_match_score,
        ap.target.date_filter.clone(),
    );

    relax_legacy_filters(&mut ap);
    let after_second = (
        ap.filter.keywords.clone(),
        ap.filter.min_match_score,
        ap.target.date_filter.clone(),
    );

    assert_eq!(after_first, after_second, "second call must be a no-op");
}

// ── relax_legacy_filters_once (I/O orchestration) ────────────────────────────

/// Seed a store with one restrictive autopilot (the legacy defaults that caused
/// zero-jobs) and return its id. Shared setup for the `_once` tests.
fn seed_restrictive(store: &AutopilotStore) -> String {
    store
        .create(serde_json::json!({
            "name": "Legacy",
            "target": {
                "board": "linkedin",
                "query": "rust",
                "pages": 1,
                "dateFilter": "24h"
            },
            "filter": {
                "minMatchScore": 50.0,
                "keywords": ["rust", "go"]
            },
            "schedule": "daily",
        }))
        .id
}

#[test]
fn relax_legacy_filters_once_relaxes_and_writes_marker_on_first_run() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let dir = temp.path().to_path_buf();
    let store = AutopilotStore::new(&dir);
    let id = seed_restrictive(&store);

    // Marker must not exist before the first run.
    let marker = dir.join(RELAX_MARKER_FILE);
    assert!(!marker.exists(), "marker must be absent before migration");

    store.relax_legacy_filters_once();

    // (a) Marker written after a successful first run.
    assert!(marker.exists(), "marker must be created after first run");

    // (b) On-disk autopilot has been relaxed.
    let ap = store.get(&id).expect("autopilot must still exist");
    assert_eq!(
        ap.filter.min_match_score, 0.0,
        "min_match_score must be reset from 50.0 to 0.0"
    );
    assert!(
        ap.filter.keywords.is_none(),
        "keywords must be cleared to None"
    );
    assert!(
        ap.target.date_filter.is_none(),
        "date_filter must be cleared from \"24h\" to None"
    );
}

#[test]
fn relax_legacy_filters_once_skips_when_marker_present() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let dir = temp.path().to_path_buf();
    let store = AutopilotStore::new(&dir);
    let id = seed_restrictive(&store);

    // Pre-create the marker — simulates a store that was already migrated.
    let marker = dir.join(RELAX_MARKER_FILE);
    std::fs::write(&marker, b"1").unwrap();

    store.relax_legacy_filters_once();

    // The autopilot must be completely unchanged (still restrictive).
    let ap = store.get(&id).expect("autopilot must still exist");
    assert_eq!(
        ap.filter.min_match_score, 50.0,
        "min_match_score must be left at 50.0 when marker is present"
    );
    assert!(
        ap.filter.keywords.is_some(),
        "keywords must remain Some([...]) when marker is present"
    );
    assert_eq!(
        ap.target.date_filter.as_deref(),
        Some("24h"),
        "date_filter must remain \"24h\" when marker is present"
    );
}

#[test]
fn relax_legacy_filters_once_does_not_write_marker_when_persist_fails() {
    use tempfile::TempDir;

    // Force write_to_disk to fail: create a DIRECTORY at the data_file path
    // (autopilots.json). std::fs::write() to a path that is a directory fails
    // on every platform. The marker's parent dir remains writable, so the only
    // thing that can gate the marker write is whether write_to_disk returned Ok.
    let temp = TempDir::new().unwrap();
    let dir = temp.path().to_path_buf();

    // Create <dir>/autopilots.json as a directory, not a file.
    let data_file = dir.join("autopilots.json");
    std::fs::create_dir_all(&data_file).unwrap();

    // AutopilotStore::new expects the *parent* dir to exist, which it does (temp).
    // Passing `dir` means data_file = dir/autopilots.json — already a dir above.
    let store = AutopilotStore::new(&dir);

    // load() will return an empty map (can't read a dir as JSON), which is fine —
    // we just need write_to_disk to fail so the marker is NOT written.
    store.relax_legacy_filters_once();

    let marker = dir.join(RELAX_MARKER_FILE);
    assert!(
        !marker.exists(),
        "marker must NOT be written when write_to_disk fails (retry guarantee)"
    );
}
