//! Cross-autopilot "Best Matches": the current top-scoring qualifying jobs
//! across every non-archived autopilot record, recomputed on every call.
//!
//! **Membership is not persisted.** Clustering here runs over the WHOLE UNION
//! of every included record's `found_jobs` — a cluster spanning two
//! autopilots belongs to no single one of them, so there is nowhere on either
//! `Autopilot` record to persist "this job also matched record B" without
//! creating a second source of truth that a later per-record recluster
//! (`commands::autopilot::recluster_autopilot_record`, which only ever
//! reclusters ONE record's own jobs) could silently disagree with.
//! Recomputing at query time instead mirrors the recompute-at-ingest property
//! ADR-029 already relies on for the single-record case — the union
//! clustering is exactly as pure and exactly as cheap to redo on every call.
//!
//! Split into a sibling module for the same LOC-cap reason `rerank` is (see
//! its doc). Everything in this file is pure and unit-tested directly; the
//! `#[tauri::command]` wrapper (I/O: loads the autopilot records + the
//! dedup/interaction/application stores) lives in the parent file.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::autopilot::{Autopilot, AutopilotStatus, FoundJob, ScoreSource};
use crate::ipc_contracts::match_tiers::{
    MATCH_TIER_COMBINED_HIGH, MATCH_TIER_COMBINED_MEDIUM, MATCH_TIER_COVERAGE_HIGH,
    MATCH_TIER_COVERAGE_MEDIUM,
};
use crate::scraping::cluster::{assign_clusters, ClusterMemberRef};
use crate::scraping::trust::TrustAssessment;

/// Payload guard, not the selection rule: qualification (`qualifies`) and the
/// sort below decide what's IN this list; this only bounds how many of those
/// qualifying rows cross the wire in one response.
const BEST_MATCHES_CAP: usize = 100;

/// One autopilot that surfaced a [`BestMatchRow`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BestMatchSource {
    pub(super) autopilot_id: String,
    pub(super) autopilot_name: String,
    pub(super) paused: bool,
    pub(super) found_at: u64,
}

/// One cross-autopilot best-match row — mirrors the shared `AutopilotBestMatch`
/// TS contract field-for-field (`packages/shared/src/ipc/contracts/autopilot.ts`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BestMatchRow {
    pub(super) key: String,
    pub(super) title: String,
    pub(super) company: String,
    pub(super) url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) board: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) salary_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) salary_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) salary_currency: Option<String>,
    pub(super) score: f64,
    pub(super) score_source: ScoreSource,
    pub(super) score_provisional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) posted_at: Option<i64>,
    pub(super) found_at: u64,
    /// Filled in by the command wrapper (`ApplicationStore`) — always `false`
    /// here, mirroring `FoundJob::applied`'s own "never hand-set" contract.
    pub(super) applied: bool,
    pub(super) is_agency: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) trust: Option<TrustAssessment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) assistant_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) cluster_members: Vec<ClusterMemberRef>,
    pub(super) sources: Vec<BestMatchSource>,
}

/// [`compute_best_matches`]'s full result. `total`/`matches.len()` are kept
/// separate (rather than truncating in place and losing the pre-cap count) so
/// a caller — and every test in this file — can tell "capped" from "this is
/// everything".
#[derive(Debug, Default)]
pub(super) struct BestMatchesOutcome {
    pub(super) matches: Vec<BestMatchRow>,
    /// Qualifying count BEFORE [`BEST_MATCHES_CAP`] truncates `matches`.
    pub(super) total: usize,
    /// Distinct autopilots contributing at least one qualifying row.
    pub(super) autopilot_count: usize,
}

/// The (high, medium) cut-point pair for whichever kernel produced a score —
/// read from the generated `MATCH_TIER_CUTS`-derived consts, never hardcoded,
/// so this and the renderer's `scoreTier` can't disagree.
fn cuts(source: ScoreSource) -> (f64, f64) {
    match source {
        ScoreSource::Keyword => (MATCH_TIER_COVERAGE_HIGH, MATCH_TIER_COVERAGE_MEDIUM),
        ScoreSource::Combined => (MATCH_TIER_COMBINED_HIGH, MATCH_TIER_COMBINED_MEDIUM),
    }
}

/// 2 = High, 1 = Medium, 0 = Low — mirrors the renderer's `scoreTier`. Every
/// row that survives [`qualifies`] is High by construction; kept as a real
/// 3-way rank (not a bool) so the sort below stays correct if the
/// qualification bar ever loosens to Medium-or-above.
fn tier_rank(score: f64, source: ScoreSource) -> u8 {
    let (high, medium) = cuts(source);
    if score >= high {
        2
    } else if score >= medium {
        1
    } else {
        0
    }
}

/// Whether a score clears its OWN kernel's High cut — the qualification bar.
/// Never a single shared threshold: a `keyword` 60 qualifies, a `combined` 60
/// does not (coverage scores cluster lower than combined ones).
fn qualifies(score: f64, source: ScoreSource) -> bool {
    score >= cuts(source).0
}

/// Compute the cross-autopilot best-matches list. Pure: every input the
/// command wrapper would otherwise reach through `AppHandle` (the dedup
/// snapshot, which cluster-member keys carry a `dismissed` interaction) is
/// passed in already-resolved, so this is unit-testable with no Tauri
/// runtime. `records` is every autopilot record — archived ones are filtered
/// out HERE (not by the caller), so that exclusion is covered by this file's
/// own tests.
pub(super) fn compute_best_matches(
    records: &[Autopilot],
    tombstones: &HashSet<(String, String)>,
    extra_agency: &[String],
    dismissed_keys: &HashSet<String>,
) -> BestMatchesOutcome {
    struct Origin {
        autopilot_id: String,
        autopilot_name: String,
        paused: bool,
    }

    // Steps 1+2: flat-map every non-archived record's found jobs, tagging
    // each with its origin. Paused records DO contribute — pause only stops
    // an autopilot scraping, it doesn't forget what it already found.
    let mut jobs: Vec<FoundJob> = Vec::new();
    let mut origins: Vec<Origin> = Vec::new();
    for ap in records
        .iter()
        .filter(|ap| ap.status != AutopilotStatus::Archived)
    {
        let paused = ap.status == AutopilotStatus::Paused;
        for job in &ap.found_jobs {
            jobs.push(job.clone());
            origins.push(Origin {
                autopilot_id: ap.id.clone(),
                autopilot_name: ap.name.clone(),
                paused,
            });
        }
    }

    if jobs.is_empty() {
        return BestMatchesOutcome::default();
    }

    // Step 3+4: cluster inputs over the WHOLE union, then assign clusters —
    // the SAME pair of calls `cluster_aware_retain` makes for one record.
    let inputs = crate::autopilot::found_job_cluster_inputs(&jobs);
    let assignments = assign_clusters(inputs, tombstones, extra_agency);

    // Step 5: group by cluster id.
    let mut by_cluster: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, a) in assignments.iter().enumerate() {
        by_cluster.entry(a.cluster_id.as_str()).or_default().push(i);
    }

    let mut rows: Vec<BestMatchRow> = Vec::new();
    let mut contributing: HashSet<&str> = HashSet::new();

    for (cluster_id, idxs) in &by_cluster {
        // The best-scored member decides qualification, and its
        // score/scoreSource/scoreProvisional travel to the row — mirrors
        // `commands::autopilot::is_better_representative`'s own
        // scored-beats-unscored, higher-beats-lower contract.
        let best_idx = idxs
            .iter()
            .copied()
            .reduce(|acc, cand| {
                if super::is_better_representative(&jobs[cand], &jobs[acc]) {
                    cand
                } else {
                    acc
                }
            })
            .expect("a cluster group from `by_cluster` is never empty");

        // Step 6: unscored clusters never qualify; scored ones must clear
        // their own kernel's High cut.
        let Some(score) = jobs[best_idx].score else {
            continue;
        };
        let source = jobs[best_idx].score_source;
        if !qualifies(score, source) {
            continue;
        }

        // Step 7: a `dismissed` interaction against ANY member's own
        // identity drops the whole cluster — `members` already carries each
        // member's `canonical_job_key` (computed once by
        // `found_job_cluster_inputs`), so no extra key derivation is needed.
        let members = &assignments[idxs[0]].members;
        if members.iter().any(|m| dismissed_keys.contains(&m.key)) {
            continue;
        }

        // Display fields come from the canonical member.
        let canonical_idx = idxs
            .iter()
            .copied()
            .find(|&i| assignments[i].canonical)
            .unwrap_or(idxs[0]);
        let canonical = &jobs[canonical_idx];

        let found_at = idxs
            .iter()
            .map(|&i| jobs[i].found_at)
            .min()
            .unwrap_or(canonical.found_at);
        let assistant_notes = idxs.iter().find_map(|&i| jobs[i].assistant_notes.clone());

        // One `BestMatchSource` per distinct contributing autopilot — a
        // `BTreeMap` (not `HashMap`) so a cluster whose members span the same
        // few autopilots always serializes `sources` in the same order.
        let mut per_autopilot: std::collections::BTreeMap<&str, (&str, bool, u64)> =
            std::collections::BTreeMap::new();
        for &i in idxs {
            let o = &origins[i];
            per_autopilot
                .entry(o.autopilot_id.as_str())
                .and_modify(|(_, _, found_at)| *found_at = (*found_at).min(jobs[i].found_at))
                .or_insert((o.autopilot_name.as_str(), o.paused, jobs[i].found_at));
            contributing.insert(o.autopilot_id.as_str());
        }
        let sources: Vec<BestMatchSource> = per_autopilot
            .into_iter()
            .map(
                |(autopilot_id, (autopilot_name, paused, found_at))| BestMatchSource {
                    autopilot_id: autopilot_id.to_string(),
                    autopilot_name: autopilot_name.to_string(),
                    paused,
                    found_at,
                },
            )
            .collect();

        rows.push(BestMatchRow {
            key: (*cluster_id).to_string(),
            title: canonical.title.clone(),
            company: canonical.company.clone(),
            url: canonical.url.clone(),
            location: canonical.location.clone(),
            board: canonical.board.clone(),
            salary_min: canonical.salary_min,
            salary_max: canonical.salary_max,
            salary_currency: canonical.salary_currency.clone(),
            score,
            score_source: source,
            score_provisional: jobs[best_idx].score_provisional,
            posted_at: canonical.posted_at,
            found_at,
            applied: false,
            is_agency: assignments[canonical_idx].is_agency,
            trust: canonical.trust.clone(),
            assistant_notes,
            cluster_members: members.clone(),
            sources,
        });
    }

    // Step 9: (tier desc, score desc, key asc). Every row here already
    // cleared its own High cut, so `tier_rank` is currently a constant 2 —
    // kept anyway (see its doc) rather than sorting on score alone.
    rows.sort_by(|a, b| {
        let ta = tier_rank(a.score, a.score_source);
        let tb = tier_rank(b.score, b.score_source);
        tb.cmp(&ta)
            .then_with(|| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.key.cmp(&b.key))
    });

    // Step 10: `total` is the qualifying count BEFORE the cap.
    let total = rows.len();
    let autopilot_count = contributing.len();
    rows.truncate(BEST_MATCHES_CAP);

    BestMatchesOutcome {
        matches: rows,
        total,
        autopilot_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autopilot::{AutopilotFilter, AutopilotTarget};

    fn job(
        url: &str,
        title: &str,
        company: &str,
        score: Option<f64>,
        source: ScoreSource,
    ) -> FoundJob {
        FoundJob {
            title: title.into(),
            company: company.into(),
            url: url.into(),
            location: None,
            board: None,
            description: None,
            salary_min: None,
            salary_max: None,
            salary_currency: None,
            score,
            score_provisional: false,
            score_source: source,
            found_at: 0,
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

    fn autopilot(id: &str, status: AutopilotStatus, found_jobs: Vec<FoundJob>) -> Autopilot {
        Autopilot {
            id: id.into(),
            name: format!("autopilot-{id}"),
            status,
            target: AutopilotTarget {
                boards: Vec::new(),
                query: String::new(),
                location: None,
                country_code: None,
                work_types: None,
                pages: 1,
                date_filter: None,
                top_n: 3,
                watched_companies_only: None,
            },
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
            total_found: found_jobs.len() as u32,
            total_applied: 0,
            found_jobs,
            run_status: None,
            last_run_summaries: Vec::new(),
            last_run_at: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn no_tombstones() -> HashSet<(String, String)> {
        HashSet::new()
    }

    fn no_dismissed() -> HashSet<String> {
        HashSet::new()
    }

    #[test]
    fn same_listing_across_two_autopilots_merges_into_one_row_with_two_sources() {
        let a = autopilot(
            "a",
            AutopilotStatus::Active,
            vec![job(
                "https://a.example.com/job",
                "Rust Developer",
                "Acme",
                Some(80.0),
                ScoreSource::Keyword,
            )],
        );
        let b = autopilot(
            "b",
            AutopilotStatus::Active,
            vec![job(
                "https://b.example.com/job",
                "Rust Developer",
                "Acme",
                Some(85.0),
                ScoreSource::Keyword,
            )],
        );
        let out = compute_best_matches(&[a, b], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(
            out.matches.len(),
            1,
            "same title+company from two autopilots is one cluster"
        );
        assert_eq!(out.matches[0].sources.len(), 2);
        assert_eq!(out.total, 1);
        assert_eq!(out.autopilot_count, 2);
    }

    #[test]
    fn archived_excluded_paused_included_and_marked() {
        let archived = autopilot(
            "arc",
            AutopilotStatus::Archived,
            vec![job(
                "https://x.example.com/job",
                "Backend Engineer",
                "Widgets Co",
                Some(90.0),
                ScoreSource::Keyword,
            )],
        );
        let paused = autopilot(
            "p",
            AutopilotStatus::Paused,
            vec![job(
                "https://y.example.com/job",
                "Backend Engineer",
                "Gizmos Inc",
                Some(90.0),
                ScoreSource::Keyword,
            )],
        );
        let out = compute_best_matches(&[archived, paused], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(
            out.matches.len(),
            1,
            "an archived record's jobs never appear"
        );
        assert!(
            out.matches[0].sources[0].paused,
            "a paused autopilot's rows are marked paused, not excluded"
        );
    }

    #[test]
    fn qualification_cut_depends_on_score_source() {
        let keyword_ap = autopilot(
            "k",
            AutopilotStatus::Active,
            vec![job(
                "https://k.example.com/job",
                "Data Engineer",
                "KeyCo",
                Some(60.0),
                ScoreSource::Keyword,
            )],
        );
        let combined_ap = autopilot(
            "c",
            AutopilotStatus::Active,
            vec![job(
                "https://c.example.com/job",
                "Data Engineer II",
                "CombCo",
                Some(60.0),
                ScoreSource::Combined,
            )],
        );
        let out = compute_best_matches(
            &[keyword_ap, combined_ap],
            &no_tombstones(),
            &[],
            &no_dismissed(),
        );
        assert_eq!(
            out.matches.len(),
            1,
            "a 60 keyword row qualifies, a 60 combined row does not"
        );
        assert_eq!(out.matches[0].score_source, ScoreSource::Keyword);
    }

    #[test]
    fn dismissed_url_is_dropped() {
        let ap = autopilot(
            "d",
            AutopilotStatus::Active,
            vec![job(
                "https://d.example.com/job",
                "Platform Engineer",
                "Dropco",
                Some(90.0),
                ScoreSource::Keyword,
            )],
        );
        let dismissed_key = crate::scraping::boards::common::canonical_job_key(
            "https://d.example.com/job",
            "Platform Engineer",
            "Dropco",
        );
        let dismissed: HashSet<String> = [dismissed_key].into_iter().collect();
        let out = compute_best_matches(&[ap], &no_tombstones(), &[], &dismissed);
        assert!(
            out.matches.is_empty(),
            "a dismissed url's cluster never qualifies"
        );
        assert_eq!(out.total, 0);
    }

    #[test]
    fn total_counts_qualifying_rows_before_the_cap() {
        let jobs: Vec<FoundJob> = (0..120)
            .map(|i| {
                job(
                    &format!("https://many.example.com/job/{i}"),
                    &format!("Engineer {i}"),
                    &format!("Co{i}"),
                    Some(90.0),
                    ScoreSource::Keyword,
                )
            })
            .collect();
        let ap = autopilot("many", AutopilotStatus::Active, jobs);
        let out = compute_best_matches(&[ap], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(out.total, 120, "total is the pre-cap qualifying count");
        assert!(
            out.matches.len() < out.total,
            "matches is capped, total is not"
        );
        assert_eq!(out.matches.len(), BEST_MATCHES_CAP);
    }

    #[test]
    fn unscored_clusters_never_qualify() {
        let ap = autopilot(
            "u",
            AutopilotStatus::Active,
            vec![job(
                "https://u.example.com/job",
                "Support Engineer",
                "Unco",
                None,
                ScoreSource::Keyword,
            )],
        );
        let out = compute_best_matches(&[ap], &no_tombstones(), &[], &no_dismissed());
        assert!(out.matches.is_empty());
        assert_eq!(out.total, 0);
    }
}
