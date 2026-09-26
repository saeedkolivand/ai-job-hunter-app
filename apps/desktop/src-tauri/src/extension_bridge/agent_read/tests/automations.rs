//! Tests for the `automations` resource (`automations.rs`).

use crate::autopilot::Autopilot;

use super::super::automations::{project_automation, resolve_automations};
use super::super::*;
use super::support::{blank_autopilot, full_found_job};

#[test]
fn automations_projection_has_exact_keys() {
    // Exercises the REAL production path (`project_automation` — the
    // direct field mapping, not `project_value`'s round trip) so this
    // test can't drift from what `resolve_automations` actually ships.
    let value =
        serde_json::to_value(project_automation(&blank_autopilot("ap-1"))).expect("projects");
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "createdAt",
            "foundJobsTotal",
            "id",
            "lastRunAt",
            "name",
            "runStatus",
            "status",
            "target",
            "totalFound",
            "updatedAt",
        ]
    );
    let target = value["target"].as_object().unwrap();
    let mut target_keys: Vec<String> = target.keys().cloned().collect();
    target_keys.sort();
    assert_eq!(target_keys, vec!["boards", "location", "query"]);
}

/// Issue #1132 — `totalFound` is the LAST run's kept count (an
/// `AutopilotStore::record_run` overwrite), so a caller reading it as "how
/// many jobs does this automation have" is off by however many earlier runs
/// found. `foundJobsTotal` is the traversable count, anchored HERE to
/// `found-jobs`' own `total` rather than to a hand-typed N, so the two
/// surfaces cannot drift apart while both still passing.
#[test]
fn automations_found_jobs_total_matches_found_jobs_own_total() {
    let records = vec![Autopilot {
        found_jobs: (0..7).map(|_| full_found_job()).collect(),
        total_found: 2, // the last run kept 2 — deliberately NOT 7
        ..blank_autopilot("ap-1")
    }];
    let row = &resolve_automations(&records)["automations"][0];
    let no_filters = found_jobs::FoundJobsFilters::from_payload(&json!({})).unwrap();
    let paged = found_jobs::resolve_found_jobs(
        &records,
        Some("ap-1"),
        &no_filters,
        &std::collections::HashSet::new(),
        0,
        1,
    )
    .expect("pages");
    assert_eq!(
        row["foundJobsTotal"], paged["total"],
        "foundJobsTotal must be exactly what found-jobs will page through"
    );
    assert_eq!(
        row["totalFound"], 2,
        "totalFound must keep its last-run meaning, unchanged by the new field"
    );
    assert_ne!(
        row["foundJobsTotal"], row["totalFound"],
        "the fixture must actually distinguish the two counts"
    );
}

#[test]
fn automations_projection_never_carries_forbidden_keys() {
    let value = resolve_automations(&[blank_autopilot("ap-1")]);
    let text = value.to_string();
    for forbidden in [
        "resumeText",
        "coverLetter",
        "assistantProvider",
        "assistantModel",
        "assistantBaseUrl",
        "totalApplied", // issue #1171 — dead on the source struct, never a real applied count
        "SECRET",
        "internal.example.local",
    ] {
        assert!(!text.contains(forbidden), "leaked {forbidden}");
    }
}

// ── best-matches ─────────────────────────────────────────────────────────
