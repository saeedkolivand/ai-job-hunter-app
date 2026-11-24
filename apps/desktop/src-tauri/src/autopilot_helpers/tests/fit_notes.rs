//! The notes loop's structured fit and its stale-note regeneration path (#1446, slices 2 and 3).

use parking_lot::Mutex;

use super::super::*;
use crate::autopilot::tests::support::{autopilot_fixture, found_job_full};
use crate::autopilot::{notes_basis, FitCategory};

const RESUME: &str = "CRM manager. Italian B1, English C2.";
const AD: &str = "Requirements:\nFluent Italian (C1)\nNice to have: Meta Ads";
const REPLY: &str = "You are fluent in Italian. Lead with your retention results.";

struct CountingEnv(Mutex<usize>);

#[async_trait]
impl NoteEnv for CountingEnv {
    async fn complete(&self, _s: &str, _u: &str, _t: f64) -> AppResult<String> {
        *self.0.lock() += 1;
        Ok(REPLY.to_string())
    }
    fn charge_daily(&self) -> AppResult<()> {
        Ok(())
    }
}

fn job(description: &str) -> FoundJob {
    FoundJob {
        is_new: true,
        description: Some(description.into()),
        ..found_job_full("https://acme.example/1", "CRM", "Acme", 0)
    }
}

async fn run(jobs: &mut [FoundJob], prior: &HashSet<String>, resume: &str) -> usize {
    let env = CountingEnv(Mutex::new(0));
    run_notes_loop(&env, resume, resume, jobs, prior, &CancellationToken::new()).await;
    env.0.into_inner()
}

#[tokio::test]
async fn structured_fit_is_filled_and_the_prose_carries_no_gap_text() {
    let mut jobs = vec![job(AD)];
    run(&mut jobs, &HashSet::new(), RESUME).await;
    let j = &jobs[0];
    assert_eq!(
        j.assistant_notes.as_deref(),
        Some("Lead with your retention results.")
    );
    assert_eq!(
        j.mandatory_gaps,
        vec!["Italian C1 required, résumé shows B1".to_string()]
    );
    assert_eq!(j.fit_category, Some(FitCategory::ExplicitGap));
    assert_eq!(
        j.notes_basis.as_deref(),
        Some(notes_basis(RESUME, Some(AD)).as_str())
    );
}

#[tokio::test]
async fn a_stale_prior_note_is_regenerated_not_skipped() {
    // Prior run: note written from the old résumé.
    let mut prior = job(AD);
    run(
        std::slice::from_mut(&mut prior),
        &HashSet::new(),
        "CRM manager. Italian A2.",
    )
    .await;
    prior.is_new = false;
    let ap = Autopilot {
        resume_text: Some(RESUME.into()),
        found_jobs: vec![prior.clone()],
        ..autopilot_fixture()
    };

    // The résumé changed → the job is NOT settled, so the loop pays for a fresh note.
    let mut incoming = vec![job(AD)];
    let settled = settled_note_keys(&ap, &incoming);
    assert_eq!(run(&mut incoming, &settled, RESUME).await, 1);
    assert_eq!(
        incoming[0].notes_basis.as_deref(),
        Some(notes_basis(RESUME, Some(AD)).as_str())
    );
    assert_ne!(incoming[0].notes_basis, prior.notes_basis);
}

#[tokio::test]
async fn an_unchanged_prior_note_is_not_regenerated() {
    let mut prior = job(AD);
    run(std::slice::from_mut(&mut prior), &HashSet::new(), RESUME).await;
    let ap = Autopilot {
        resume_text: Some(RESUME.into()),
        found_jobs: vec![prior],
        ..autopilot_fixture()
    };
    let mut incoming = vec![job(AD)];
    let settled = settled_note_keys(&ap, &incoming);
    assert_eq!(run(&mut incoming, &settled, RESUME).await, 0);
}
