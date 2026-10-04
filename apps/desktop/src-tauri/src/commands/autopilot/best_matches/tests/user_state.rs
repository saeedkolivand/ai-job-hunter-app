//! What the user has already acted on: dismissed clusters never list, and a row reads as applied
//! through any of its members.

use super::*;

#[test]
fn dismissed_url_is_dropped() {
    let ap = single(
        "d",
        "https://d.example.com/job",
        "Platform Engineer",
        "Dropco",
        90.0,
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
    let ap = single("blank", "", "", "", 90.0);
    let dismissed: HashSet<String> = [degenerate_key].into_iter().collect();
    let out = compute_best_matches(&[ap], &no_tombstones(), &[], &dismissed);
    assert_eq!(
        out.matches.len(),
        1,
        "a degenerate dismissed key must not veto a job whose own identity is equally degenerate"
    );
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
    let out = best(&[ap]);
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
