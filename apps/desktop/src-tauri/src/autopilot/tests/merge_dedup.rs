//! Merging a run's postings into the found-jobs list: first-seen order, `is_new`, and the
//! canonical-URL dedup.

use super::support::*;
use crate::autopilot::merge::merge_found_jobs;

#[test]
fn merge_dedups_by_url_preserving_first_seen_and_flagging_new() {
    let existing = vec![found_job("https://a.com/1", 100)];
    let incoming = vec![
        found_job("https://a.com/1", 999), // re-surfaced — keep found_at=100
        found_job("https://a.com/2", 200), // genuinely new
    ];

    let merged = merge_found_jobs(&existing, incoming);

    assert_eq!(merged.len(), 2, "no duplicate row for the same url");
    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(a1.found_at, 100, "first-seen time preserved");
    assert!(!a1.is_new, "an existing job is not new");
    let a2 = merged.iter().find(|j| j.url == "https://a.com/2").unwrap();
    assert!(a2.is_new, "a never-seen url is flagged new");
}

#[test]
fn merge_is_idempotent_on_a_repeated_run() {
    let first = merge_found_jobs(&[], vec![found_job("u1", 1), found_job("u2", 2)]);
    assert!(first.iter().all(|j| j.is_new));

    // Re-running with the same postings yields the same set; only is_new clears.
    let second = merge_found_jobs(&first, vec![found_job("u1", 9), found_job("u2", 9)]);
    assert_eq!(second.len(), 2);
    assert!(second.iter().all(|j| !j.is_new));
}

#[test]
fn merge_keeps_prior_jobs_not_in_the_new_run() {
    let existing = vec![found_job("old", 1)];
    let merged = merge_found_jobs(&existing, vec![found_job("fresh", 2)]);
    assert_eq!(merged.len(), 2, "prior finds retained below the new one");
    assert!(merged.iter().any(|j| j.url == "old"));
}

#[test]
fn merge_puts_newly_found_jobs_on_top() {
    let merged = merge_found_jobs(&[found_job("old", 1)], vec![found_job("fresh", 2)]);
    assert_eq!(
        merged[0].url, "fresh",
        "newly found job is first (top of list)"
    );
    assert!(merged[0].is_new);
    assert_eq!(merged[1].url, "old", "prior finds fall below the new one");
}

// ── Cross-source / canonical-URL dedup ────────────────────────────────────────

#[test]
fn merge_dedups_same_job_across_two_url_variants() {
    // Same job captured two ways: tracking query params vs. a hash fragment.
    let a = found_job(
        "https://boards.example.com/jobs/42?utm_source=aggregator",
        1,
    );
    let b = found_job("https://boards.example.com/jobs/42#apply", 2);

    let merged = merge_found_jobs(&[], vec![a, b]);

    assert_eq!(
        merged.len(),
        1,
        "tracking-param and hash variants of one job URL must merge to a single row"
    );
    assert!(merged[0].is_new, "the single merged row is newly surfaced");
}

#[test]
fn merge_collapses_persisted_row_against_a_new_url_variant() {
    // Back-compat: a found-job persisted under one raw URL must merge with an
    // incoming batch item that is a *variant* of the same URL (tracking params
    // vs. hash fragment) — they share one canonical key, so the re-surfaced job
    // updates the existing row instead of adding a duplicate, and only the truly
    // new job is flagged. Guards the merge_key → canonical_job_key delegation:
    // the algorithm is unchanged, so old-scheme keys still recompute identically.
    let persisted = found_job("https://boards.example.com/jobs/42?utm_source=x", 100);
    let variant = found_job("https://boards.example.com/jobs/42#apply", 999);
    let fresh = found_job("https://boards.example.com/jobs/99", 200);

    let merged = merge_found_jobs(&[persisted], vec![variant, fresh]);

    assert_eq!(
        merged.len(),
        2,
        "the variant merges into the persisted row; only the fresh job is added"
    );
    let resurfaced = merged
        .iter()
        .find(|j| j.url.contains("/jobs/42"))
        .expect("the persisted job's row survives");
    assert_eq!(resurfaced.found_at, 100, "first-seen time preserved");
    assert!(!resurfaced.is_new, "a re-surfaced URL variant is not new");
    let brand_new = merged
        .iter()
        .find(|j| j.url.contains("/jobs/99"))
        .expect("the fresh job is present");
    assert!(brand_new.is_new, "the never-seen job is flagged new");
}

#[test]
fn merge_dedups_internal_batch_duplicate() {
    // The same job surfaced by two sources (aggregator + a named board) in ONE run.
    let from_aggregator = found_job("https://jobs.example.com/eng-42", 1);
    let from_board = found_job("https://jobs.example.com/eng-42", 2);

    let merged = merge_found_jobs(&[], vec![from_aggregator, from_board]);

    assert_eq!(
        merged.len(),
        1,
        "an internal batch duplicate must produce exactly one row"
    );
}

#[test]
fn merge_distinct_jobs_are_unaffected_by_dedup() {
    let merged = merge_found_jobs(
        &[],
        vec![
            found_job("https://a.example.com/1", 1),
            found_job("https://b.example.com/2", 2),
            found_job("https://c.example.com/3", 3),
        ],
    );

    assert_eq!(
        merged.len(),
        3,
        "three distinct jobs must remain three rows"
    );
    assert!(merged.iter().all(|j| j.is_new));
}

#[test]
fn merge_within_batch_dup_keeps_longer_description() {
    // First-seen carries a short description; the later duplicate a longer one.
    let mut short = found_job("https://jobs.example.com/eng-42?ref=a", 1);
    short.description = Some("short".into());
    let mut long = found_job("https://jobs.example.com/eng-42?ref=b", 2);
    long.description = Some("a much longer and more complete description".into());

    let merged = merge_found_jobs(&[], vec![short, long]);

    assert_eq!(merged.len(), 1, "the two variants merge to one row");
    assert_eq!(
        merged[0].description.as_deref(),
        Some("a much longer and more complete description"),
        "the longer description from the later duplicate must win"
    );
}

#[test]
fn merge_dedups_url_less_jobs_by_title_and_company() {
    // No URL → fall back to a normalized title+company key. `found_job` sets
    // title "Engineer", company "Acme".
    let a = found_job("", 1); // url ""            → fallback key
    let b = found_job("   ", 2); // whitespace url  → normalizes to "" → same key
    let mut c = found_job("", 3);
    c.company = "Globex".into(); // same title, different company → distinct key

    let merged = merge_found_jobs(&[], vec![a, b, c]);

    assert_eq!(
        merged.len(),
        2,
        "URL-less jobs dedupe by normalized title+company: the two Acme rows merge, Globex stays"
    );
}
