//! `AutopilotTarget` / `AutopilotFilter` as persisted: the legacy `board` string, the lenient
//! `workTypes` parse, and the optional fields that must stay off the wire when unset.

use super::super::*;
use super::support::*;
use crate::scraping::types::WorkType;

#[test]
fn test_autopilot_target_serialization() {
    let target = AutopilotTarget {
        query: "software engineer".to_string(),
        location: Some("Berlin".to_string()),
        pages: 5,
        ..target_fixture()
    };
    let json = serde_json::to_string(&target);
    assert!(json.is_ok());
}

// ── `parse_work_types_lenient` — the only persisted, data-loss-capable path ──
//
// MUTATION CHECK for all four tests below: replace the `work_types` field's
// attributes with the naive
// `#[serde(default)] pub work_types: Option<Vec<crate::scraping::types::WorkType>>`
// (delete `deserialize_with = "parse_work_types_lenient"`). Every one of these
// tests goes red — `work_types_mixed_validity_drops_bad_entry_keeps_boards_and_query`
// because stock `Vec<WorkType>::deserialize` fails the WHOLE array (and so the
// whole `from_value::<AutopilotTarget>` call) on the one unrecognised entry
// instead of dropping it, which is exactly the data-loss path
// `AutopilotStore::load` → `save()` this field's doc warns about. Restore
// after checking.

#[test]
fn work_types_mixed_validity_drops_bad_entry_keeps_boards_and_query() {
    let raw = serde_json::json!({
        "boards": ["linkedin"],
        "query": "rust",
        "pages": 1,
        "workTypes": ["remote", "bogus", "hybrid"],
    });
    let target: AutopilotTarget = serde_json::from_value(raw)
        .expect("a single unrecognised workTypes entry must not fail the whole target");
    assert_eq!(target.boards, vec!["linkedin".to_string()]);
    assert_eq!(target.query, "rust");
    assert_eq!(
        target.work_types,
        Some(vec![WorkType::Remote, WorkType::Hybrid]),
        "the bad entry is dropped, the valid ones kept in order"
    );
}

#[test]
fn work_types_absent_key_deserializes_to_none() {
    // Every autopilot on disk today has no `workTypes` key at all (the only
    // UI control that could set it was removed in PR #614 before this field
    // existed) — this is the common case, not an edge case.
    let raw = serde_json::json!({
        "boards": ["linkedin"],
        "query": "rust",
        "pages": 1,
    });
    let target: AutopilotTarget = serde_json::from_value(raw)
        .expect("a legacy record with no workTypes key must deserialize");
    assert_eq!(target.work_types, None);
}

#[test]
fn work_types_all_unrecognised_collapses_to_empty_vec_not_none() {
    // `Some(vec![])`, not `None` — the deserializer's job is only to drop bad
    // entries, not to decide "empty means no filter"; that collapse belongs to
    // `BoardSearchInput::work_type_spec`, the single seam every consumer reads.
    let raw = serde_json::json!({
        "boards": ["linkedin"],
        "query": "rust",
        "pages": 1,
        "workTypes": ["bogus", "also-bogus"],
    });
    let target: AutopilotTarget =
        serde_json::from_value(raw).expect("an all-unrecognised array must still deserialize");
    assert_eq!(target.work_types, Some(Vec::new()));
}

#[test]
fn work_types_roundtrips_through_serialize_deserialize() {
    let target = AutopilotTarget {
        work_types: Some(vec![WorkType::OnSite, WorkType::Hybrid]),
        top_n: default_top_n(),
        ..target_fixture()
    };
    let json = serde_json::to_value(&target).unwrap();
    assert_eq!(
        json.get("workTypes"),
        Some(&serde_json::json!(["on-site", "hybrid"])),
        "must serialize through the shared kebab-case WorkType vocabulary"
    );
    let back: AutopilotTarget = serde_json::from_value(json).unwrap();
    assert_eq!(
        back.work_types,
        Some(vec![WorkType::OnSite, WorkType::Hybrid])
    );
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
        pages: 2,
        ..target_fixture()
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
        country_code: Some("us".to_string()),
        ..target_fixture()
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
        country_code: None,
        ..with_code
    };
    let json_none = serde_json::to_string(&without_code).unwrap();
    assert!(
        !json_none.contains("countryCode"),
        "country_code None must be omitted from serialized JSON (skip_serializing_if)"
    );
}
