use super::*;
use crate::autopilot::tests::support::{autopilot_fixture, found_job_full};
use crate::autopilot::{notes_basis, FoundJob};

#[test]
fn stale_flag_follows_the_resume_the_run_would_use() {
    let stamped = FoundJob {
        description: Some("jd".into()),
        assistant_notes: Some("note".into()),
        notes_basis: Some(notes_basis("cv", Some("jd"))),
        ..found_job_full("https://acme.example/1", "CRM", "Acme", 0)
    };
    let mut ap = Autopilot {
        found_jobs: vec![
            stamped,
            found_job_full("https://acme.example/2", "CRM", "Acme", 0),
        ],
        ..autopilot_fixture()
    };
    mark_notes_stale(&mut ap, "cv");
    assert!(!ap.found_jobs[0].notes_stale);
    mark_notes_stale(&mut ap, "a different cv");
    assert!(ap.found_jobs[0].notes_stale, "résumé changed");
    assert!(!ap.found_jobs[1].notes_stale, "no note, nothing to label");
}
