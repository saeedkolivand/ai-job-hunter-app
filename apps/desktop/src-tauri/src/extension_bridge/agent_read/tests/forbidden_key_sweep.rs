//! The forbidden-key sweep across every non-schema resource.

use crate::autopilot::Autopilot;

use super::super::automations::resolve_automations;
use super::super::job::{project_value, AgentJob};
use super::super::*;
use super::best_matches_projection::full_best_match_row_json;
use super::support::{blank_autopilot, full_found_job};

#[test]
fn no_resource_output_ever_carries_a_forbidden_key() {
    let job = project_value::<_, AgentJob>(&full_found_job()).unwrap();
    let automations = resolve_automations(&[blank_autopilot("ap-1")]);
    let best_matches =
        best_matches::resolve_best_matches(&[full_best_match_row_json()], 0, 20, None);
    let found_jobs_records = vec![Autopilot {
        found_jobs: vec![full_found_job()],
        ..blank_autopilot("ap-1")
    }];
    let no_filters = found_jobs::FoundJobsFilters::from_payload(&json!({})).unwrap();
    let found_jobs = found_jobs::resolve_found_jobs(
        &found_jobs_records,
        Some("ap-1"),
        &no_filters,
        &std::collections::HashSet::new(),
        0,
        20,
    )
    .unwrap();
    for value in [job, automations, best_matches, found_jobs] {
        let text = value.to_string();
        for forbidden in [
            // Key names.
            "resumeText",
            "coverLetter",
            "assistantNotes",
            "assistantProvider",
            "assistantModel",
            "assistantBaseUrl",
            // T3 hardening — the distinctive VALUES the fixtures above carry
            // for those keys, so a projection regression that leaks the same
            // content under a differently-named key (e.g. `notes`, `body`,
            // `sourceText`) cannot pass this sweep just by renaming the key.
            "SECRET RESUME TEXT",
            "SECRET COVER LETTER",
            "secret AI note",
            "gpt-secret",
            "internal.example.local",
        ] {
            assert!(!text.contains(forbidden), "leaked {forbidden} in {text}");
        }
    }
}

// ── issue #1155 — retryAfterMs + refused-request identity on the throttle envelope ──
