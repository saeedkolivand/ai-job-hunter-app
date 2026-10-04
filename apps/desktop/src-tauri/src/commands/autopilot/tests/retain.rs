//! Cluster-aware minimum-score retention (ADR-029 §g) and the re-admission of already-known rows it
//! would otherwise freeze.

use std::collections::HashSet;

use super::super::keyword_rank::{cluster_aware_retain, readmit_stale_known_jobs};
use super::support::*;
use crate::autopilot::tests::support::{create_ap, temp_store};
use crate::autopilot::{AutopilotStore, FoundJob, ScoreSource};

// ── cluster-aware retention (ADR-029 §g) ───────────────────────────────────

#[test]
fn cluster_aware_retain_keeps_below_bar_member_of_passing_cluster() {
    // Two board copies of the SAME job (same title+company, different urls)
    // form ONE cluster. The strong copy (80) clears the 50 bar, so the whole
    // cluster — including the below-bar (40) copy — is retained.
    let strong = ranked("https://a.example.com/job", 80.0);
    let weak = ranked("https://b.example.com/job", 40.0);
    let (kept, _) = cluster_aware_retain(vec![strong, weak], 50.0, &HashSet::new(), &[]);
    assert_eq!(
        kept.len(),
        2,
        "a below-bar member of a passing cluster must be kept"
    );
}

#[test]
fn cluster_aware_retain_drops_a_failing_cluster() {
    // A lone scored job below the bar → its cluster fails → dropped.
    let weak = ranked("https://c.example.com/job", 40.0);
    let (kept, _) = cluster_aware_retain(vec![weak], 50.0, &HashSet::new(), &[]);
    assert!(kept.is_empty(), "a below-bar singleton cluster is dropped");
}

#[test]
fn cluster_aware_retain_keeps_fully_unscored_cluster() {
    let unscored = FoundJob {
        url: "https://d.example.com/job".into(),
        ..found(None)
    };
    let (kept, _) = cluster_aware_retain(vec![unscored], 50.0, &HashSet::new(), &[]);
    assert_eq!(
        kept.len(),
        1,
        "a fully-unscored cluster keeps the keep-unscored behavior"
    );
}

#[test]
fn mixed_cluster_with_below_bar_scored_representative_is_dropped_even_with_unscored_member() {
    // Same job on two boards → ONE cluster. One copy scores 40 (below the 50
    // bar); the other is unscored. Per ADR-029 §g the cluster representative
    // is its best-SCORED member (40 < 50), so the WHOLE cluster is dropped —
    // the unscored member does NOT rescue it. Keep-unscored only applies to a
    // cluster with NO scored member at all.
    let scored_below = ranked("https://a.example.com/job", 40.0);
    let unscored = FoundJob {
        url: "https://b.example.com/job".into(),
        ..found(None)
    };
    let (kept, _) = cluster_aware_retain(vec![scored_below, unscored], 50.0, &HashSet::new(), &[]);
    assert!(
        kept.is_empty(),
        "a below-bar scored representative drops the whole cluster, unscored member included"
    );
}

// ── staleness gap: retain must never freeze an already-known job's score ──
// (issue #1104's sibling gap)

#[test]
fn readmit_stale_known_jobs_restores_an_already_known_row_the_retain_filter_dropped() {
    // A job the store already knows about (from a prior run) is rescraped
    // this run, freshly scored at 40 — but a raised `minMatchScore` means
    // `cluster_aware_retain` drops its cluster from `found_jobs`. Without
    // re-admitting it, `merge_found_jobs` never sees its key in `incoming`
    // and the persisted row stays frozen at its OLD score (e.g. 90 from a
    // run before the bar was raised).
    let known = FoundJob {
        url: "https://known.example.com/job".into(),
        title: "Rust Engineer".into(),
        company: "Acme".into(),
        score: Some(90.0),
        ..found(Some(90.0))
    };
    let rescored_known = FoundJob {
        score: Some(40.0),
        ..known.clone()
    };
    let persisted = vec![known];
    let retained: Vec<FoundJob> = Vec::new(); // dropped by the (raised) threshold
    let out = readmit_stale_known_jobs(retained, std::slice::from_ref(&rescored_known), &persisted);
    assert_eq!(out.len(), 1, "the already-known job is re-admitted");
    assert_eq!(
        out[0].score,
        Some(40.0),
        "re-admitted with its FRESH score, not the stale persisted one"
    );
}

#[test]
fn readmit_stale_known_jobs_never_readmits_a_job_the_store_has_never_seen() {
    // A brand-new below-bar job the retain filter correctly dropped: the
    // store has never seen it, so it must NOT be smuggled in — a genuinely
    // new below-threshold job still stays out on first sighting.
    let new_job = ranked("https://new.example.com/job", 10.0);
    let persisted: Vec<FoundJob> = Vec::new();
    let retained: Vec<FoundJob> = Vec::new();
    let out = readmit_stale_known_jobs(retained, &[new_job], &persisted);
    assert!(
        out.is_empty(),
        "a never-before-seen job stays dropped, retain filter or not"
    );
}

#[test]
fn readmit_stale_known_jobs_does_not_duplicate_a_row_that_already_passed_retain() {
    // The common case: the job clears the (possibly unchanged) threshold and
    // is already present in `retained`. It must appear exactly once in the
    // output, not doubled.
    let passing = ranked("https://passing.example.com/job", 90.0);
    let persisted = vec![passing.clone()];
    let retained = vec![passing.clone()];
    let out = readmit_stale_known_jobs(retained, &[passing], &persisted);
    assert_eq!(
        out.len(),
        1,
        "an already-retained known row is not duplicated"
    );
}

// ── readmit must never downgrade a persisted Combined score to Keyword ────
// (issue #1104/#1105 review finding — HIGH)

#[test]
fn readmit_stale_known_jobs_never_downgrades_a_persisted_combined_score() {
    // Run 1 (not modelled directly): a job scored Keyword 65, passed retain,
    // phase 2 upgraded it to Combined 85 — a genuine top match, persisted.
    let persisted_job = FoundJob {
        url: "https://known.example.com/job".into(),
        title: "Rust Engineer".into(),
        company: "Acme".into(),
        score: Some(85.0),
        score_source: ScoreSource::Combined,
        score_provisional: false,
        ..found(Some(85.0))
    };
    // Run 2: the board returns a truncated JD, this run's fresh keyword
    // coverage drops to 45 (< minMatchScore 60), so `cluster_aware_retain`
    // drops the cluster — `readmit_stale_known_jobs` re-admits it, but its
    // fresh score is Keyword (phase 2 never reaches a readmitted row).
    let rescored_this_run = FoundJob {
        score: Some(45.0),
        score_source: ScoreSource::Keyword,
        score_provisional: false,
        ..persisted_job.clone()
    };
    let persisted = vec![persisted_job];
    let retained: Vec<FoundJob> = Vec::new(); // dropped by this run's retain
    let out = readmit_stale_known_jobs(retained, &[rescored_this_run], &persisted);

    assert_eq!(out.len(), 1, "the already-known job is still re-admitted");
    assert_eq!(
        out[0].score,
        Some(85.0),
        "a persisted Combined score must survive a readmit whose fresh score \
         is only Keyword — overwriting it would fail `qualifies`'s own \
         (higher) Combined cut on the OLD score while a Keyword 45 also \
         fails the (lower) Keyword cut, vanishing the row from best-matches \
         entirely"
    );
    assert_eq!(
        out[0].score_source,
        ScoreSource::Combined,
        "the score label must not silently relabel a semantic verdict as a \
         keyword one"
    );
    assert!(
        !out[0].score_provisional,
        "the persisted (non-provisional) verdict must survive alongside its score"
    );
}

#[test]
fn readmit_stale_known_jobs_still_lets_a_same_run_upgrade_through() {
    // The mirror case the guard above must NOT block: a job genuinely Keyword
    // this run that phase 2 upgrades to Combined in the SAME run. That job
    // passed retain (it's in `retained`), so it never enters this function's
    // readmit loop at all — `present_keys` already contains its key, so the
    // loop below is a no-op for it and the upgraded row passes through
    // `retained` untouched.
    let upgraded = FoundJob {
        url: "https://upgraded.example.com/job".into(),
        score: Some(91.0),
        score_source: ScoreSource::Combined,
        ..found(Some(91.0))
    };
    let persisted = vec![FoundJob {
        score: Some(62.0),
        score_source: ScoreSource::Keyword,
        ..upgraded.clone()
    }];
    let retained = vec![upgraded.clone()];
    // Also feed the SAME (now-upgraded) job through `scored_before_retain` —
    // the real caller always does, since it's the pre-filter snapshot — to
    // prove the dedup-by-key path (not the kernel guard) is what keeps this
    // case a no-op.
    let pre_filter_keyword_copy = FoundJob {
        score: Some(62.0),
        score_source: ScoreSource::Keyword,
        ..upgraded.clone()
    };
    let out = readmit_stale_known_jobs(retained, &[pre_filter_keyword_copy], &persisted);

    assert_eq!(out.len(), 1, "no duplicate row");
    assert_eq!(
        out[0].score,
        Some(91.0),
        "a same-run Combined upgrade must not be reverted to its pre-rerank \
         Keyword score"
    );
    assert_eq!(out[0].score_source, ScoreSource::Combined);
}

/// Locks the END-TO-END composition (readmit_stale_known_jobs →
/// `AutopilotStore::record_run` → `merge_found_jobs` → the persisted store),
/// not just `readmit_stale_known_jobs` in isolation. The isolated tests above
/// only assert on `readmit_stale_known_jobs`'s OWN return value — flipping
/// `merge_found_jobs`'s `if inc.score.is_some() { .. }` guard (e.g. to
/// `if row.score.is_none()`) would leave every one of them green while
/// silently breaking the feature end to end, because none of them ever
/// invoke `merge_found_jobs` or touch a real store.
#[test]
fn readmit_then_record_run_round_trips_the_refreshed_score_through_a_real_store() {
    let (temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");

    let first_run_job = FoundJob {
        url: "https://known.example.com/job".into(),
        title: "Rust Engineer".into(),
        company: "Acme".into(),
        score: Some(65.0),
        ..found(Some(65.0))
    };
    store.record_run(
        &ap.id,
        1,
        0,
        vec![first_run_job.clone()],
        Vec::new(),
        &HashSet::new(),
        &[],
    );
    assert_eq!(store.list()[0].found_jobs[0].score, Some(65.0), "seeded");

    // Second run: the raised threshold drops this job from `retained`, so the
    // command builds the readmit batch from the fresh (rescored) copy before
    // handing it to `record_run`, exactly like `autopilot_run` does.
    let rescored = FoundJob {
        score: Some(40.0),
        ..first_run_job.clone()
    };
    let persisted_before = store.list()[0].found_jobs.clone();
    let readmitted = readmit_stale_known_jobs(Vec::new(), &[rescored], &persisted_before);
    store.record_run(&ap.id, 0, 0, readmitted, Vec::new(), &HashSet::new(), &[]);

    // Re-read from a FRESH store instance backed by the same directory — this
    // proves the refreshed score round-tripped through disk, not just the
    // in-memory `self.cache` the writer already held.
    let reloaded = AutopilotStore::new(&temp.path().to_path_buf());
    let reloaded_list = reloaded.list();
    let persisted_job = reloaded_list[0]
        .found_jobs
        .iter()
        .find(|j| j.url == first_run_job.url)
        .expect("the readmitted row must persist across the store round-trip");
    assert_eq!(
        persisted_job.score,
        Some(40.0),
        "the readmitted job's fresh score must survive record_run + a fresh-store re-read"
    );
}
