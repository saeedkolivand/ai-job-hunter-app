//! `AssistStreamRegistry::cancel` / `cancel_all` — `JobCanceller`-generic.

use super::super::AssistStreamRegistry;
use super::support::RecordingCanceller;

#[test]
fn cancel_on_an_unknown_req_id_never_touches_the_canceller() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    r.cancel(&canceller, "never-registered");
    assert!(canceller.cancelled.borrow().is_empty());
}

#[test]
fn cancel_on_a_running_req_id_cancels_its_job_and_forgets_the_mapping() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    r.register("req-1", r#gen, "job-1");
    r.cancel(&canceller, "req-1");
    assert_eq!(canceller.cancelled.into_inner(), vec!["job-1".to_string()]);
    assert_eq!(r.take("req-1"), None, "cancel also forgets the mapping");
}

#[test]
fn cancel_all_cancels_every_running_stream_and_leaves_pending_alone_besides_marking_it() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let gen_1 = r.begin("req-1").expect("a fresh reqId");
    r.register("req-1", gen_1, "job-1");
    let gen_2 = r.begin("req-2").expect("a fresh reqId");
    r.register("req-2", gen_2, "job-2");
    let gen_3 = r.begin("req-3").expect("a fresh reqId"); // still pending — no job to cancel
    r.cancel_all(&canceller);

    let mut got = canceller.cancelled.into_inner();
    got.sort();
    assert_eq!(
        got,
        vec!["job-1".to_string(), "job-2".to_string()],
        "only RUNNING entries are ever job-cancelled"
    );
    // The pending entry is now cancelled-early — a still in-flight
    // pre-compose caller must never be allowed to register a job for it.
    assert!(!r.register("req-3", gen_3, "job-3"));
}

#[test]
fn cancel_all_on_an_empty_registry_is_a_no_op() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    r.cancel_all(&canceller);
    assert!(canceller.cancelled.into_inner().is_empty());
}

#[test]
fn cancel_all_preserves_an_already_cancelled_early_entry() {
    // HIGH regression: cancel-then-disconnect during the pre-compose
    // window. `begin` + `cancel` leaves `req-1` as `CancelledEarly`
    // BEFORE `cancel_all` ever runs; `cancel_all` must reinsert it
    // (not drop it on the floor), or the later `register` call for the
    // same `req_id` finds nothing, returns `true`, and starts a full
    // billable generation for a request the user already cancelled.
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    r.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet
    r.cancel_all(&canceller);

    assert!(
        canceller.cancelled.borrow().is_empty(),
        "no Running job existed at any point — nothing to job_cancel"
    );
    assert!(
        !r.register("req-1", r#gen, "job-1"),
        "the CancelledEarly guard must survive cancel_all's drain-and-reinsert"
    );
}
