//! `decide()` case by case.

use super::*;
use crate::sync::rules::{decide, Decision, Local};
use crate::sync::state::BaselineEntry;

#[test]
fn numeric_payload_hash_survives_the_wire_round_trip() {
    for cost in [
        9.500900000000001e-05_f64,
        9.969900000000001e-05,
        9.010100000000001e-05,
    ] {
        let payload = json!({"estCostUsd": cost});
        let received: Value =
            serde_json::from_str(&serde_json::to_string(&payload).unwrap()).unwrap();
        assert_eq!(received, payload);
        assert_eq!(
            crate::sync::envelope::record_hash(&received),
            crate::sync::envelope::record_hash(&payload)
        );
    }
}

fn env(rev: u64, device: &str, schema: u32, payload: Value) -> Envelope {
    Envelope {
        store: "items".into(),
        record_id: "1".into(),
        rev,
        device_id: device.into(),
        deleted: payload.is_null(),
        schema_version: schema,
        payload,
    }
}

fn base(rev: u64, device: &str, hash: &str, deleted: bool) -> BaselineEntry {
    BaselineEntry {
        hash: hash.into(),
        rev,
        device_id: device.into(),
        schema_version: 1,
        deleted,
    }
}

fn local<'a>(baseline: Option<&'a BaselineEntry>, current: Option<&'a str>) -> Local<'a> {
    Local {
        baseline,
        current_hash: current,
        schema_version: 1,
        run_rev: 100,
        device_id: "me",
    }
}

#[test]
fn unknown_store_or_newer_schema_is_held() {
    let e = env(50, "other", 1, v("x"));
    assert_eq!(decide(&e, &local(None, None), false), Decision::Hold);
    let e = env(50, "other", 2, v("x"));
    assert_eq!(decide(&e, &local(None, None), true), Decision::Hold);
}

#[test]
fn no_baseline_adopts_the_server_state() {
    let e = env(5, "other", 1, v("x"));
    assert_eq!(
        decide(&e, &local(None, Some("local")), true),
        Decision::Upsert
    );
    let t = env(5, "other", 1, Value::Null);
    assert_eq!(
        decide(&t, &local(None, Some("local")), true),
        Decision::Delete
    );
}

#[test]
fn a_tombstoned_record_ignores_everything() {
    let b = base(10, "other", "", true);
    let e = env(99, "other", 1, v("x"));
    assert_eq!(decide(&e, &local(Some(&b), None), true), Decision::Ignore);
}

#[test]
fn an_unchanged_record_takes_a_newer_write() {
    let b = base(10, "other", "h", false);
    let e = env(20, "other", 1, v("x"));
    assert_eq!(
        decide(&e, &local(Some(&b), Some("h")), true),
        Decision::Upsert
    );
}

#[test]
fn an_echo_of_the_baseline_write_is_ignored() {
    let b = base(10, "me", "h", false);
    let e = env(10, "me", 1, v("x"));
    assert_eq!(
        decide(&e, &local(Some(&b), Some("h")), true),
        Decision::Ignore
    );
}

#[test]
fn a_local_edit_competes_as_this_run() {
    let b = base(10, "other", "h", false);
    // Older than this run: the local edit wins.
    let e = env(50, "other", 1, v("x"));
    assert_eq!(
        decide(&e, &local(Some(&b), Some("edited")), true),
        Decision::Ignore
    );
    // Newer than this run (another device's clock is ahead): it wins.
    let e = env(150, "other", 1, v("x"));
    assert_eq!(
        decide(&e, &local(Some(&b), Some("edited")), true),
        Decision::Upsert
    );
}

#[test]
fn equal_rev_breaks_ties_by_device_id() {
    let b = base(10, "other", "h", false);
    let low = env(100, "aaa", 1, v("x"));
    let high = env(100, "zzz", 1, v("x"));
    assert_eq!(
        decide(&low, &local(Some(&b), Some("edited")), true),
        Decision::Ignore
    );
    assert_eq!(
        decide(&high, &local(Some(&b), Some("edited")), true),
        Decision::Upsert
    );
}

#[test]
fn a_locally_deleted_record_competes_as_this_run() {
    let b = base(10, "other", "h", false);
    let e = env(50, "other", 1, v("x"));
    assert_eq!(decide(&e, &local(Some(&b), None), true), Decision::Ignore);
}

#[test]
fn an_older_schema_payload_never_replaces_a_newer_one() {
    let mut b = base(10, "other", "h", false);
    b.schema_version = 2;
    let l = Local {
        schema_version: 2,
        ..local(Some(&b), Some("h"))
    };
    let e = env(20, "old-app", 1, v("x"));
    assert_eq!(decide(&e, &l, true), Decision::Ignore);
    // A delete still applies: tombstones carry no payload shape.
    let t = env(20, "old-app", 1, Value::Null);
    assert_eq!(decide(&t, &l, true), Decision::Delete);
}
