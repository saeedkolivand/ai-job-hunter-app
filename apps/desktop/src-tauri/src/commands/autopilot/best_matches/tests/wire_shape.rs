//! The `BestMatchRow` wire-shape pin.

use super::*;

// ── BestMatchRow wire-shape pin (review round 2, issue #1106 follow-up)
// ───────────────────────────────────────────────────────────────────
// `extension_bridge::agent_read`'s `best-matches` projection test builds
// its own input fixture as a hand-typed JSON literal — it cannot name
// `BestMatchRow` directly (this module is private to `commands::
// autopilot`, so the two files can't share a fixture-building fn across
// that boundary). A hand-typed literal alone would NOT notice a
// `BestMatchRow` field rename; it would just keep sending the OLD key
// name forever while `agent_read`'s test stayed green. This test is the
// compile-time backstop instead: it's a REAL struct literal naming every
// field, so renaming, adding, or removing one is either a compile error
// (fails the WHOLE crate's test build — nothing "stays green") or an
// assertion failure below. If this test's key list ever needs to change,
// update `agent_read`'s `full_best_match_row_json()` fixture and
// `best_match_projection_has_exact_keys` test to match in the same PR.
#[test]
fn best_match_row_wire_shape_is_pinned() {
    let row = BestMatchRow {
        key: "cluster-abc".to_string(),
        title: "Backend Engineer".to_string(),
        company: "Acme".to_string(),
        url: "https://boards.example.com/jobs/42".to_string(),
        location: Some("Berlin".to_string()),
        board: Some("adzuna".to_string()),
        salary_min: Some(60_000.0),
        salary_max: Some(80_000.0),
        salary_currency: Some("EUR".to_string()),
        score: 82.0,
        score_source: ScoreSource::Combined,
        score_provisional: false,
        has_description: true,
        score_url: Some("https://boards.example.com/jobs/42-other-member".to_string()),
        posted_at: Some(1_699_000_000),
        found_at: 1_700_000_000,
        applied: false,
        applied_exact: false,
        applied_related: false,
        availability: Some(Availability::Open),
        availability_checked_at: Some(1_700_000_000),
        availability_source: Some(AvailabilityEvidence::Ok),
        is_agency: false,
        trust: Some(crate::scraping::trust::TrustAssessment {
            score: 90,
            level: crate::scraping::trust::TrustLevel::High,
            flags: Vec::new(),
        }),
        ghost_signals: Some(crate::posting_history::signals_for(0, 2, 50 * 86_400_000)),
        assistant_notes: Some("secret AI note".to_string()),
        fit: crate::autopilot::NoteFitView {
            mandatory_gaps: vec!["Italian C1 required, résumé shows B1".to_string()],
            to_verify: vec!["Italian C1".to_string()],
            fit_category: Some(crate::autopilot::FitCategory::ExplicitGap),
            notes_stale: true,
        },
        cluster_members: vec![ClusterMemberRef {
            key: "k1".to_string(),
            board: Some("adzuna".to_string()),
            url: "https://boards.example.com/jobs/42".to_string(),
        }],
        sources: vec![BestMatchSource {
            autopilot_id: "ap-1".to_string(),
            autopilot_name: "My autopilot".to_string(),
            paused: false,
            found_at: 1_700_000_000,
        }],
    };
    let value = serde_json::to_value(&row).expect("BestMatchRow always serializes");
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "applied",
            "appliedExact",
            "appliedRelated",
            "assistantNotes",
            "availability",
            "availabilityCheckedAt",
            "availabilitySource",
            "board",
            "clusterMembers",
            "company",
            "fitCategory",
            "foundAt",
            "ghostSignals",
            "isAgency",
            "key",
            "location",
            "mandatoryGaps",
            "notesStale",
            "postedAt",
            "salaryCurrency",
            "salaryMax",
            "salaryMin",
            "score",
            "scoreProvisional",
            "scoreSource",
            "scoreUrl",
            "sources",
            "title",
            "toVerify",
            "trust",
            "url",
        ],
        "BestMatchRow's wire shape changed — mirror the change onto \
         extension_bridge::agent_read's AgentBestMatch projection and its \
         full_best_match_row_json() test fixture in the same PR"
    );
}

#[test]
fn ipc_cap_is_a_projection_over_the_full_sorted_set() {
    let jobs: Vec<FoundJob> = (0..256)
        .map(|i| {
            job(
                &format!("https://cap.example.com/job/{i}"),
                &format!("Engineer {i}"),
                &format!("Co{i}"),
                Some(90.0),
                ScoreSource::Keyword,
            )
        })
        .collect();
    let full = best(&[autopilot("cap", AutopilotStatus::Active, jobs)]).matches;
    assert_eq!(full.len(), 256);
    let head: Vec<String> = full[..BEST_MATCHES_CAP]
        .iter()
        .map(|r| r.key.clone())
        .collect();

    let mut agent = full.clone();
    limit_rows(&mut agent, false);
    assert_eq!(agent.len(), 256, "the agent path is uncapped");

    let mut native = full;
    limit_rows(&mut native, true);
    assert_eq!(native.len(), 100, "the native command is capped");
    assert_eq!(
        native.iter().map(|r| r.key.clone()).collect::<Vec<_>>(),
        head
    );
}
