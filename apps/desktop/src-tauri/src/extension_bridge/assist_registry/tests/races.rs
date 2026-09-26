//! Three related race/guard-integrity groups pinned together in the original
//! file and kept together here: the pre-registration cancel race (`begin`'s
//! `Option<u64>` + `register`'s `bool`), duplicate-`reqId` rejection, and
//! generation-scoped removal (security-review + CodeRabbit fix).

use super::super::AssistStreamRegistry;
use super::support::RecordingCanceller;

// ── Pre-registration cancel race (LOW fix) ──────────────────────────────────

#[test]
fn cancel_during_the_pending_window_prevents_the_later_register_call() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId"); // no job yet
    r.cancel(&canceller, "req-1"); // assist.cancel races the awaits

    assert!(
        canceller.cancelled.borrow().is_empty(),
        "no job exists yet — there is nothing to job_cancel"
    );
    assert!(
        !r.register("req-1", r#gen, "job-1"),
        "the compose call must never start a billable job for an early-cancelled reqId"
    );
    assert_eq!(
        r.take("req-1"),
        None,
        "the cancelled-early marker must never surface as a real job"
    );
}

#[test]
fn register_without_a_prior_cancel_succeeds_normally_after_begin() {
    let r = AssistStreamRegistry::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1"));
    assert_eq!(r.take("req-1"), Some("job-1".to_string()));
}

// ── Duplicate reqId rejection (MEDIUM fix): begin() on an already-active
// entry must never orphan the original job ─────────────────────────────────

#[test]
fn begin_on_a_fresh_req_id_succeeds() {
    let r = AssistStreamRegistry::default();
    assert!(r.begin("req-1").is_some());
}

#[test]
fn begin_on_an_already_pending_req_id_is_rejected() {
    let r = AssistStreamRegistry::default();
    r.begin("req-1"); // first request's pre-compose window is in flight

    assert!(
        r.begin("req-1").is_none(),
        "a second begin for the same still-Pending reqId must be rejected"
    );
}

#[test]
fn begin_on_an_already_running_req_id_is_rejected_and_the_original_stays_cancellable() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1")); // the original is now Running

    // A client reusing the SAME reqId while the original is still
    // running must be rejected — never silently overwrite the Running
    // entry with a fresh Pending, which would orphan job-1 (still
    // running server-side, but no longer reachable to cancel).
    assert!(r.begin("req-1").is_none());

    r.cancel(&canceller, "req-1");
    assert_eq!(
        canceller.cancelled.into_inner(),
        vec!["job-1".to_string()],
        "the original job must still be there and cancellable after the rejected begin"
    );
}

#[test]
fn begin_on_a_cancelled_early_req_id_is_rejected() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    r.begin("req-1");
    r.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet

    assert!(
        r.begin("req-1").is_none(),
        "a CancelledEarly marker is not settled — reuse must be rejected \
         until register (or cancel_all) consumes it"
    );
}

#[test]
fn begin_on_a_cancelled_early_req_id_is_rejected_until_register_consumes_it() {
    // Full spend/cancel-integrity guarantee: a run that reused req-1
    // before the CancelledEarly marker was consumed used to be able to
    // slip a fresh Pending in, which let the FIRST (already-cancelled)
    // run's later `register` call see that Pending instead of its own
    // marker and start a billable job anyway — the exact hole this fix
    // closes.
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let gen_a = r.begin("req-1").expect("a fresh reqId"); // run A opens
    r.cancel(&canceller, "req-1"); // -> CancelledEarly, run A has no job yet

    assert!(
        r.begin("req-1").is_none(),
        "a second run reusing req-1 must be rejected while the marker is un-consumed"
    );
    assert!(
        !r.register("req-1", gen_a, "job-a"),
        "register consumes the CancelledEarly marker and reports false — \
         run A never starts a billable job"
    );
    assert!(
        r.begin("req-1").is_some(),
        "once consumed, req-1 names no entry at all — reuse is allowed again"
    );
}

// ── Generation-scoped removal (security-review + CodeRabbit fix):
// unregister_gen must never clobber a reused reqId's successor entry ────────

#[test]
fn unregister_gen_never_clobbers_a_reused_req_ids_successor_entry() {
    // The exact clobber this generation token closes: A registers
    // Running, an `assist.cancel` removes A's entry (cancelling job-a),
    // then a client reuses the SAME reqId for a brand-new request B —
    // B's `begin`/`register` succeed with a STRICTLY HIGHER generation.
    // A's own end-of-request cleanup then arrives LATE (after B has
    // already registered) — keyed by reqId alone, the old unconditional
    // `unregister` would have clobbered B's fresh entry here.
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();

    let gen_a = r
        .begin("req-x")
        .expect("A's begin succeeds on a fresh reqId");
    assert!(r.register("req-x", gen_a, "job-a"));
    r.cancel(&canceller, "req-x"); // removes A's Running entry, cancels job-a

    let gen_b = r
        .begin("req-x")
        .expect("B may reuse req-x once A's entry is gone");
    assert!(
        gen_b > gen_a,
        "B's generation must be strictly higher than A's"
    );
    assert!(r.register("req-x", gen_b, "job-b"));

    // A's tail cleanup arrives LATE — after B has already begun and
    // registered — the exact race the generation token exists to close.
    r.unregister_gen("req-x", gen_a);

    assert!(
        r.contains("req-x"),
        "A's stale, lower-generation cleanup must never remove B's fresh entry"
    );

    // B's job is still fully reachable AND cancellable — never clobbered.
    r.cancel(&canceller, "req-x");
    assert_eq!(
        canceller.cancelled.into_inner(),
        vec!["job-a".to_string(), "job-b".to_string()],
        "B's job must still be cancellable after A's stale cleanup ran"
    );
}

#[test]
fn unregister_gen_removes_the_entry_when_the_generation_still_matches() {
    // The normal (non-reused-reqId) case: the caller's own gen still
    // names the SAME entry it was handed for, so unregister_gen must
    // actually remove it — this is the "one owner cleans up its own
    // request" path every non-race request takes.
    let r = AssistStreamRegistry::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1"));

    r.unregister_gen("req-1", r#gen);

    assert!(
        !r.contains("req-1"),
        "the matching generation must remove the entry"
    );
}
