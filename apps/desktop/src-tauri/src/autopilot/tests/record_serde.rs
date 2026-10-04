//! Back-compat for the other persisted shapes: a record or found job written before a field
//! existed must still load, and a round trip must not grow keys.

use super::super::*;
use super::support::*;

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
fn legacy_record_without_assistant_fields_defaults_to_disabled() {
    // An autopilots.json written before Phase 4 has no `assistant*` keys — it must
    // load with AI notes OFF and no provider snapshot (opt-in, zero surprise).
    let json = serde_json::json!({
        "_id": "ap1",
        "name": "Legacy",
        "status": "active",
        "target": { "boards": ["linkedin"], "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 0.0 },
        "schedule": "daily",
        "totalFound": 0,
        "totalApplied": 0,
        "createdAt": 1,
        "updatedAt": 1
    });
    let ap: Autopilot = serde_json::from_value(json).expect("legacy record must deserialize");
    assert!(
        !ap.assistant,
        "AI notes must default OFF for a legacy record"
    );
    assert!(ap.assistant_provider.is_none());
    assert!(ap.assistant_model.is_none());
    assert!(ap.assistant_base_url.is_none());
}

#[test]
fn legacy_found_job_without_assistant_notes_deserializes_to_none() {
    // A found job persisted before Phase 4 has no `assistantNotes` key.
    let json = serde_json::json!({
        "title": "Engineer",
        "company": "Acme",
        "url": "https://acme.example/1",
        "foundAt": 1u64
    });
    let job: FoundJob = serde_json::from_value(json).expect("legacy found job must deserialize");
    assert!(job.assistant_notes.is_none());
}

#[test]
fn assistant_note_round_trips_on_a_found_job() {
    // A note set by the AI-notes step survives serialize→deserialize (persisted on
    // the record, surfaced to the renderer under `assistantNotes`).
    let mut job = found_job("https://acme.example/2", 5);
    job.assistant_notes = Some("Strong Rust fit; tailor the systems-design bullet.".into());
    let round: FoundJob = serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
    assert_eq!(
        round.assistant_notes.as_deref(),
        Some("Strong Rust fit; tailor the systems-design bullet.")
    );
}

#[test]
fn autopilot_record_without_summaries_field_deserializes_to_empty() {
    // A record persisted before `lastRunSummaries` / the new `runStatus` variant
    // existed must still load — `#[serde(default)]` fills the missing field with
    // an empty list rather than failing the whole store read.
    let legacy = serde_json::json!({
        "_id": "legacy-1",
        "name": "Legacy",
        "status": "active",
        "target": { "board": "linkedin", "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 0.0 },
        "schedule": "manual",
        "totalFound": 0,
        "totalApplied": 0,
        "createdAt": 1,
        "updatedAt": 1
        // no runStatus, no foundJobs, no lastRunSummaries
    });
    let ap: Autopilot = serde_json::from_value(legacy).expect("legacy record must deserialize");
    assert!(ap.last_run_summaries.is_empty());
    assert_eq!(ap.run_status, None);
}

#[test]
fn board_scrape_summary_legacy_single_note_field_deserializes_with_empty_notes() {
    // A record persisted by the old single-slot `note: Option<String>` shape
    // must still load. There is no `#[serde(alias = "note")]` — the legacy key
    // is silently ignored (unknown fields are dropped by default) and `notes`
    // takes its `#[serde(default)]` empty vec. Documented as an accepted,
    // already-established trade-off for this display-only field (see the doc
    // on `BoardScrapeSummary::notes`) — pinned here so it stays a DECISION,
    // not an unverified assumption.
    let legacy = serde_json::json!({
        "board": "greenhouse",
        "count": 6,
        "note": "location-filtered:5",
    });
    let back: crate::scraping::BoardScrapeSummary =
        serde_json::from_value(legacy).expect("legacy single-note record must deserialize");
    assert_eq!(back.board, "greenhouse");
    assert_eq!(back.count, 6);
    assert!(
        back.notes.is_empty(),
        "the legacy singular `note` key must NOT populate `notes` (no alias); got {:?}",
        back.notes
    );
}

#[test]
fn board_scrape_summary_round_trips_through_the_run_record() {
    // The run record persists `BoardScrapeSummary` (Serialize + Deserialize), so
    // a summary with every optional set must survive a serialize→deserialize
    // cycle unchanged — omitted `error`/`skipped` come back as `None`.
    let original = board_summary(
        "themuse",
        7,
        None,
        None,
        Some("page 3 of 5 failed: HTTP 429"),
    );
    let json = serde_json::to_string(&original).unwrap();
    let back: crate::scraping::BoardScrapeSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(back.board, "themuse");
    assert_eq!(back.count, 7);
    assert_eq!(back.error, None);
    assert_eq!(back.skipped, None);
    assert_eq!(
        back.truncated.as_deref(),
        Some("page 3 of 5 failed: HTTP 429")
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
fn legacy_found_job_without_posted_at_deserializes_to_none() {
    // Old persisted FoundJob records pre-date the `postedAt` field. The
    // `#[serde(default)]` must let them load with `posted_at: None` rather than
    // failing, and re-serialize without emitting a `postedAt` key at all
    // (`skip_serializing_if`) — a legacy record must load AND save unchanged.
    let json = r#"{
        "title": "Engineer",
        "company": "Acme",
        "url": "https://a.com/1",
        "foundAt": 100
    }"#;
    let job: FoundJob = serde_json::from_str(json).expect("legacy FoundJob must deserialize");
    assert_eq!(job.posted_at, None, "absent postedAt must default to None");

    let round_tripped = serde_json::to_string(&job).unwrap();
    assert!(
        !round_tripped.contains("postedAt"),
        "a legacy record with no posted_at must not grow a postedAt key on save; got {round_tripped}"
    );
    let restored: FoundJob =
        serde_json::from_str(&round_tripped).expect("re-serialized legacy record must load");
    assert_eq!(restored.posted_at, None);
}

#[test]
fn found_job_with_posted_at_round_trips() {
    // A FoundJob carrying the posting's publish date serializes the camelCase
    // key as epoch millis and round-trips.
    let mut job = found_job("https://a.com/1", 100);
    job.posted_at = Some(1_700_000_000_000);
    let json = serde_json::to_string(&job).unwrap();
    assert!(
        json.contains("\"postedAt\":1700000000000"),
        "postedAt must serialize as a camelCase epoch-ms number; got {json}"
    );
    let restored: FoundJob = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.posted_at, Some(1_700_000_000_000));
}
