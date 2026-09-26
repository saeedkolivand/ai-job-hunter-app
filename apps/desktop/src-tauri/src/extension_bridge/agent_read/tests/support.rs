//! Shared fixtures for `agent_read::tests`' topic modules.

use serde_json::Value;

use crate::autopilot::{
    Autopilot, AutopilotFilter, AutopilotStatus, AutopilotTarget, FoundJob, RunStatus, ScoreSource,
};
use crate::scraping::cluster::ClusterMemberRef;
use crate::scraping::trust::{TrustAssessment, TrustLevel};

/// Assert `value`'s object key set (sorted) equals `expected` — used to
/// descend into a NESTED object-valued field (`trust`, one `sources`
/// entry), not just the top level. The exact-keys tests below are the
/// mutation-checked regression guard for finding #2 (security review):
/// before [`AgentTrust`] existed, `trust`'s source type (`TrustAssessment`)
/// was serialized whole, so this same assertion — added first, against
/// the OLD code — failed the moment a field was added to that source
/// struct (verified by hand during review; not re-run here since it would
/// require mutating a sibling domain's type). `AgentTrust`'s own explicit
/// field set is what makes it pass now.
///
/// `pub(super)` for `job`/`best-matches`'s own nested-object descent below
/// (issue #1167's compact `found-jobs` row no longer carries a nested
/// `trust` object, so `found_jobs::tests` no longer needs this helper).
pub(in crate::extension_bridge::agent_read) fn assert_object_keys(
    value: &Value,
    path: &str,
    expected: &[&str],
) {
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("{path} must be an object, got {value}"));
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(keys, expected, "unexpected key set at {path}");
}

// ── RESOURCES / schema ───────────────────────────────────────────────────

/// `pub(super)` — reused verbatim by `found_jobs::tests`.
pub(in crate::extension_bridge::agent_read) fn full_found_job() -> FoundJob {
    FoundJob {
        title: "Backend Engineer".into(),
        company: "Acme".into(),
        url: "https://boards.example.com/jobs/42".into(),
        location: Some("Berlin".into()),
        board: Some("adzuna".into()),
        board_remote: false,
        description: Some("Full posting text.".into()),
        salary_min: Some(60_000.0),
        salary_max: Some(80_000.0),
        salary_currency: Some("EUR".into()),
        score: Some(82.0),
        score_provisional: false,
        score_source: ScoreSource::Combined,
        found_at: 1_700_000_000,
        posted_at: Some(1_699_000_000),
        is_new: true,
        applied: false,
        trust: Some(TrustAssessment {
            score: 90,
            level: TrustLevel::High,
            flags: vec![],
        }),
        assistant_notes: Some("secret AI note about this posting".into()),
        cluster_id: Some("cluster-1".into()),
        cluster_canonical: true,
        cluster_members: vec![ClusterMemberRef {
            key: "opaque-cluster-key".into(),
            board: Some("adzuna".into()),
            url: "https://boards.example.com/jobs/42".into(),
        }],
        is_agency: false,
    }
}

/// `pub(super)` — reused verbatim by `found_jobs::tests`.
pub(in crate::extension_bridge::agent_read) fn blank_autopilot(id: &str) -> Autopilot {
    Autopilot {
        id: id.into(),
        name: format!("autopilot-{id}"),
        status: AutopilotStatus::Active,
        target: AutopilotTarget {
            boards: vec!["adzuna".into()],
            query: "backend engineer".into(),
            location: Some("Berlin".into()),
            country_code: Some("de".into()),
            work_types: None,
            pages: 1,
            date_filter: None,
            top_n: 3,
            watched_companies_only: None,
        },
        filter: AutopilotFilter {
            min_match_score: 60.0,
            keywords: None,
            exclude_keywords: None,
        },
        schedule: "manual".into(),
        schedule_hour: None,
        schedule_minute: None,
        resume_text: Some("SECRET RESUME TEXT".into()),
        cover_letter: Some("SECRET COVER LETTER".into()),
        assistant: true,
        assistant_provider: Some("openai".into()),
        assistant_model: Some("gpt-secret".into()),
        assistant_base_url: Some("http://internal.example.local:11434".into()),
        total_found: 1,
        total_applied: 0,
        found_jobs: vec![],
        run_status: Some(RunStatus::Completed),
        last_run_summaries: vec![],
        last_run_at: Some(1_700_000_000),
        created_at: 1_600_000_000,
        updated_at: 1_700_000_000,
    }
}
