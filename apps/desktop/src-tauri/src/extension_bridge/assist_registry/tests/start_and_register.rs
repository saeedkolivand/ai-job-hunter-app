//! `start_and_register` (HIGH fix: job_start-before-register TOCTOU —
//! starting the job before registering it means a cancel racing the gap
//! finds Pending, not a not-yet-existing Running job).

use std::cell::RefCell;

use super::super::{start_and_register, AssistStreamRegistry, JobCanceller, JobStarter};
use super::support::RecordingCanceller;

#[derive(Default)]
struct RecordingStarterCanceller {
    started: RefCell<Vec<String>>,
    cancelled: RefCell<Vec<String>>,
}

impl JobStarter for RecordingStarterCanceller {
    fn start_job(&self, job_id: &str) {
        self.started.borrow_mut().push(job_id.to_string());
    }
}

impl JobCanceller for RecordingStarterCanceller {
    fn cancel_job(&self, job_id: &str) {
        self.cancelled.borrow_mut().push(job_id.to_string());
    }
}

#[test]
fn start_and_register_starts_the_job_then_registers_it_on_the_happy_path() {
    let registry = AssistStreamRegistry::default();
    let recorder = RecordingStarterCanceller::default();
    let r#gen = registry.begin("req-1").expect("a fresh reqId");

    let job_id = start_and_register(&recorder, &registry, "req-1", r#gen)
        .expect("a fresh reqId with no prior cancel must register successfully");

    assert_eq!(
        recorder.started.into_inner(),
        vec![job_id.clone()],
        "job_start must have run, unconditionally, before register was ever consulted"
    );
    assert!(
        recorder.cancelled.borrow().is_empty(),
        "a successful register must never cancel the job it just started"
    );
    assert_eq!(
        registry.take("req-1"),
        Some(job_id),
        "register must have recorded the Running entry"
    );
}

#[test]
fn start_and_register_cancels_the_just_started_job_when_a_cancel_already_raced_ahead() {
    // The exact TOCTOU this reorder closes: an `assist.cancel` that
    // arrived during the pre-compose window (captured here as a
    // pre-seeded `CancelledEarly` marker — see `AssistStreamRegistry::
    // begin`/`cancel`) must never leave a job that's Running but neither
    // cancelled nor cancellable. `job_start` still runs — unconditionally,
    // BEFORE `register` is ever consulted, proving the new order — but
    // `register` then reports the race, and this function must cancel
    // the very job it just started rather than leaving it orphaned.
    let registry = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = registry.begin("req-1").expect("a fresh reqId");
    registry.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet

    let recorder = RecordingStarterCanceller::default();
    let result = start_and_register(&recorder, &registry, "req-1", r#gen);

    assert!(
        result.is_none(),
        "a raced-ahead cancel must make start_and_register report failure"
    );
    assert_eq!(
        recorder.started.borrow().len(),
        1,
        "job_start must still have run — it happens BEFORE register is ever consulted"
    );
    let started_id = recorder.started.borrow()[0].clone();
    assert_eq!(
        recorder.cancelled.into_inner(),
        vec![started_id],
        "the job just started must be job-cancelled immediately — no leaked Running job"
    );
    assert!(
        !registry.contains("req-1"),
        "the CancelledEarly marker must be consumed, not left behind"
    );
}
