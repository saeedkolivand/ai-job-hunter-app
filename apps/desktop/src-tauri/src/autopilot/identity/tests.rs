use super::*;
use crate::autopilot::tests::support::*;

fn resolved(title: &str, company: &str, url: &str) -> ResolvedIdentity {
    ResolvedIdentity {
        title: title.into(),
        company: company.into(),
        url: url.into(),
        external_id: None,
    }
}

#[test]
fn drift_sets_a_review_and_only_dismissal_releases_it() {
    let mut job = found_job_full(
        "https://www.adzuna.de/land/ad/1",
        "Rust Engineer",
        "Acme",
        1,
    );
    let other = resolved("Cook", "Diner", "https://boards.greenhouse.io/diner/jobs/1");
    assert!(!job.check_resolved(&other));
    assert_eq!(job.identity_review.as_ref().unwrap().resolved_title, "Cook");

    let same = resolved(
        "Rust Engineer",
        "Acme GmbH",
        "https://boards.greenhouse.io/acme/jobs/9",
    );
    // A caller-supplied "same" identity must neither release the hold nor be written.
    assert!(!job.check_resolved(&same));
    assert!(job.identity_review.is_some());

    assert!(job.dismiss_identity_review());
    assert!(job.check_resolved(&same));
    assert!(job.identity_review.is_none());
}

#[test]
fn a_dismissed_drift_does_not_reflag_but_a_different_one_does() {
    let mut job = found_job_full(
        "https://www.adzuna.de/land/ad/1",
        "Rust Engineer",
        "Acme",
        1,
    );
    let first = resolved("Cook", "Diner", "https://boards.greenhouse.io/diner/jobs/1");
    job.check_resolved(&first);
    assert!(job.dismiss_identity_review());
    assert!(job.identity_review.is_none());
    assert!(!job.dismiss_identity_review(), "nothing left to dismiss");

    assert!(!job.check_resolved(&first));
    assert!(job.identity_review.is_none(), "same drift stays dismissed");

    let second = resolved("Pilot", "Air", "https://boards.greenhouse.io/air/jobs/2");
    assert!(!job.check_resolved(&second));
    assert!(job.identity_review.is_some(), "a different drift re-flags");
}

#[test]
fn legacy_records_without_the_fields_load_and_round_trip() {
    let mut job = found_job_full("https://x.example.com/1", "T", "C", 1);
    let value = serde_json::to_value(&job).unwrap();
    assert!(value.get("identityReview").is_none());
    assert!(value.get("identityDismissed").is_none());
    let legacy: crate::autopilot::FoundJob = serde_json::from_value(value).unwrap();
    assert!(legacy.identity_review.is_none());

    job.check_resolved(&resolved("Cook", "Diner", "https://y.example.com/2"));
    let back: crate::autopilot::FoundJob =
        serde_json::from_value(serde_json::to_value(&job).unwrap()).unwrap();
    assert_eq!(back.identity_review, job.identity_review);
}
