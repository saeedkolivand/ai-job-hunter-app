//! Spanning-every-autopilot tests (issue #1168): dedup, dedup ordering (B3-r1-F1), spanning
//! cursor walk, and cursor scope isolation.

use super::super::*;
use super::support::*;
use crate::autopilot::FoundJob;
use std::collections::HashSet;

#[test]
fn found_jobs_spans_every_autopilot_when_autopilot_id_is_omitted() {
    let records = vec![
        autopilot_with_jobs("ap-1", (0..3).map(numbered_job).collect()),
        autopilot_with_jobs("ap-2", (3..5).map(numbered_job).collect()),
    ];
    let out = resolve_found_jobs(&records, None, &no_filters(), &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 5);
    assert!(out.get("autopilotId").is_none());
    assert!(out.get("autopilotName").is_none());
    let urls: Vec<String> = out["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["url"].as_str().unwrap().to_string())
        .collect();
    for n in 0..5 {
        assert!(
            urls.contains(&format!("https://boards.example.com/jobs/{n}")),
            "job {n} missing from the spanning traversal"
        );
    }
    // Rows must carry their OWN autopilotId — the only way to tell which
    // autopilot each spanning-traversal row came from.
    let owners: HashSet<&str> = out["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["autopilotId"].as_str().unwrap())
        .collect();
    assert_eq!(owners, HashSet::from(["ap-1", "ap-2"]));
}

#[test]
fn found_jobs_spanning_dedupes_the_same_job_across_autopilots() {
    let shared = numbered_job(1);
    let records = vec![
        autopilot_with_jobs("ap-1", vec![shared.clone()]),
        autopilot_with_jobs("ap-2", vec![shared]),
    ];
    let out = resolve_found_jobs(&records, None, &no_filters(), &no_applied(), 0, 20).unwrap();
    assert_eq!(
        out["total"], 1,
        "the same posting in two autopilots' lists must collapse to one row"
    );
    assert_eq!(
        out["jobs"][0]["autopilotId"], "ap-1",
        "the FIRST autopilot in store order wins the dedup"
    );
}

/// B3-r1-F1 (HIGH) — `score` is per-autopilot (the SAME posting scored
/// against each autopilot's own resume), so dedup must run AFTER filtering:
/// the FIRST autopilot's copy fails `minScore`, the SECOND's passes. Before
/// the fix, the dedup slot was consumed by the failing first copy and the
/// passing second copy was silently dropped, under-reporting `total` on the
/// exact filter this resource exists to serve.
#[test]
fn found_jobs_spanning_dedup_lets_a_later_autopilots_passing_copy_win_over_an_earlier_failing_one()
{
    let shared = numbered_job(1);
    let scored_low_in_ap1 = FoundJob {
        score: Some(50.0),
        ..shared.clone()
    };
    let scored_high_in_ap2 = FoundJob {
        score: Some(90.0),
        ..shared
    };
    let records = vec![
        autopilot_with_jobs("ap-1", vec![scored_low_in_ap1]),
        autopilot_with_jobs("ap-2", vec![scored_high_in_ap2]),
    ];
    let filters = FoundJobsFilters::from_payload(&json!({ "minScore": 70 })).unwrap();
    let out = resolve_found_jobs(&records, None, &filters, &no_applied(), 0, 20).unwrap();
    assert_eq!(
        out["total"], 1,
        "the passing copy under ap-2 must survive even though ap-1's copy of the same \
         posting failed minScore first"
    );
    assert_eq!(
        out["jobs"][0]["autopilotId"], "ap-2",
        "the row returned must be the PASSING copy, not the failing one that happened to be \
         first in store order"
    );
}

#[test]
fn found_jobs_spanning_cursor_walks_every_autopilot_then_terminates() {
    let records = vec![
        autopilot_with_jobs("ap-1", (0..7).map(numbered_job).collect()),
        autopilot_with_jobs("ap-2", (7..12).map(numbered_job).collect()),
    ];
    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let offset = parse_found_jobs_cursor(&json!({ "cursor": cursor }), &issuer(None))
            .expect("own cursor");
        let out =
            resolve_found_jobs(&records, None, &no_filters(), &no_applied(), offset, 4).unwrap();
        for row in out["jobs"].as_array().unwrap() {
            seen.push(row["url"].as_str().unwrap().to_string());
        }
        match out["nextCursor"].as_str() {
            Some(next) => cursor = Some(next.to_string()),
            None => break,
        }
        assert!(seen.len() <= 12, "must terminate at the true end");
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        12,
        "every job across both autopilots seen once"
    );
}

#[test]
fn found_jobs_all_autopilots_cursor_is_rejected_when_replayed_scoped() {
    let records = vec![autopilot_with_jobs(
        "ap-1",
        (0..30).map(numbered_job).collect(),
    )];
    let issued = resolve_found_jobs(&records, None, &no_filters(), &no_applied(), 0, 10)
        .expect("page 1")["nextCursor"]
        .as_str()
        .expect("more pages")
        .to_string();
    let err = parse_found_jobs_cursor(&json!({ "cursor": issued }), "ap-1").unwrap_err();
    assert_eq!(err.to_string(), WRONG_AUTOPILOT_CURSOR_MESSAGE);
}

/// B3-r1-F4 regression pin (round 3, B3-r3-F6; T1 hardening) — every OTHER
/// cursor test in this file computes its expected issuer via
/// `issuer()`/`no_filters()` (self-referential against
/// `found_jobs_cursor_issuer`) or passes a bare id string that mismatches
/// with or without the fingerprint either way, so none of them can catch a
/// regression that drops one of the FIVE parts `found_jobs_cursor_issuer`
/// folds into its fingerprint (`min_score`, `country`, `remote`, `applied`,
/// `query`). This drives two DIFFERENT filter sets, same scope, by hand,
/// once per part: a cursor issued under one value of that part must be
/// rejected when replayed against the issuer for the other value — if
/// `found_jobs_cursor_issuer` ever stopped folding a given part in, that
/// one case would wrongly succeed while the other four stayed green.
#[test]
fn found_jobs_cursor_issued_under_one_filter_set_is_rejected_under_another() {
    let cases: [(Value, Value); 5] = [
        (json!({ "minScore": 70 }), json!({ "minScore": 90 })),
        (json!({ "country": "de" }), json!({ "country": "fr" })),
        (json!({ "remote": true }), json!({ "remote": false })),
        (json!({ "applied": true }), json!({ "applied": false })),
        (json!({ "query": "a" }), json!({ "query": "b" })),
    ];
    for (payload_a, payload_b) in cases {
        let issuer_a = found_jobs_cursor_issuer(
            Some("ap-1"),
            &FoundJobsFilters::from_payload(&payload_a).unwrap(),
        );
        let cursor = format!("{issuer_a}:5");

        let issuer_b = found_jobs_cursor_issuer(
            Some("ap-1"),
            &FoundJobsFilters::from_payload(&payload_b).unwrap(),
        );
        let err = parse_found_jobs_cursor(&json!({ "cursor": cursor }), &issuer_b).unwrap_err();
        assert_eq!(
            err.to_string(),
            WRONG_AUTOPILOT_CURSOR_MESSAGE,
            "cursor issued under {payload_a} must be rejected when replayed under {payload_b}"
        );
    }
}

// ── issue #1167: server-side filters ───────────────────────────────────
