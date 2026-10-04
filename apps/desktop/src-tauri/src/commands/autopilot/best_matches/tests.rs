//! Tests for the cross-autopilot best-matches computation, split by topic: [`merging`] (clusters
//! spanning autopilots), [`representative`] (which member a row stands for), [`qualification`]
//! (what makes the list and in what order) and [`user_state`] (dismissals and applied marking).
//! The fixtures they share live here.

use super::*;
use crate::autopilot::tests::support::{
    autopilot_fixture, found_job_full, no_tombstones, target_fixture,
};
use crate::autopilot::AutopilotTarget;

mod merging;
mod qualification;
mod representative;
mod user_state;

fn job(url: &str, title: &str, company: &str, score: Option<f64>, source: ScoreSource) -> FoundJob {
    FoundJob {
        score,
        score_source: source,
        ..found_job_full(url, title, company, 0)
    }
}

/// `job()` above hardcodes `found_at: 0` (every existing fixture needs
/// it); this overrides it for the tests that actually exercise the
/// EARLIEST-across-sources rule.
fn job_found_at(mut j: FoundJob, found_at: u64) -> FoundJob {
    j.found_at = found_at;
    j
}

fn autopilot(id: &str, status: AutopilotStatus, found_jobs: Vec<FoundJob>) -> Autopilot {
    Autopilot {
        id: id.into(),
        name: format!("autopilot-{id}"),
        status,
        target: AutopilotTarget {
            boards: Vec::new(),
            query: String::new(),
            ..target_fixture()
        },
        total_found: found_jobs.len() as u32,
        found_jobs,
        ..autopilot_fixture()
    }
}

/// An active autopilot holding one keyword-scored job.
fn single(id: &str, url: &str, title: &str, company: &str, score: f64) -> Autopilot {
    autopilot(
        id,
        AutopilotStatus::Active,
        vec![job(url, title, company, Some(score), ScoreSource::Keyword)],
    )
}

fn no_dismissed() -> HashSet<String> {
    HashSet::new()
}

/// Mirrors `scraping::cluster::mod::ordered_pair` (private to that
/// module) — the same `key_a <= key_b` canonical shape `DedupStore::pair`
/// enforces on write.
fn tombstone_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// `compute_best_matches` with no dedup tombstones, no extra agencies and nothing dismissed.
fn best(records: &[Autopilot]) -> BestMatchesOutcome {
    compute_best_matches(records, &no_tombstones(), &[], &no_dismissed())
}
