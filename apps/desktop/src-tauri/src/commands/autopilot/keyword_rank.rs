//! Phase 1 of the Autopilot rank: the free, embedding-free keyword prefilter. Posting -> found-job
//! projection, the keyword filters, and the cluster-aware minimum-score retention (plus the
//! re-admission of already-known rows it would otherwise freeze). Everything here is pure.
//!
//! Split out of the command module for the R8 LOC cap; `rerank` is the optional phase 2.

use std::collections::HashSet;

use crate::autopilot::{AutopilotFilter, FoundJob, ScoreSource};
use crate::scraping::JobPosting;

/// `JobPosting.source` of the aggregator board (Adzuna → JSearch). Adzuna caps
/// descriptions to a snippet and its detail pages block anonymous fetches, so a
/// keyword-coverage score computed over that snippet can diverge from the detail
/// pane's full-text re-score (trust-audit root cause 6). A run's aggregator
/// scores are therefore flagged provisional; direct full-text boards are not.
/// Sourced directly from the aggregator scraper's own `id()` constant (not a
/// duplicated literal), so a rename there can't silently desync this check.
pub(super) const AGGREGATOR_SNIPPET_SOURCE: &str =
    crate::scraping::boards::aggregator::AGGREGATOR_BOARD_ID;

/// Pure `JobPosting → FoundJob` projection — the same one `autopilot_run`'s
/// `postings.iter().map(..)` calls. Extracted so a unit test can exercise the
/// REAL projection (every field, plus the
/// `assess_trust(&p.url, &p.company, p.description...)` call and its arg
/// order) instead of a hand-retyped mirror that could silently drift from
/// this one (e.g. a dropped field or swapped args).
pub(crate) fn build_found_job(p: &JobPosting, resume: &str, found_at: u64) -> FoundJob {
    // Keyword-coverage match %: share of the JD's keywords present in the
    // résumé, scored over the SAME blob as `commands::match_resume`
    // (title + description + requirements via `posting_text_blob`).
    // Embedding-free.
    let score = if resume.is_empty() {
        None
    } else {
        crate::documents::keywords::posting_text_blob(
            &p.title,
            p.description.as_deref(),
            p.requirements.as_deref(),
        )
        .map(|blob| crate::documents::keywords::coverage_score(resume, &blob))
    };
    // Whether the scoring blob had any usable description/requirements text,
    // matching `posting_text_blob`'s own notion of "usable" (non-empty after
    // `markdown_to_plain` for description, non-empty after trim for each
    // requirement). Title is deliberately excluded: a title-only blob (e.g.
    // LinkedIn's free-tier `description: Some("")`) is exactly the "don't
    // fully trust this number" case this flags — a title full of
    // résumé-matching words can round to a high coverage % with no JD text
    // behind it at all.
    //
    // Blast radius: this is NOT a LinkedIn-only flag. Six other boards never
    // populate a search-result description either (no detail-enrichment pass
    // runs between the initial scrape and this projection for any of them):
    // TheMuse, Comeet, Breezy, BambooHR, Pinpoint, Rippling. Every row from
    // those boards is honestly title-only too, so it shows the same muted/
    // provisional score marker as LinkedIn's — see
    // `docs/knowledge/matching-algorithm.md`.
    let no_jd_text = crate::documents::keywords::description_is_blank(p.description.as_deref())
        && p.requirements
            .as_deref()
            .map(|reqs| reqs.iter().all(|r| r.trim().is_empty()))
            .unwrap_or(true);
    FoundJob {
        title: p.title.clone(),
        company: p.company.clone(),
        url: p.url.clone(),
        location: p.location.clone(),
        board: {
            let s = p.source.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        },
        board_remote: p
            .extra
            .get("remote")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        description: p.description.clone(),
        salary_min: p.extra.get("salaryMin").and_then(|v| v.as_f64()),
        salary_max: p.extra.get("salaryMax").and_then(|v| v.as_f64()),
        salary_currency: p
            .extra
            .get("salaryCurrency")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        score,
        // Only a real score is qualified: an aggregator (snippet-ranked) score
        // is provisional, and so is any score built with no usable
        // description/requirements text (`no_jd_text` above) regardless of
        // source — a full-text board's score over real JD text is
        // authoritative, and an unscored job (no résumé/blob) is neither.
        score_provisional: score.is_some()
            && (p.source.trim() == AGGREGATOR_SNIPPET_SOURCE || no_jd_text),
        // Phase 1 of the rank always produces a keyword-coverage number. The
        // optional phase-2 semantic re-rank (`semantic_rerank`, only when the
        // user has semantic scoring on) is the ONLY thing that promotes a job to
        // `Combined` — so a build-time default of `Keyword` is always honest,
        // including for a job whose re-rank later degrades.
        score_source: ScoreSource::Keyword,
        found_at,
        // The posting's publish-or-last-updated date, copied straight from the
        // source — most boards report a genuine publish date, but a few
        // (Jooble, Comeet, the Bundesagentur für Arbeit) only expose an
        // "updated"/"current" timestamp upstream (see `FoundJob::posted_at`);
        // a board with no date field at all leaves it `None`.
        posted_at: p.posted_at,
        // Set by the dedup merge in `record_run`; `applied` is derived on read.
        is_new: false,
        applied: false,
        // `p` never went through the engine's streaming wrapper (this Vec is
        // `scraper.search()`'s own separately-returned copy, not the
        // on_item-streamed one `ScraperEngine::run_one` attaches trust to) —
        // compute it directly here, same pure call.
        trust: Some(crate::scraping::trust::assess_trust(
            &p.url,
            &p.company,
            p.description.as_deref().unwrap_or(""),
        )),
        // Set later by the AI-notes step (`generate_assistant_notes`) for the top
        // matches when the autopilot opted in; `None` on every fresh build.
        assistant_notes: None,
        // Cluster annotations are computed + written by `record_run`'s clustering
        // pass (and the retention pass), never at build time — defaults here.
        cluster_id: None,
        cluster_canonical: true,
        cluster_members: Vec::new(),
        is_agency: false,
    }
}

/// Whether a posting passes the autopilot's keyword filters: it must contain
/// **all** must-include keywords and **none** of the exclude keywords, matched
/// case-insensitively against the title + description. Empty/absent lists are
/// no-ops.
pub(super) fn matches_keyword_filters(posting: &JobPosting, filter: &AutopilotFilter) -> bool {
    let haystack = format!(
        "{} {}",
        posting.title.to_lowercase(),
        posting
            .description
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
    );

    if let Some(excludes) = &filter.exclude_keywords {
        let hits_excluded = excludes.iter().any(|k| {
            let k = k.trim().to_lowercase();
            !k.is_empty() && haystack.contains(&k)
        });
        if hits_excluded {
            return false;
        }
    }

    if let Some(keywords) = &filter.keywords {
        let all_present = keywords.iter().all(|k| {
            let k = k.trim().to_lowercase();
            k.is_empty() || haystack.contains(&k)
        });
        if !all_present {
            return false;
        }
    }

    true
}

/// Whether a found job clears the autopilot's `min_match_score`. The score being
/// gated is the keyword-coverage match % (the shared embedding-free kernel from
/// `commands::match_resume`). Postings we could not score (no resume set, or no
/// description to compare against) carry no score and are always kept — the
/// threshold only gates rankable jobs.
pub(super) fn passes_min_score(job: &FoundJob, min_match_score: f64) -> bool {
    job.score.is_none_or(|s| s >= min_match_score)
}

/// The SAME identity `autopilot::merge_key` computes (that helper is private to
/// `autopilot::mod`, so this mirrors its one-line body rather than reach into
/// it) — used below to tell an already-persisted job apart from a genuinely
/// new one when the min-score retain filter runs.
fn found_job_key(j: &FoundJob) -> String {
    crate::scraping::boards::common::canonical_job_key(&j.url, &j.title, &j.company)
}

/// Whether `a` is a better cluster representative than `b` for the min-score
/// gate: a scored member always beats an unscored one, and a higher score beats
/// a lower one. So a cluster's representative is its best-scored member, or (when
/// none is scored) its first member — exactly what "best member passes" needs.
fn is_better_representative(a: &FoundJob, b: &FoundJob) -> bool {
    match (a.score, b.score) {
        (Some(x), Some(y)) => x > y,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Cluster-aware minimum-score retention (ADR-029 §g): cluster the batch with
/// the SAME pass the annotation step uses, then keep EVERY member of a cluster
/// whose representative (best-scored member) clears `threshold` via
/// [`passes_min_score`]. A cluster with no scored member keeps today's
/// keep-unscored behavior. So a below-bar copy survives when a cluster-mate
/// scores well (it still carries a source chip + salary), and a weak member can
/// now "hide" behind a strong one — a deliberate loosening.
///
/// Returns the retained jobs together with THEIR clustering verdicts, in the
/// same order. The verdicts are computed here anyway, and phase 2 needs them to
/// spend one embed per cluster (on the member the UI will display) rather than
/// one per board copy — the alternative, clustering a second time downstream,
/// could disagree with this pass.
pub(super) fn cluster_aware_retain(
    found_jobs: Vec<FoundJob>,
    threshold: f64,
    tombstones: &HashSet<(String, String)>,
    extra_agency: &[String],
) -> (
    Vec<FoundJob>,
    Vec<crate::scraping::cluster::ClusterAssignment>,
) {
    if found_jobs.is_empty() {
        return (found_jobs, Vec::new());
    }
    let inputs = crate::autopilot::found_job_cluster_inputs(&found_jobs);
    let assignments = crate::scraping::cluster::assign_clusters(inputs, tombstones, extra_agency);

    // The representative (best) member index per cluster.
    let mut rep_by_cluster: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for (i, assignment) in assignments.iter().enumerate() {
        let cid = assignment.cluster_id.as_str();
        match rep_by_cluster.get(cid).copied() {
            Some(cur) if !is_better_representative(&found_jobs[i], &found_jobs[cur]) => {}
            _ => {
                rep_by_cluster.insert(cid, i);
            }
        }
    }

    // A cluster passes iff its representative passes the per-member gate. Owned
    // ids: `assignments` is consumed by the zip below (its verdicts travel out
    // with the retained rows), so this set must not borrow from it.
    let passing: HashSet<String> = rep_by_cluster
        .iter()
        .filter(|&(_, &idx)| passes_min_score(&found_jobs[idx], threshold))
        .map(|(&cid, _)| cid.to_string())
        .collect();

    found_jobs
        .into_iter()
        .zip(assignments)
        .filter(|(_, assignment)| passing.contains(&assignment.cluster_id))
        .unzip()
}

/// `cluster_aware_retain` is a visibility filter on which NEW jobs make it
/// into the persisted list — never a staleness gate on a job the store
/// ALREADY knows. `merge_found_jobs` (`autopilot::mod`) only refreshes an
/// existing row's score/`score_provisional`/`score_source` when that row's
/// key reappears in this run's merged batch, so a re-scraped, already-known
/// job whose cluster the retain filter just dropped (e.g. a raised
/// `minMatchScore`) would otherwise leave its persisted score frozen at
/// whatever the last PASSING run left it, with no signal it's stale.
///
/// Re-admits exactly those rows — using their FRESH score from
/// `scored_before_retain` (computed before the filter ran) — into `retained`.
/// A job `persisted` has never seen is left dropped: the filter is still
/// entitled to keep a genuinely new below-bar job out on first sighting. Pure
/// and unit-tested directly; the caller (`autopilot_run`) supplies this AFTER
/// computing its own `kept`/`dropped` counts and running phase 2/AI notes, so
/// a re-admitted row is counted in none of those — it exists in the batch
/// handed to `record_run` purely so the merge can refresh it.
///
/// **Never a kernel downgrade.** Every row `scored_before_retain` carries is
/// necessarily [`ScoreSource::Keyword`] (phase 2's semantic re-rank runs
/// AFTER the retain filter, on `retained` only — see `autopilot_run`), so a
/// row whose PERSISTED score was already [`ScoreSource::Combined`] (a prior
/// run's semantic re-rank) would otherwise have that better score silently
/// overwritten by this run's cheaper one, purely because a truncated/degraded
/// re-scrape happened to score it below the (possibly unchanged) bar.
/// `merge_found_jobs` refreshes `score`/`score_provisional`/`score_source` as
/// one trio whenever the incoming value `is_some()`, with no notion of which
/// kernel is "better" — and `compute_best_matches`'s `qualifies` gate uses a
/// HIGHER cut for `Combined` than `Keyword`, so a downgraded row can fail its
/// own (lower) tier and vanish from best-matches entirely, worse off than if
/// it had just stayed frozen. Freeze the score trio at the persisted values
/// in that one case; every other field on the readmitted row (title/company/
/// description/trust/salary/…) still refreshes normally via
/// `merge_found_jobs`'s existing resurface path. This is directional, not a
/// blanket "ignore fresh Keyword scores": a job that's genuinely Keyword this
/// run and gets upgraded to Combined by phase 2 in the SAME run never goes
/// through this function at all (phase 2 only sees `retained` rows, and a
/// readmitted row is by definition NOT in `retained`), so that upgrade path
/// is untouched.
pub(super) fn readmit_stale_known_jobs(
    retained: Vec<FoundJob>,
    scored_before_retain: &[FoundJob],
    persisted: &[FoundJob],
) -> Vec<FoundJob> {
    let persisted_by_key: std::collections::HashMap<String, &FoundJob> =
        persisted.iter().map(|j| (found_job_key(j), j)).collect();
    let mut present_keys: HashSet<String> = retained.iter().map(found_job_key).collect();
    let mut out = retained;
    for job in scored_before_retain {
        let key = found_job_key(job);
        let Some(&persisted_job) = persisted_by_key.get(&key) else {
            continue; // never seen before — still entitled to stay dropped.
        };
        if !present_keys.insert(key) {
            continue; // already passed retain on its own; readmit is a no-op.
        }
        let mut job = job.clone();
        if persisted_job.score_source == ScoreSource::Combined
            && job.score_source == ScoreSource::Keyword
        {
            job.score = persisted_job.score;
            job.score_source = persisted_job.score_source;
            job.score_provisional = persisted_job.score_provisional;
        }
        out.push(job);
    }
    out
}
