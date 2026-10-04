//! The `JobPosting` -> `FoundJob` projection: provisional-score flags and the copied fields.

use super::super::keyword_rank::{build_found_job, AGGREGATOR_SNIPPET_SOURCE};
use super::support::*;

// ── snippet-score provisional flag (item 4) ────────────────────────────

#[test]
fn build_found_job_flags_aggregator_snippet_scores_as_provisional() {
    // An aggregator (Adzuna) posting is ranked over a truncated snippet, so
    // its score is provisional.
    let mut agg = posting("Rust Engineer", Some("We use Rust and Go"));
    agg.source = AGGREGATOR_SNIPPET_SOURCE.into();
    let job = build_found_job(&agg, "rust go", 0);
    assert!(job.score.is_some(), "a résumé + description yields a score");
    assert!(
        job.score_provisional,
        "an aggregator snippet score must be flagged provisional"
    );

    // A direct full-text board's score is authoritative — not provisional.
    let mut greenhouse = posting("Rust Engineer", Some("We use Rust and Go"));
    greenhouse.source = "greenhouse".into();
    let job = build_found_job(&greenhouse, "rust go", 0);
    assert!(job.score.is_some());
    assert!(
        !job.score_provisional,
        "a full-text board score must not be flagged provisional"
    );

    // No résumé → no score → nothing to qualify, even for an aggregator job.
    let mut agg_unscored = posting("Rust Engineer", Some("We use Rust"));
    agg_unscored.source = AGGREGATOR_SNIPPET_SOURCE.into();
    let job = build_found_job(&agg_unscored, "", 0);
    assert!(job.score.is_none());
    assert!(
        !job.score_provisional,
        "an unscored job is never provisional"
    );
}

// Issue #1105: a title-only blob (LinkedIn's free-tier `description:
// Some("")`) must be flagged provisional too, regardless of source — the
// title alone can round to full coverage with no JD text behind it.
#[test]
fn build_found_job_flags_title_only_blob_as_provisional_regardless_of_source() {
    // LinkedIn — NOT the aggregator source — with an empty description and a
    // title whose words fully match the résumé.
    let mut linkedin = posting("Rust Engineer", Some(""));
    linkedin.source = "linkedin".into();
    let job = build_found_job(&linkedin, "rust engineer", 0);
    assert_eq!(
        job.score,
        Some(100.0),
        "a title-only blob can round to full coverage"
    );
    assert!(
        job.score_provisional,
        "a title-only score must be flagged provisional even on a non-aggregator source"
    );

    // Same posting, but with real description text — no longer provisional
    // (assuming a non-aggregator source).
    let mut linkedin_full = posting("Rust Engineer", Some("We use Rust and Go"));
    linkedin_full.source = "linkedin".into();
    let job = build_found_job(&linkedin_full, "rust engineer", 0);
    assert!(job.score.is_some());
    assert!(
        !job.score_provisional,
        "real description text on a non-aggregator source is authoritative"
    );

    // Regression guard: the existing aggregator-snippet provisional case
    // (real description text, aggregator source) must still fire — the
    // broadened condition must not accidentally narrow the original one.
    let mut agg_with_text = posting("Rust Engineer", Some("We use Rust and Go"));
    agg_with_text.source = AGGREGATOR_SNIPPET_SOURCE.into();
    let job = build_found_job(&agg_with_text, "rust go", 0);
    assert!(job.score.is_some());
    assert!(
        job.score_provisional,
        "an aggregator snippet score with real description text is still provisional"
    );
}

// ── posted_at projection ────────────────────────────────────────────────

#[test]
fn build_found_job_copies_posted_at_from_the_posting() {
    // Every aggregator provider (Adzuna, JSearch, Jooble, Apify LinkedIn)
    // parses the posting's own publish date into `JobPosting.posted_at`
    // (epoch ms) — `build_found_job` must carry it straight through.
    let mut dated = posting("Rust Engineer", None);
    dated.posted_at = Some(1_700_000_000_000);
    let job = build_found_job(&dated, "", 0);
    assert_eq!(job.posted_at, Some(1_700_000_000_000));

    // Several full-text boards don't expose a publish date and leave the
    // posting's `posted_at` at `None` — that must flow through as `None` too,
    // not a silently-invented default.
    let dateless = posting("Rust Engineer", None);
    let job = build_found_job(&dateless, "", 0);
    assert_eq!(job.posted_at, None);
}

// ── board_remote projection (round-3 fix, H1) ─────────────────────────────

#[test]
fn build_found_job_copies_the_boards_remote_flag_from_extra() {
    // An all-remote board (WeWorkRemotely/RemoteOK/Remotive/Jobicy) sets
    // `extra["remote"] = true` unconditionally, often alongside a `None` or
    // marker-free `location` — `build_found_job` must carry that flag
    // through onto `FoundJob.board_remote` so `found-jobs`' `remote` filter
    // can trust it, not just `location` text.
    let mut remote = posting("Rust Engineer", None);
    remote
        .extra
        .insert("remote".to_string(), serde_json::json!(true));
    let job = build_found_job(&remote, "", 0);
    assert!(job.board_remote);

    let onsite = posting("Rust Engineer", None);
    let job = build_found_job(&onsite, "", 0);
    assert!(
        !job.board_remote,
        "absent extra[\"remote\"] must default false"
    );

    let mut wrong_type = posting("Rust Engineer", None);
    wrong_type
        .extra
        .insert("remote".to_string(), serde_json::json!("yes"));
    let job = build_found_job(&wrong_type, "", 0);
    assert!(
        !job.board_remote,
        "a non-bool extra[\"remote\"] must not be treated as true"
    );
}
