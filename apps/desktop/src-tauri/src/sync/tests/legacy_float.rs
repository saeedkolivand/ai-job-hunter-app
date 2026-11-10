//! The `float_roundtrip` upgrade bridge (#1435): a baseline hashed from a
//! payload the old fast float parser produced must not read as a local edit.

use super::*;
use crate::sync::envelope::record_hash;
use crate::sync::legacy_float::{legacy_parse_f64, legacy_record_hash};
use crate::sync::state::BaselineEntry;

const ISSUE_TEXT: &str = "0.00009500900000000001";

fn exact() -> f64 {
    serde_json::from_str(ISSUE_TEXT).unwrap()
}

#[test]
fn legacy_parser_reproduces_the_old_rounding() {
    let old = legacy_parse_f64(ISSUE_TEXT).unwrap();
    assert_eq!(old.to_bits(), 0.000_095_009_f64.to_bits());
    assert_eq!(exact().to_bits(), 9.500_900_000_000_001e-5_f64.to_bits());
}

#[test]
fn legacy_parser_agrees_with_the_exact_one_on_ordinary_values() {
    for text in [
        "0.1", "1.5", "-2.25", "1.0", "3.14159", "1e21", "-1.5e-7", "0.5",
    ] {
        let exact: f64 = text.parse().unwrap();
        assert_eq!(legacy_parse_f64(text), Some(exact), "{text}");
    }
}

#[test]
fn legacy_parser_handles_long_and_extreme_text() {
    // More than 19 significant digits: the extra digits are ignored.
    assert_eq!(legacy_parse_f64("1.00000000000000000000001"), Some(1.0));
    assert_eq!(
        legacy_parse_f64("123456789012345678901.5"),
        Some(1.234_567_890_123_456_7e20)
    );
    // Beyond the POW10 table, and out of range.
    assert_eq!(legacy_parse_f64("1e-320"), Some(1e-320));
    assert_eq!(legacy_parse_f64("1e400"), None);
    assert_eq!(legacy_parse_f64("0e99999999999"), Some(0.0));
    assert_eq!(legacy_parse_f64("12"), None);
}

fn baseline_of(hash: String) -> BaselineEntry {
    BaselineEntry {
        hash,
        rev: 10,
        device_id: "zzz".into(),
        schema_version: 1,
        deleted: false,
    }
}

/// `a` holds `local` for record 1, baselined from the legacy-rounded value;
/// `b` has pushed a newer remote edit.
fn upgraded_pair(local: Value, server: &FakeServer) -> Device {
    let (a, b) = (Device::new(), Device::new());
    let stale = json!({ "f": exact() });
    assert_ne!(record_hash(&stale), legacy_record_hash(&stale));
    a.items.put("1", local);
    a.state
        .put_baseline(
            "scope-a",
            "items",
            "1",
            &baseline_of(legacy_record_hash(&stale)),
        )
        .unwrap();
    b.items.put("1", json!({ "f": 1.5 }));
    b.sync(server, 20).unwrap();
    a
}

fn baseline_hash(d: &Device) -> String {
    let rows = d.state.baseline("scope-a").unwrap();
    rows.into_iter().find(|r| r.1 == "1").unwrap().2.hash
}

#[test]
fn an_unchanged_legacy_hashed_record_does_not_hide_a_pending_remote_update() {
    let server = FakeServer::new();
    let a = upgraded_pair(json!({ "f": exact() }), &server);

    let report = a.sync(&server, 30).unwrap();

    assert_eq!(a.items.get("1"), Some(json!({ "f": 1.5 })));
    assert_eq!((report.applied, report.pushed), (1, 0));
    assert_eq!(server.len(), 1);
}

#[test]
fn an_unchanged_legacy_hashed_record_is_rebaselined_and_not_pushed() {
    let server = FakeServer::new();
    let a = Device::new();
    let local = json!({ "f": exact() });
    a.items.put("1", local.clone());
    a.state
        .put_baseline(
            "scope-a",
            "items",
            "1",
            &baseline_of(legacy_record_hash(&local)),
        )
        .unwrap();

    let report = a.sync(&server, 30).unwrap();

    assert_eq!(report.pushed, 0);
    assert_eq!(baseline_hash(&a), record_hash(&local));
}

#[test]
fn a_genuine_local_edit_still_wins_over_an_older_remote_update() {
    let server = FakeServer::new();
    let edited = json!({ "f": 2.5 });
    let a = upgraded_pair(edited.clone(), &server);

    let report = a.sync(&server, 30).unwrap();

    assert_eq!(a.items.get("1"), Some(edited.clone()));
    assert_eq!((report.applied, report.pushed), (0, 1));
    assert_eq!(baseline_hash(&a), record_hash(&edited));
}
