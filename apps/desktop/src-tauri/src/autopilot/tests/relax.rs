//! The legacy-filter loosen migration: `relax_legacy_filters` (pure, filesystem-free) and
//! `relax_legacy_filters_once` (the I/O orchestration around it).

use super::super::*;
use super::support::*;
use crate::autopilot::relax::{relax_legacy_filters, RELAX_MARKER_FILE};
use crate::scraping::types::WorkType;

// ── relax_legacy_filters ──────────────────────────────────────────────────────

/// Return a fully-populated `Autopilot` that each test can mutate in place.
/// Starts with the legacy restrictive defaults (the zero-jobs configuration)
/// so most tests only need to tweak the one field they care about.
fn base_autopilot() -> Autopilot {
    let now = 1_000_000u64;
    Autopilot {
        id: "test-id".into(),
        name: "Test AP".into(),
        target: AutopilotTarget {
            query: "engineer".into(),
            date_filter: Some("24h".into()),
            ..target_fixture()
        },
        filter: AutopilotFilter {
            min_match_score: 50.0,
            keywords: Some(vec!["rust".into(), "go".into()]),
            exclude_keywords: None,
        },
        schedule: "daily".into(),
        created_at: now,
        updated_at: now,
        ..autopilot_fixture()
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
    // Pins the documented narrow gap (autopilot/relax.rs `was_legacy`): a record
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
    for (start, expected, why) in [
        // 50.0 → reset to 0.0.
        (50.0, 0.0, "default 50.0 must be reset to 0.0"),
        // 75.0 (deliberate user setting) → unchanged.
        (75.0, 75.0, "custom 75.0 must not be touched"),
        // Already 0.0 → stays 0.0 (idempotent / already relaxed).
        (0.0, 0.0, "an already-relaxed 0.0 stays 0.0"),
        // 49.9 is close to but NOT the magic 50.0 → unchanged.
        (
            49.9,
            49.9,
            "49.9 is not the legacy default; must be left unchanged",
        ),
    ] {
        let mut ap = base_autopilot();
        ap.filter.min_match_score = start;
        relax_legacy_filters(&mut ap);
        assert_eq!(ap.filter.min_match_score, expected, "{why}");
    }
}

#[test]
fn relax_clears_date_filter_only_for_24h() {
    for (start, expected, why) in [
        // "24h" is the legacy auto-default → should become None.
        (
            Some("24h"),
            None,
            "\"24h\" legacy default must be cleared to None",
        ),
        (
            Some("week"),
            Some("week"),
            "user-picked \"week\" must be left alone",
        ),
        (
            Some("month"),
            Some("month"),
            "user-picked \"month\" must be left alone",
        ),
        (None, None, "an unset date_filter stays None"),
    ] {
        let mut ap = base_autopilot();
        ap.target.date_filter = start.map(Into::into);
        relax_legacy_filters(&mut ap);
        assert_eq!(ap.target.date_filter.as_deref(), expected, "{why}");
    }
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
    ap.target.work_types = Some(vec![WorkType::Remote]);

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
    assert_eq!(ap.target.work_types, Some(vec![WorkType::Remote]));
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
    let (_temp, dir) = temp_dir();
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
    let (_temp, dir) = temp_dir();
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
    // Force write_to_disk to fail: `autopilots.json` is a non-empty DIRECTORY, so
    // it can be neither read nor replaced on any platform (the load treats it as
    // unreadable and blocks saves; a rename onto it would fail anyway). The
    // marker's parent dir remains writable, so the only thing that can gate the
    // marker write is whether write_to_disk returned Ok.
    let (_temp, dir) = temp_dir();
    let data_path = dir.join("autopilots.json");
    std::fs::create_dir_all(&data_path).unwrap();
    std::fs::write(data_path.join("keep"), b"x").unwrap();

    let store = AutopilotStore::new(&dir);
    store.relax_legacy_filters_once();

    let marker = dir.join(RELAX_MARKER_FILE);
    assert!(
        !marker.exists(),
        "marker must NOT be written when write_to_disk fails (retry guarantee)"
    );
}
