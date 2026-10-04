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
use crate::ipc_contracts::match_tiers::{MATCH_TIER_COMBINED_HIGH, MATCH_TIER_COVERAGE_HIGH};
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
    /// Present only when `score`/`score_source`/`score_provisional` belong to
    /// a DIFFERENT cluster member than the one `url`/the other display fields
    /// describe (the best-scored member decides the score — ADR-036's "always
    /// surface the best available score" — while the content-richest member
    /// decides display; the two are not always the same real posting). Carries
    /// that member's own url so a client can tell, and look up, which actual
    /// posting the displayed score belongs to. `None` means the row is already
    /// self-consistent: `url` IS that member's url.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) score_url: Option<String>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
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

/// The High cut-point for whichever kernel produced a score — read from the
/// generated `MATCH_TIER_CUTS`-derived consts, never hardcoded, so this and
/// the renderer's `scoreTier` can't disagree.
fn high_cut(source: ScoreSource) -> f64 {
    match source {
        ScoreSource::Keyword => MATCH_TIER_COVERAGE_HIGH,
        ScoreSource::Combined => MATCH_TIER_COMBINED_HIGH,
    }
}

/// Whether a score clears its OWN kernel's High cut — the qualification bar.
/// Never a single shared threshold: a `keyword` 60 qualifies, a `combined` 60
/// does not (coverage scores cluster lower than combined ones).
fn qualifies(score: f64, source: ScoreSource) -> bool {
    score >= high_cut(source)
}

/// A `canonical_job_key` degrades to the empty string or the bare separator
/// (`"\u{1}"`) when a record carries no url, title, or company at all (see
/// `canonical_job_key`'s own url-less fallback). Treating either as a real
/// identity would let ONE degenerate dismissed record veto EVERY other
/// equally-degenerate cluster — never a job the user actually asked to hide.
fn is_degenerate_key(key: &str) -> bool {
    key.is_empty() || key == "\u{1}"
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
        /// THIS origin's own `found_at` — kept per-origin (not read off the
        /// surviving `FoundJob` below) because two origins deduped onto the
        /// same key can disagree about when THEY first saw it.
        found_at: u64,
    }

    // Steps 1+2: flat-map every non-archived record's found jobs, tagging
    // each with its origin, then dedupe the UNION on `canonical_job_key`
    // (H3) BEFORE clustering. Two different problems both need this: the
    // ordinary case is the same posting surfacing under two autopilots; the
    // dangerous one is the key itself colliding for two UNRELATED postings
    // (an ATS whose job id lives in the query string —
    // `applications/job_url.rs`). Either way, `assign_clusters`'s
    // `cluster_id = items[seed].key` is only guaranteed unique WITHIN one
    // title/company block — two items sharing a key can resolve in two
    // DIFFERENT blocks and each seed its own cluster under the identical id
    // string, and `by_cluster` (below) then silently unions them: whichever
    // resolved first wins the display fields, and a `clusterMembers` entry
    // can be duplicated once per contributing autopilot. Deduping to ONE
    // entry per key here makes every surviving key globally unique, so that
    // collision can no longer happen. Kept: the best-scored copy (block-aware
    // — the same Combined-beats-Keyword rule a cluster's own representative
    // uses below, so this step can't reintroduce that bug one level up), and
    // EVERY origin (not just the winner's) is unioned — mirrors
    // `merge_found_jobs`'s per-record dedupe, one level higher (across
    // records instead of across runs).
    let mut key_order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, (&FoundJob, Vec<Origin>)> = HashMap::new();
    for ap in records
        .iter()
        .filter(|ap| ap.status != AutopilotStatus::Archived)
    {
        let paused = ap.status == AutopilotStatus::Paused;
        for job in &ap.found_jobs {
            let key = crate::scraping::boards::common::canonical_job_key(
                &job.url,
                &job.title,
                &job.company,
            );
            let origin = Origin {
                autopilot_id: ap.id.clone(),
                autopilot_name: ap.name.clone(),
                paused,
                found_at: job.found_at,
            };
            match by_key.get_mut(&key) {
                Some((kept, origins)) => {
                    if super::rerank::by_rank(job, kept) == std::cmp::Ordering::Less {
                        *kept = job;
                    }
                    origins.push(origin);
                }
                None => {
                    key_order.push(key.clone());
                    by_key.insert(key, (job, vec![origin]));
                }
            }
        }
    }

    if key_order.is_empty() {
        return BestMatchesOutcome::default();
    }

    let jobs: Vec<&FoundJob> = key_order.iter().map(|k| by_key[k].0).collect();
    let origins: Vec<&Vec<Origin>> = key_order.iter().map(|k| &by_key[k].1).collect();

    // Step 3+4: cluster inputs over the WHOLE (now deduped) union, then
    // assign clusters — the SAME pair of calls `cluster_aware_retain` makes
    // for one record.
    let inputs = crate::autopilot::found_job_cluster_inputs(jobs.iter().copied());
    let assignments = assign_clusters(inputs, tombstones, extra_agency);

    // Step 5: group by cluster id. Safe to key straight off `cluster_id` now
    // — every input key is globally unique post-dedupe, so two different
    // blocks can never resolve to the same id.
    let mut by_cluster: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, a) in assignments.iter().enumerate() {
        by_cluster.entry(a.cluster_id.as_str()).or_default().push(i);
    }

    let mut rows: Vec<BestMatchRow> = Vec::new();
    let mut contributing: HashSet<&str> = HashSet::new();

    for (cluster_id, idxs) in &by_cluster {
        // The best-scored member decides qualification, and its
        // score/scoreSource/scoreProvisional travel to the row — picked
        // WITHIN one `score_source` block first (Combined beats Keyword
        // regardless of the raw number), exactly `rerank::by_rank`'s own
        // ordering. `is_better_representative`'s raw `a.score > b.score`
        // compare is only sound single-scale (its one prior caller runs
        // BEFORE the semantic re-rank); this cluster spans the whole union,
        // where a Combined canonical and a Keyword aggregator copy are the
        // NORM once semantic scoring is on (H1).
        let best_idx = idxs
            .iter()
            .copied()
            .min_by(|&a, &b| super::rerank::by_rank(jobs[a], jobs[b]))
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
        // Every index in `idxs` shares the SAME `members` list (attached
        // identically to every member of a resolved cluster by
        // `assign_clusters`), so reading it off `idxs[0]` is safe. A
        // degenerate key (empty url/title/company) is skipped so one
        // degenerate dismissed record can't veto every other equally
        // degenerate cluster (L2).
        let members = &assignments[idxs[0]].members;
        if members
            .iter()
            .any(|m| !is_degenerate_key(&m.key) && dismissed_keys.contains(&m.key))
        {
            continue;
        }

        // Display fields come from the canonical member.
        let canonical_idx = idxs
            .iter()
            .copied()
            .find(|&i| assignments[i].canonical)
            .unwrap_or(idxs[0]);
        let canonical = jobs[canonical_idx];

        // The score above is the BEST-scored member's; `url` below is the
        // CANONICAL member's — two different selection rules over the same
        // cluster, not guaranteed to land on the same real posting (#1104).
        // Compare by url (not index) so the field is only ever populated when
        // it would actually carry NEW information for the caller.
        let score_url = (jobs[best_idx].url != canonical.url).then(|| jobs[best_idx].url.clone());

        // EARLIEST discovery across every ORIGIN (not every deduped job) —
        // two origins deduped onto the same key can carry different
        // `found_at` values even though only one `FoundJob` survived above.
        let found_at = idxs
            .iter()
            .flat_map(|&i| origins[i].iter().map(|o| o.found_at))
            .min()
            .expect("a cluster group has at least one origin");
        // Prefer the canonical member's own note (every OTHER display field
        // already reads from `canonical`), then the best-scored member's
        // (the row's score/scoreSource identity), then any member's — never
        // whichever member happens to be first in `idxs`' iteration order,
        // which on a merged cross-autopilot row can be a different
        // autopilot's résumé/provider context entirely, with no provenance
        // on the payload to say so.
        let assistant_notes = canonical
            .assistant_notes
            .clone()
            .or_else(|| jobs[best_idx].assistant_notes.clone())
            .or_else(|| idxs.iter().find_map(|&i| jobs[i].assistant_notes.clone()));

        // One `BestMatchSource` per distinct contributing autopilot — a
        // `BTreeMap` (not `HashMap`) so a cluster whose members span the same
        // few autopilots always serializes `sources` in the same order.
        let mut per_autopilot: std::collections::BTreeMap<&str, (&str, bool, u64)> =
            std::collections::BTreeMap::new();
        for &i in idxs {
            for o in origins[i] {
                per_autopilot
                    .entry(o.autopilot_id.as_str())
                    .and_modify(|(_, _, found_at)| *found_at = (*found_at).min(o.found_at))
                    .or_insert((o.autopilot_name.as_str(), o.paused, o.found_at));
                contributing.insert(o.autopilot_id.as_str());
            }
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
            score_url,
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

    // Step 9: ADR-020's two-block rule — `Combined` rows first, then
    // `Keyword`, each block score-desc, `key` asc. Not a single cross-scale
    // axis: every row here already cleared its OWN kernel's High cut, so a
    // tier-desc-then-score-desc sort degenerates to a raw score compare — a
    // `keyword` 95 is not "better" than a `combined` 80, they are not on the
    // same scale. `score_block` is the exact rule `rerank::by_rank` uses to
    // order `FoundJob`s, reused here (over `BestMatchRow`) instead of
    // re-derived.
    rows.sort_by(|a, b| {
        super::rerank::score_block(a.score_source)
            .cmp(&super::rerank::score_block(b.score_source))
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

/// Mark every row that already has an Application (ADR 0001) — checked
/// against every `clusterMembers[i].url` (which always includes the
/// canonical's own, since the canonical is itself one of the cluster's
/// members), not just `row.url`. A row's canonical is picked by content
/// richness (`has_description` etc.), not by which board copy the user
/// actually clicked "Apply" from — checking only the canonical url missed a
/// row applied to via a non-canonical copy (M2), leaving `applied: false`
/// and inviting a duplicate application. Pure and unit-tested directly; the
/// one I/O caller is `autopilot_best_matches`.
pub(super) fn mark_applied(rows: &mut [BestMatchRow], applied: &HashSet<String>) {
    if applied.is_empty() {
        return;
    }
    for row in rows.iter_mut() {
        row.applied = row
            .cluster_members
            .iter()
            .any(|m| applied.contains(&crate::applications::normalize_job_url(&m.url)));
    }
}

#[cfg(test)]
mod tests;
