//! Fixtures shared by the autopilot store's tests and by the other autopilot-family test
//! modules (`autopilot_helpers`, `autopilot_scheduler`, `commands::autopilot`).

use std::path::PathBuf;

use tempfile::TempDir;

use crate::autopilot::{
    Autopilot, AutopilotFilter, AutopilotStatus, AutopilotStore, AutopilotTarget, FoundJob,
};

/// A fresh store over a throwaway data dir. Keep the `TempDir` alive for as long as the store
/// (or anything under the dir) is used.
pub(crate) fn temp_store() -> (TempDir, AutopilotStore) {
    let temp = TempDir::new().unwrap();
    let store = AutopilotStore::new(&temp.path().to_path_buf());
    (temp, store)
}

/// A throwaway data dir, for tests that open the store (or write the files under it) themselves.
pub(crate) fn temp_dir() -> (TempDir, PathBuf) {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().to_path_buf();
    (temp, dir)
}

/// `store.create(..)` for the record most tests need: one board, query `rust`, one page.
pub(crate) fn create_ap(
    store: &AutopilotStore,
    name: &str,
    board: &str,
    min_match_score: f64,
    schedule: &str,
) -> Autopilot {
    store.create(serde_json::json!({
        "name": name,
        "target": { "board": board, "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": min_match_score },
        "schedule": schedule,
    }))
}

/// `record_run` for a run with no board summaries, no dedup tombstones and no extra agencies —
/// everything except the jobs is incidental to the test.
pub(crate) fn record(
    store: &AutopilotStore,
    id: &str,
    total_found: u32,
    found_jobs: Vec<FoundJob>,
) -> u32 {
    store.record_run(
        id,
        total_found,
        0,
        found_jobs,
        Vec::new(),
        &no_tombstones(),
        &[],
    )
}

/// A neutral, fully-populated target: one board, query `rust`, one page, no optional field set.
pub(crate) fn target_fixture() -> AutopilotTarget {
    AutopilotTarget {
        boards: vec!["linkedin".into()],
        query: "rust".into(),
        location: None,
        country_code: None,
        work_types: None,
        pages: 1,
        date_filter: None,
        top_n: 3,
        watched_companies_only: None,
    }
}

/// A neutral, fully-populated `Autopilot`: active, manual, no filters, zeroed timestamps. Tests
/// override only the fields they care about with struct-update syntax.
pub(crate) fn autopilot_fixture() -> Autopilot {
    Autopilot {
        id: "id".into(),
        name: "name".into(),
        status: AutopilotStatus::Active,
        target: target_fixture(),
        filter: AutopilotFilter {
            min_match_score: 0.0,
            keywords: None,
            exclude_keywords: None,
        },
        schedule: "manual".into(),
        schedule_hour: None,
        schedule_minute: None,
        resume_text: None,
        cover_letter: None,
        assistant: false,
        assistant_provider: None,
        assistant_model: None,
        assistant_base_url: None,
        total_found: 0,
        total_applied: 0,
        found_jobs: Vec::new(),
        run_status: None,
        last_run_summaries: Vec::new(),
        last_run_at: None,
        created_at: 0,
        updated_at: 0,
    }
}

/// Build a per-board summary for the status-derivation tests. `count` is only
/// nonzero for the "succeeded" cases so a reader can tell them apart.
pub(crate) fn board_summary(
    board: &str,
    count: usize,
    error: Option<&str>,
    skipped: Option<&str>,
    truncated: Option<&str>,
) -> crate::scraping::BoardScrapeSummary {
    crate::scraping::BoardScrapeSummary {
        board: board.into(),
        count,
        error: error.map(String::from),
        skipped: skipped.map(String::from),
        truncated: truncated.map(String::from),
        notes: Vec::new(),
        health: None,
    }
}

pub(crate) fn found_job(url: &str, found_at: u64) -> FoundJob {
    found_job_full(url, "Engineer", "Acme", found_at)
}

/// A [`FoundJob`] with an explicit title + company, so clustering-sensitive
/// tests can control whether two rows share a block (same title+company → one
/// cluster) or stay distinct.
pub(crate) fn found_job_full(url: &str, title: &str, company: &str, found_at: u64) -> FoundJob {
    FoundJob {
        title: title.into(),
        company: company.into(),
        url: url.into(),
        location: None,
        board: None,
        board_remote: false,
        description: None,
        salary_min: None,
        salary_max: None,
        salary_currency: None,
        score: None,
        score_provisional: false,
        score_source: crate::autopilot::ScoreSource::Keyword,
        found_at,
        posted_at: None,
        is_new: false,
        applied: false,
        trust: None,
        assistant_notes: None,
        cluster_id: None,
        cluster_canonical: true,
        cluster_members: Vec::new(),
        is_agency: false,
    }
}

/// Empty tombstone set for record_run calls that don't exercise splits.
pub(crate) fn no_tombstones() -> std::collections::HashSet<(String, String)> {
    std::collections::HashSet::new()
}
