//! Ghost signals on a found job: back-compat with records written before the
//! field existed, and the merge refreshing them on a re-surfaced posting.

use super::support::*;
use crate::autopilot::merge::merge_found_jobs;
use crate::posting_history::signals_for;

#[test]
fn a_found_job_without_the_field_still_deserializes() {
    // The shape an old `autopilots.json` holds: no `ghostSignals` key at all.
    let old = serde_json::json!({
        "title": "Engineer", "company": "Acme", "url": "https://acme.test/1", "foundAt": 1
    });
    let job: crate::autopilot::FoundJob = serde_json::from_value(old).unwrap();
    assert!(job.ghost_signals.is_none());
    let out = serde_json::to_value(&job).unwrap();
    assert!(
        out.get("ghostSignals").is_none(),
        "absent stays absent on the wire"
    );
}

#[test]
fn ghost_signals_use_the_camel_case_wire_shape() {
    let mut job = found_job("https://acme.test/1", 1);
    job.ghost_signals = Some(signals_for(0, 2, 50 * 86_400_000));
    let v = serde_json::to_value(&job).unwrap();
    let g = &v["ghostSignals"];
    assert_eq!(g["firstSeenAt"], 0);
    assert_eq!(g["openDays"], 50);
    assert_eq!(g["repostCount"], 2);
    assert_eq!(g["openLong"], true);
    assert_eq!(g["reposted"], true);
    let back: crate::autopilot::FoundJob = serde_json::from_value(v).unwrap();
    assert_eq!(back.ghost_signals, job.ghost_signals);
}

#[test]
fn a_resurfaced_row_takes_the_fresh_signals_and_an_unsignalled_one_keeps_its_own() {
    let mut old = found_job("https://acme.test/1", 1);
    old.ghost_signals = Some(signals_for(0, 0, 0));
    let mut fresh = found_job("https://acme.test/1", 2);
    fresh.ghost_signals = Some(signals_for(0, 1, 10 * 86_400_000));

    let merged = merge_found_jobs(&[old.clone()], vec![fresh.clone()]);
    assert_eq!(merged[0].ghost_signals, fresh.ghost_signals);

    let mut unsignalled = fresh;
    unsignalled.ghost_signals = None;
    let merged = merge_found_jobs(&[old.clone()], vec![unsignalled]);
    assert_eq!(merged[0].ghost_signals, old.ghost_signals);
}
