//! Shared fixtures for `found_jobs`' test topics — `no_filters`/`issuer`/`no_applied`/
//! `autopilot_with_jobs`/`numbered_job`/`richest_realistic_job`/`worst_permitted_job`.

use super::super::*;
use crate::autopilot::{Autopilot, FoundJob, ScoreSource};
use crate::scraping::trust::{TrustAssessment, TrustLevel};
use std::collections::HashSet;

// Reused from `agent_read::tests::support` (marked `pub(in agent_read)` there) rather than
// duplicated — one `full_found_job`/`blank_autopilot` fixture for the whole module, never two
// that could drift.
pub(super) use super::super::super::tests::support::{blank_autopilot, full_found_job};

pub(super) fn no_filters() -> FoundJobsFilters {
    FoundJobsFilters::from_payload(&json!({})).unwrap()
}

/// The scope+filters cursor issuer a call with `autopilot_id`/`no_filters()`
/// actually issues (B3-r1-F4) — computed the SAME way the code under test
/// does, never a hand-typed literal, so this test file fails the moment the
/// two halves of the format ever stop agreeing.
pub(super) fn issuer(autopilot_id: Option<&str>) -> String {
    found_jobs_cursor_issuer(autopilot_id, &no_filters())
}

pub(super) fn no_applied() -> HashSet<String> {
    HashSet::new()
}

pub(super) fn autopilot_with_jobs(id: &str, jobs: Vec<FoundJob>) -> Autopilot {
    Autopilot {
        found_jobs: jobs,
        ..blank_autopilot(id)
    }
}

/// One job per index, distinguishable by `url` — lets a pagination test
/// assert every job was seen exactly once, not just that SOME jobs came
/// back.
pub(super) fn numbered_job(n: usize) -> FoundJob {
    FoundJob {
        url: format!("https://boards.example.com/jobs/{n}"),
        title: format!("Job {n}"),
        ..full_found_job()
    }
}

/// A realistic-but-rich job: short title/company/location, a full
/// preview-cap description (opted in), every optional numeric/trust
/// field populated — the ORDINARY shape a full page should rarely need
/// trimming for, even with description opted in.
pub(super) fn richest_realistic_job(n: usize) -> FoundJob {
    FoundJob {
        title: format!("Senior Backend Engineer - Distributed Systems, Platform Team #{n}"),
        company: "A Reasonably Long International Holdings GmbH & Co. KG".to_string(),
        url: format!(
            "https://boards.example.com/jobs/senior-backend-engineer-platform-team-{n}?utm_source=agent"
        ),
        location: Some("Berlin, Germany (Hybrid — 3 days onsite per week)".to_string()),
        board: Some("adzuna".to_string()),
        board_remote: false,
        description: Some("x".repeat(FOUND_JOBS_DESCRIPTION_PREVIEW_CAP)),
        salary_min: Some(65_000.0),
        salary_max: Some(95_000.0),
        salary_currency: Some("EUR".to_string()),
        score: Some(87.5),
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
        assistant_notes: None,
        cluster_id: None,
        cluster_canonical: true,
        cluster_members: vec![],
        is_agency: false,
    }
}

/// A job at the REAL permitted worst case: title/company/location each
/// pinned to `crate::prompt_fence::JOB_CAP` (8,000 chars), in
/// multi-byte CJK text (stresses the char-vs-byte distinction — a
/// char-counted cap is NOT a byte cap), plus a full-length preview
/// description (opted in). This is legitimate, non-adversarial content a
/// board could genuinely return.
pub(super) fn worst_permitted_job(n: usize) -> FoundJob {
    // U+4E2D ("中") is 3 bytes in UTF-8 — repeating it stresses the
    // byte/char gap far more than an ASCII fixture ever could.
    let cjk_field = |cap: usize| "中".repeat(cap);
    FoundJob {
        title: cjk_field(crate::prompt_fence::JOB_CAP),
        company: cjk_field(crate::prompt_fence::JOB_CAP),
        url: format!("https://boards.example.com/jobs/{n}"),
        location: Some(cjk_field(crate::prompt_fence::JOB_CAP)),
        board: Some("adzuna".to_string()),
        description: Some(cjk_field(FOUND_JOBS_DESCRIPTION_PREVIEW_CAP)),
        ..full_found_job()
    }
}
