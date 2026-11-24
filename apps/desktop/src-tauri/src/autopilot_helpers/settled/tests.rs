use super::*;
use crate::autopilot::tests::support::{autopilot_fixture, found_job_full};
use crate::autopilot::{notes_basis, IdentityReview};

const URL: &str = "https://acme.example/1";

fn noted(resume: &str, description: Option<&str>) -> FoundJob {
    FoundJob {
        description: description.map(str::to_string),
        assistant_notes: Some("note".into()),
        notes_basis: Some(notes_basis(resume, description)),
        ..found_job_full(URL, "CRM", "Acme", 0)
    }
}

fn incoming(description: Option<&str>) -> Vec<FoundJob> {
    vec![FoundJob {
        description: description.map(str::to_string),
        ..found_job_full(URL, "CRM", "Acme", 1)
    }]
}

fn ap(resume: &str, prior: FoundJob) -> Autopilot {
    Autopilot {
        resume_text: Some(resume.into()),
        found_jobs: vec![prior],
        ..autopilot_fixture()
    }
}

fn key() -> String {
    canonical_job_key(URL, "CRM", "Acme")
}

#[test]
fn unchanged_inputs_stay_settled() {
    let keys = settled_note_keys(&ap("cv", noted("cv", Some("jd"))), &incoming(Some("jd")));
    assert!(keys.contains(&key()));
}

#[test]
fn a_changed_resume_unsettles_the_job() {
    let keys = settled_note_keys(&ap("cv v2", noted("cv", Some("jd"))), &incoming(Some("jd")));
    assert!(!keys.contains(&key()));
}

#[test]
fn a_changed_description_unsettles_the_job() {
    let keys = settled_note_keys(
        &ap("cv", noted("cv", Some("jd"))),
        &incoming(Some("jd edited")),
    );
    assert!(!keys.contains(&key()));
}

#[test]
fn a_blank_resurfaced_description_compares_against_the_kept_one() {
    // The merge keeps the prior description over a blank one, so nothing changed.
    let keys = settled_note_keys(&ap("cv", noted("cv", Some("jd"))), &incoming(Some("  ")));
    assert!(keys.contains(&key()));
}

#[test]
fn legacy_unstamped_and_unnoted_jobs_stay_settled() {
    let legacy = FoundJob {
        notes_basis: None,
        ..noted("cv", Some("jd"))
    };
    assert!(settled_note_keys(&ap("other cv", legacy), &incoming(Some("x"))).contains(&key()));
    let bare = found_job_full(URL, "CRM", "Acme", 0);
    assert!(settled_note_keys(&ap("cv", bare), &incoming(Some("jd"))).contains(&key()));
}

#[test]
fn a_row_held_for_identity_review_stays_settled() {
    let held = FoundJob {
        identity_review: Some(IdentityReview {
            resolved_title: "Other".into(),
            resolved_company: "Other".into(),
            resolved_url: "https://other.example/9".into(),
            resolved_external_id: None,
            detected_at: 1,
        }),
        ..noted("cv", Some("jd"))
    };
    assert!(settled_note_keys(&ap("cv v2", held), &incoming(Some("jd"))).contains(&key()));
}
