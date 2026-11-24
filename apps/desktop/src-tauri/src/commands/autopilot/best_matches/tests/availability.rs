//! Verified availability (#1444): dead postings leave the actionable list, unverified ones stay
//! labelled, and a closure is never inherited by a different posting.

use super::*;
use crate::scraping::availability::{Availability, AvailabilityEvidence};

fn verified(mut ap: Autopilot, idx: usize, a: Availability) -> Autopilot {
    let j = &mut ap.found_jobs[idx];
    j.availability = Some(a);
    j.availability_checked_at = Some(42);
    j.availability_source = Some(AvailabilityEvidence::ClosedMarker);
    ap
}

#[test]
fn closed_and_removed_rows_are_not_actionable_recommendations() {
    for dead in [Availability::Closed, Availability::Removed] {
        let ap = verified(
            single(
                "c",
                "https://c.example.com/1",
                "Rust Engineer",
                "Acme",
                90.0,
            ),
            0,
            dead,
        );
        let out = best(&[ap]);
        assert!(out.matches.is_empty(), "{dead:?} must not be listed");
        assert_eq!(out.total, 0);
    }
}

#[test]
fn unknown_and_open_rows_stay_visible_with_their_check() {
    for state in [Availability::Unknown, Availability::Open] {
        let ap = verified(
            single(
                "u",
                "https://u.example.com/1",
                "Rust Engineer",
                "Acme",
                90.0,
            ),
            0,
            state,
        );
        let out = best(&[ap]);
        assert_eq!(out.matches.len(), 1);
        assert_eq!(out.matches[0].availability, Some(state));
        assert_eq!(out.matches[0].availability_checked_at, Some(42));
    }
    let unchecked = best(&[single(
        "n",
        "https://n.example.com/1",
        "Rust Engineer",
        "Acme",
        90.0,
    )]);
    assert_eq!(unchecked.matches[0].availability, None);
}

#[test]
fn a_repost_with_another_requisition_id_does_not_inherit_a_closure() {
    let ap = autopilot(
        "r",
        AutopilotStatus::Active,
        vec![
            job(
                "https://boards.example.com/jobs/111",
                "Rust Engineer",
                "Acme",
                Some(90.0),
                ScoreSource::Keyword,
            ),
            job(
                "https://boards.example.com/jobs/222",
                "Rust Engineer",
                "Acme",
                Some(90.0),
                ScoreSource::Keyword,
            ),
        ],
    );
    let out = best(&[verified(ap, 0, Availability::Closed)]);
    let urls: Vec<&str> = out.matches.iter().map(|m| m.url.as_str()).collect();
    assert_eq!(urls, vec!["https://boards.example.com/jobs/222"]);
}

#[test]
fn a_dead_verdict_on_any_copy_of_a_url_hides_every_copy() {
    // Same url in two autopilots: only the second copy carries the verdict.
    let url = "https://boards.example.com/jobs/111";
    let unchecked = single("a", url, "Rust Engineer", "Acme", 90.0);
    let dead = verified(
        single("b", url, "Rust Engineer", "Acme", 90.0),
        0,
        Availability::Removed,
    );
    for records in [[unchecked.clone(), dead.clone()], [dead, unchecked]] {
        assert!(best(&records).matches.is_empty());
    }
}
