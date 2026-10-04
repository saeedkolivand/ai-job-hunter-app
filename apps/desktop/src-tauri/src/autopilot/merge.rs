//! Folding a run's postings into an autopilot's found-jobs list, and the
//! cross-board clustering annotations written onto it (ADR-029).

use std::collections::HashSet;

use super::FoundJob;
use crate::scraping::cluster::{assign_clusters, ClusterAssignment, ClusterInput};

/// Canonical dedup key for a [`FoundJob`] — a thin `FoundJob` adapter over the
/// app-wide [`canonical_job_key`](crate::scraping::boards::common::canonical_job_key),
/// the single source of truth for "is this the same job?" shared with the scrape
/// engine's cross-source pass and (mirrored in TS) the renderer's `mergePostings`.
/// Forwarding the posting's `url`/`title`/`company` keeps autopilot's merge keyed
/// byte-for-byte identically to those, so a job surfaced by two sources collapses to
/// one row and fires one notification — and persisted found-jobs keyed under the old
/// inlined copy recompute to the same key (the algorithm is unchanged, only DRYed).
/// The aggregator-redirect-vs-direct-URL limitation is documented on that fn.
pub(super) fn merge_key(j: &FoundJob) -> String {
    crate::scraping::boards::common::canonical_job_key(&j.url, &j.title, &j.company)
}

/// Byte length of a job's description (0 when absent) — used to keep the richer of
/// two same-key postings when collapsing a within-batch duplicate.
fn description_len(j: &FoundJob) -> usize {
    j.description.as_deref().map(str::len).unwrap_or(0)
}

/// Merge a fresh run's postings into the cumulative found-jobs list, idempotently:
///
/// - the incoming batch is first collapsed on [`merge_key`], so the SAME job
///   surfaced twice in one batch, or via two tracking-param/hash URL variants,
///   becomes ONE row — and counts once in the "N new jobs" notification (this
///   does NOT collapse an aggregator's redirect URL against a board's direct
///   URL for the same job; see [`merge_key`]),
/// - existing rows are kept (preserving `found_at`/first-seen) and have `is_new`
///   cleared; if the run re-surfaced them (matched by `merge_key`), volatile
///   fields (title/company/description/score/location/salary/board/trust) refresh,
/// - postings whose key was never seen are placed first (on top) and flagged
///   `is_new`, in the incoming (already score-sorted) order.
///
/// Re-running with the same postings yields the same set — only `is_new` moves.
/// `applied` is derived on read, so it is left at its default here.
pub(super) fn merge_found_jobs(existing: &[FoundJob], incoming: Vec<FoundJob>) -> Vec<FoundJob> {
    use std::collections::{HashMap, HashSet};

    // 1) Collapse duplicates WITHIN the incoming batch by canonical key. First
    //    occurrence keeps its position; a later duplicate carrying a longer
    //    description upgrades that one field (richer text for the tailor flow).
    //    (The engine's `dedup_cross_source` runs upstream over the FULL
    //    multi-board batch, keyed on the exact same `canonical_job_key` that
    //    `build_found_job` copies verbatim into `merge_key` (url/title/company
    //    are never mutated in between) — so via the one production caller
    //    (`commands::autopilot::autopilot_run`) this `Some(kept)` branch is
    //    never actually hit: every duplicate, same-source or cross-source, was
    //    already collapsed — including its `posted_at`, via the same
    //    fill-without-clobbering pattern this file uses — before `incoming` was
    //    built. This pass is `merge_found_jobs`'s own defensive contract for a
    //    caller that hands it pre-existing duplicates directly, e.g. a test.)
    let mut order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, FoundJob> = HashMap::new();
    for job in incoming {
        let key = merge_key(&job);
        match by_key.get_mut(&key) {
            Some(kept) => {
                if description_len(&job) > description_len(kept) {
                    kept.description = job.description;
                }
            }
            None => {
                order.push(key.clone());
                by_key.insert(key, job);
            }
        }
    }
    let incoming: Vec<FoundJob> = order
        .into_iter()
        .map(|k| by_key.remove(&k).expect("key inserted above"))
        .collect();

    // 2) Merge the de-duplicated batch against existing rows, keyed the same way so
    //    a re-surfaced posting matches regardless of which URL variant/source
    //    persisted it.
    let incoming_by_key: HashMap<String, &FoundJob> =
        incoming.iter().map(|j| (merge_key(j), j)).collect();

    let refreshed_existing: Vec<FoundJob> = existing
        .iter()
        .map(|e| {
            let mut row = e.clone();
            row.is_new = false;
            if let Some(inc) = incoming_by_key.get(&merge_key(e)) {
                row.title = inc.title.clone();
                row.company = inc.company.clone();
                if inc.location.is_some() {
                    row.location = inc.location.clone();
                }
                // Carry the board over: existing rows persisted before `board` existed
                // (`None`) pick it up when the same job re-surfaces; the append branch
                // (`..inc`) preserves it for never-seen jobs.
                if inc.board.is_some() {
                    row.board = inc.board.clone();
                }
                // NOT `inc.description.is_some()` like the other fields above: a
                // LinkedIn search result always carries `Some("")` (never `None`)
                // for its description — that is its "unknown" sentinel, not
                // `None`. An `is_some()` guard would let that blank resurface
                // every run and clobber a real description the enrichment pass
                // (`autopilot_helpers::linkedin_enrich`) had already fetched and
                // written back — see
                // `merge_preserves_a_real_description_over_a_blank_resurface`.
                if !crate::documents::keywords::description_is_blank(inc.description.as_deref()) {
                    row.description = inc.description.clone();
                }
                // Same fill-without-clobbering pattern as `board`/`description`: an
                // existing row persisted before `posted_at` existed (`None`), or
                // whose board didn't expose a publish date on an earlier run, picks
                // it up when the same job re-surfaces with one known.
                if inc.posted_at.is_some() {
                    row.posted_at = inc.posted_at;
                }
                if inc.score.is_some() {
                    // Paired fields — `score_provisional` and `score_source`
                    // both describe WHICH score is on the row, so they must move
                    // with `score`, never be left stale from a prior source
                    // (e.g. a full-text board's authoritative score resurfacing
                    // over an old aggregator snippet score, or vice versa — a
                    // snippet score must never display as authoritative; and a
                    // run where the user turned semantic scoring OFF must not
                    // leave last run's "combined" label on a keyword number).
                    row.score = inc.score;
                    row.score_provisional = inc.score_provisional;
                    row.score_source = inc.score_source;
                }
                // Same fill-without-clobbering pattern as `board`/`description`: a
                // re-scrape that newly learns the salary updates the row, but never
                // overwrites an already-known value with an unknown one.
                if inc.salary_min.is_some() {
                    row.salary_min = inc.salary_min;
                }
                if inc.salary_max.is_some() {
                    row.salary_max = inc.salary_max;
                }
                if inc.salary_currency.is_some() {
                    row.salary_currency = inc.salary_currency.clone();
                }
                // Same legacy-migration case as `board` above: an existing row
                // persisted before `trust` existed (`None`) picks it up when the
                // same job re-surfaces on a later run.
                if inc.trust.is_some() {
                    row.trust = inc.trust.clone();
                }
            }
            row
        })
        .collect();

    let existing_keys: HashSet<String> = existing.iter().map(merge_key).collect();
    // New jobs go on top: the batch is already score-sorted desc upstream
    // (commands/autopilot/run.rs), so preserving incoming order here keeps that.
    let mut merged: Vec<FoundJob> = incoming
        .into_iter()
        .filter(|inc| !existing_keys.contains(&merge_key(inc)))
        .map(|inc| FoundJob {
            is_new: true,
            applied: false,
            ..inc
        })
        .collect();
    merged.extend(refreshed_existing);

    merged
}

// ── Cross-board clustering (ADR-029) ────────────────────────────────────────

/// Build [`ClusterInput`]s for a found-jobs list. `key` is the canonical
/// [`merge_key`] (the identity tombstones + the renderer share). Vectors are
/// always `None` on this path — a `FoundJob` has no posting id, so there is no
/// cached embedding to look up (the string path is structurally primary here,
/// ADR-029 §c). `pub(crate)` so the L3 retain pass reuses the exact projection.
///
/// Generic over anything that yields `&FoundJob` (a slice reference, `.iter()`,
/// or any other borrowing iterator) rather than `&[FoundJob]` specifically —
/// `commands::autopilot::best_matches::compute_best_matches` builds its union
/// as `Vec<&FoundJob>` (borrowing straight from the already-owned autopilot
/// records) so this projection never forces a second full-struct clone of
/// every found job just to feed it.
pub(crate) fn found_job_cluster_inputs<'a>(
    jobs: impl IntoIterator<Item = &'a FoundJob>,
) -> Vec<ClusterInput> {
    jobs.into_iter()
        .map(|j| ClusterInput {
            key: merge_key(j),
            title: j.title.clone(),
            company: j.company.clone(),
            url: j.url.clone(),
            source: j.board.clone(),
            has_description: j
                .description
                .as_deref()
                .is_some_and(|d| !d.trim().is_empty()),
            seen_at: j.found_at,
            vector: None,
            space: None,
        })
        .collect()
}

/// Cluster a found-jobs list IN PLACE: run [`assign_clusters`] and write each
/// verdict (`cluster_id`, `cluster_canonical`, `cluster_members`, `is_agency`)
/// onto the matching row by index. Returns the assignments so the caller can
/// derive the new-cluster count.
pub(super) fn cluster_found_jobs(
    jobs: &mut [FoundJob],
    tombstones: &HashSet<(String, String)>,
    extra_agency: &[String],
) -> Vec<ClusterAssignment> {
    let inputs = found_job_cluster_inputs(jobs.iter());
    let assignments = assign_clusters(inputs, tombstones, extra_agency);
    for (job, assignment) in jobs.iter_mut().zip(assignments.iter()) {
        job.cluster_id = Some(assignment.cluster_id.clone());
        job.cluster_canonical = assignment.canonical;
        job.cluster_members = assignment.members.clone();
        job.is_agency = assignment.is_agency;
    }
    assignments
}
