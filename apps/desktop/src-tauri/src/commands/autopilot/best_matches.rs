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
    fn identical_url_from_two_autopilots_dedupes_cluster_members_to_one() {
        // The EXACT same posting url, found independently by two autopilots.
        // Before H3's pre-clustering dedupe this produced TWO identical
        // `ClusterMemberRef`s (one per input item) even though there is only
        // one real board copy — any `clusterMembers.length > 1` gate would
        // misread that as "found on 2 boards".
        let a = autopilot(
            "a",
            AutopilotStatus::Active,
            vec![job(
                "https://jobs.lever.co/acme/123",
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
                "https://jobs.lever.co/acme/123",
                "Rust Developer",
                "Acme",
                Some(85.0),
                ScoreSource::Keyword,
            )],
        );
        let out = compute_best_matches(&[a, b], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(out.matches.len(), 1);
        assert_eq!(
            out.matches[0].cluster_members.len(),
            1,
            "the identical url is ONE cluster member, not one per contributing autopilot"
        );
        assert_eq!(
            out.matches[0].sources.len(),
            2,
            "both autopilots are still credited as sources"
        );
        assert_eq!(
            out.matches[0].score, 85.0,
            "the better-scored duplicate copy wins"
        );
    }

    #[test]
    fn best_scored_member_wins_within_its_own_scale_even_when_not_canonical() {
        // Cluster = {canonical: combined 40, full JD, direct board}
        //         u {non-canonical: keyword 60, aggregator snippet, no JD}.
        // `resolve_block`'s canonical-preference (has_description desc,
        // non-aggregator source first) picks the FIRST as canonical even
        // though its raw number is lower than the second's — exactly the
        // shape H1 broke: a raw cross-scale compare would have picked the
        // higher-numbered keyword aggregator copy as the row's score,
        // mislabeling a genuinely-scored semantic match as a much stronger
        // "combined" number than the semantic kernel actually gave it. Both
        // scores clear their OWN High cut, so the cluster still qualifies
        // either way — this isolates the block-selection question from
        // `qualifies`.
        let combined_score = MATCH_TIER_COMBINED_HIGH + 1.0;
        let keyword_score = MATCH_TIER_COVERAGE_HIGH + 35.0;
        assert!(
            keyword_score > combined_score,
            "fixture assumption: the keyword number reads as \"better\" raw"
        );
        let canonical_job = FoundJob {
            description: Some("full JD text".into()),
            board: Some("greenhouse".into()),
            ..job(
                "https://x.example.com/job",
                "Senior Rust Engineer",
                "Acme",
                Some(combined_score),
                ScoreSource::Combined,
            )
        };
        let aggregator_copy = FoundJob {
            description: None,
            board: Some(crate::scraping::boards::aggregator::AGGREGATOR_BOARD_ID.into()),
            ..job(
                "https://agg.example.com/job?id=1",
                "Senior Rust Engineer",
                "Acme",
                Some(keyword_score),
                ScoreSource::Keyword,
            )
        };
        let ap = autopilot(
            "a",
            AutopilotStatus::Active,
            vec![canonical_job, aggregator_copy],
        );
        let out = compute_best_matches(&[ap], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(
            out.matches.len(),
            1,
            "identical title+company joins one cluster"
        );
        let row = &out.matches[0];
        assert_eq!(
            row.score_source,
            ScoreSource::Combined,
            "Combined beats Keyword regardless of the raw number"
        );
        assert_eq!(row.score, combined_score);
        assert_eq!(
            row.board.as_deref(),
            Some("greenhouse"),
            "display fields still come from the CANONICAL member, not the best-scored one"
        );
    }

    #[test]
    fn assistant_notes_prefer_canonical_over_first_in_input_order() {
        // First-in-input-order member is NOT canonical (no description, so
        // `resolve_block`'s canonical-preference ranks it below the second
        // member) and carries its OWN note. The canonical member (full JD)
        // carries a DIFFERENT note. The canonical's note must win — the same
        // member every other display field (title/company/url/board/...)
        // already reads from.
        let first_in_input = FoundJob {
            description: None,
            assistant_notes: Some("note from the first-found aggregator copy".into()),
            ..job(
                "https://agg.example.com/job?id=1",
                "Senior Rust Engineer",
                "Acme",
                Some(90.0),
                ScoreSource::Keyword,
            )
        };
        let canonical_job = FoundJob {
            description: Some("full JD text".into()),
            assistant_notes: Some("note from the canonical board copy".into()),
            ..job(
                "https://x.example.com/job",
                "Senior Rust Engineer",
                "Acme",
                Some(80.0),
                ScoreSource::Keyword,
            )
        };
        let ap = autopilot(
            "a",
            AutopilotStatus::Active,
            vec![first_in_input, canonical_job],
        );
        let out = compute_best_matches(&[ap], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(
            out.matches.len(),
            1,
            "identical title+company joins one cluster"
        );
        assert_eq!(
            out.matches[0].assistant_notes.as_deref(),
            Some("note from the canonical board copy"),
            "assistant_notes must come from the canonical member, not whichever member \
             happens to be first in input order"
        );
    }

    #[test]
    fn combined_block_sorts_before_keyword_block_regardless_of_raw_score() {
        let keyword_score = MATCH_TIER_COVERAGE_HIGH + 40.0;
        let combined_score = MATCH_TIER_COMBINED_HIGH + 5.0;
        let hot_keyword = autopilot(
            "hk",
            AutopilotStatus::Active,
            vec![job(
                "https://k.example.com/job",
                "A Engineer",
                "AltCo",
                Some(keyword_score),
                ScoreSource::Keyword,
            )],
        );
        let modest_combined = autopilot(
            "mc",
            AutopilotStatus::Active,
            vec![job(
                "https://c.example.com/job",
                "B Engineer",
                "BravoCo",
                Some(combined_score),
                ScoreSource::Combined,
            )],
        );
        let out = compute_best_matches(
            &[hot_keyword, modest_combined],
            &no_tombstones(),
            &[],
            &no_dismissed(),
        );
        assert_eq!(out.matches.len(), 2);
        assert_eq!(
            out.matches[0].score_source,
            ScoreSource::Combined,
            "the combined block sorts FIRST even though its raw number ({combined_score}) is \
             lower than the keyword row's ({keyword_score}) — the two axes are not comparable"
        );
        assert_eq!(out.matches[1].score_source, ScoreSource::Keyword);
    }

    #[test]
    fn tombstone_veto_splits_a_cross_autopilot_near_duplicate_into_two_rows() {
        let key_a = crate::scraping::boards::common::canonical_job_key(
            "https://a.example.com/job1",
            "Senior Rust Engineer",
            "Acme",
        );
        let key_b = crate::scraping::boards::common::canonical_job_key(
            "https://b.example.com/job2",
            "Senior Rust Engineer",
            "Acme",
        );
        let a = autopilot(
            "a",
            AutopilotStatus::Active,
            vec![job(
                "https://a.example.com/job1",
                "Senior Rust Engineer",
                "Acme",
                Some(90.0),
                ScoreSource::Keyword,
            )],
        );
        let b = autopilot(
            "b",
            AutopilotStatus::Active,
            vec![job(
                "https://b.example.com/job2",
                "Senior Rust Engineer",
                "Acme",
                Some(85.0),
                ScoreSource::Keyword,
            )],
        );
        let tombstones: HashSet<(String, String)> =
            [tombstone_pair(&key_a, &key_b)].into_iter().collect();
        let out = compute_best_matches(&[a, b], &tombstones, &[], &no_dismissed());
        assert_eq!(
            out.matches.len(),
            2,
            "a tombstoned pair never joins, even across autopilots"
        );
        assert_eq!(out.total, 2);
        for row in &out.matches {
            assert_eq!(row.sources.len(), 1);
        }
    }

    #[test]
    fn autopilot_count_excludes_non_contributing_autopilots() {
        let a = autopilot(
            "a",
            AutopilotStatus::Active,
            vec![job(
                "https://a.example.com/job",
                "Data Engineer",
                "AlphaCo",
                Some(90.0),
                ScoreSource::Keyword,
            )],
        );
        let b = autopilot(
            "b",
            AutopilotStatus::Active,
            vec![job(
                "https://b.example.com/job",
                "Data Scientist",
                "BetaCo",
                Some(90.0),
                ScoreSource::Keyword,
            )],
        );
        let c = autopilot(
            "c",
            AutopilotStatus::Active,
            vec![job(
                "https://c.example.com/job",
                "Data Analyst",
                "GammaCo",
                Some(10.0),
                ScoreSource::Keyword,
            )],
        );
        let out = compute_best_matches(&[a, b, c], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(out.matches.len(), 2);
        assert_eq!(
            out.autopilot_count, 2,
            "an autopilot with zero qualifying rows doesn't count"
        );
    }

    #[test]
    fn found_at_is_the_earliest_across_cluster_members() {
        let a = autopilot(
            "a",
            AutopilotStatus::Active,
            vec![job_found_at(
                job(
                    "https://a.example.com/job",
                    "Senior Rust Engineer",
                    "Acme",
                    Some(90.0),
                    ScoreSource::Keyword,
                ),
                500,
            )],
        );
        let b = autopilot(
            "b",
            AutopilotStatus::Active,
            vec![job_found_at(
                job(
                    "https://b.example.com/job",
                    "Senior Rust Engineer",
                    "Acme",
                    Some(80.0),
                    ScoreSource::Keyword,
                ),
                100,
            )],
        );
        let out = compute_best_matches(&[a, b], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(out.matches.len(), 1);
        assert_eq!(
            out.matches[0].found_at, 100,
            "row found_at is the EARLIEST across all sources, not the best-scored member's own"
        );
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
        // A score that clears the (lower) coverage cut but not the (higher)
        // combined cut — derived from the consts so this stays correct if
        // either cut moves (both are documented "not calibrated"), rather
        // than pinning today's specific numbers.
        let score = MATCH_TIER_COVERAGE_HIGH + 1.0;
        assert!(
            score < MATCH_TIER_COMBINED_HIGH,
            "fixture assumption: the coverage High cut sits below the combined one"
        );
        let keyword_ap = autopilot(
            "k",
            AutopilotStatus::Active,
            vec![job(
                "https://k.example.com/job",
                "Data Engineer",
                "KeyCo",
                Some(score),
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
                Some(score),
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
            "a coverage-qualifying score does not also qualify under the combined cut"
        );
        assert_eq!(out.matches[0].score_source, ScoreSource::Keyword);
    }

    #[test]
    fn qualifies_at_the_exact_high_cut_for_both_kernels() {
        // The boundary is reachable in practice (coverage is a `matched /
        // total * 100` percentage, so an exact 55.0 is a real score) and the
        // renderer's `scoreTier` uses `>=` too — both must agree at the cut,
        // not just above it.
        let keyword_ap = autopilot(
            "k",
            AutopilotStatus::Active,
            vec![job(
                "https://k.example.com/job",
                "Data Engineer",
                "KeyCo",
                Some(MATCH_TIER_COVERAGE_HIGH),
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
                Some(MATCH_TIER_COMBINED_HIGH),
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
            2,
            "a score exactly AT the High cut qualifies, for both kernels"
        );
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
    fn dismissed_key_on_a_non_canonical_member_still_drops_the_cluster() {
        // A two-member cluster where the dismissed identity belongs to the
        // NON-canonical copy — `dismissed_url_is_dropped` above uses a
        // single-member cluster where the only member IS the canonical, so
        // it can't tell a per-member scan apart from a
        // `dismissed_keys.contains(cluster_id)` shortcut. This can.
        let canonical_job = FoundJob {
            description: Some("full JD".into()),
            ..job(
                "https://x.example.com/job",
                "Senior Rust Engineer",
                "Acme",
                Some(90.0),
                ScoreSource::Keyword,
            )
        };
        let dup_url = "https://agg.example.com/job?id=9";
        let dup_title = "Senior Rust Engineer";
        let dup_company = "Acme";
        let non_canonical = FoundJob {
            description: None,
            ..job(
                dup_url,
                dup_title,
                dup_company,
                Some(60.0),
                ScoreSource::Keyword,
            )
        };
        let ap = autopilot(
            "a",
            AutopilotStatus::Active,
            vec![canonical_job, non_canonical],
        );
        let dismissed_key =
            crate::scraping::boards::common::canonical_job_key(dup_url, dup_title, dup_company);
        let dismissed: HashSet<String> = [dismissed_key].into_iter().collect();
        let out = compute_best_matches(&[ap], &no_tombstones(), &[], &dismissed);
        assert!(
            out.matches.is_empty(),
            "dismissing the NON-canonical copy's own identity still drops the whole cluster"
        );
    }

    #[test]
    fn degenerate_dismissed_key_does_not_drop_a_blank_identity_job() {
        // A job with no url/title/company at all derives the degenerate
        // `canonical_job_key` fallback (the bare "\u{1}" separator, both
        // halves empty). `is_degenerate_key` exists so a dismissal record
        // that ALSO derived to this same meaningless identity — e.g.
        // persisted against a different, equally-blank posting — can't veto
        // this unrelated one. Without the guard, `dismissed_keys.contains`
        // matches on the bare "\u{1}" and the job silently disappears.
        let degenerate_key = crate::scraping::boards::common::canonical_job_key("", "", "");
        assert_eq!(
            degenerate_key, "\u{1}",
            "fixture assumption: blank url/title/company derives the bare separator"
        );
        let ap = autopilot(
            "blank",
            AutopilotStatus::Active,
            vec![job("", "", "", Some(90.0), ScoreSource::Keyword)],
        );
        let dismissed: HashSet<String> = [degenerate_key].into_iter().collect();
        let out = compute_best_matches(&[ap], &no_tombstones(), &[], &dismissed);
        assert_eq!(
            out.matches.len(),
            1,
            "a degenerate dismissed key must not veto a job whose own identity is equally degenerate"
        );
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

    #[test]
    fn mark_applied_matches_a_non_canonical_board_copy() {
        // The canonical url is the direct-board copy; the user actually
        // applied through the Adzuna redirect, a NON-canonical member
        // (M2). Checking only `row.url` would miss this.
        let ap = autopilot(
            "m",
            AutopilotStatus::Active,
            vec![
                FoundJob {
                    description: Some("full JD".into()),
                    ..job(
                        "https://direct.example.com/job",
                        "Senior Rust Engineer",
                        "Acme",
                        Some(90.0),
                        ScoreSource::Keyword,
                    )
                },
                FoundJob {
                    description: None,
                    board: Some(crate::scraping::boards::aggregator::AGGREGATOR_BOARD_ID.into()),
                    ..job(
                        "https://redirect.example.com/job?id=1",
                        "Senior Rust Engineer",
                        "Acme",
                        Some(60.0),
                        ScoreSource::Keyword,
                    )
                },
            ],
        );
        let out = compute_best_matches(&[ap], &no_tombstones(), &[], &no_dismissed());
        assert_eq!(out.matches.len(), 1);
        assert_eq!(
            out.matches[0].url, "https://direct.example.com/job",
            "the canonical (richer) copy is the direct-board one"
        );

        let mut matches = out.matches;
        let applied: HashSet<String> = [crate::applications::normalize_job_url(
            "https://redirect.example.com/job?id=1",
        )]
        .into_iter()
        .collect();
        mark_applied(&mut matches, &applied);
        assert!(
            matches[0].applied,
            "applied via a non-canonical cluster member still marks the row applied"
        );
    }
}
