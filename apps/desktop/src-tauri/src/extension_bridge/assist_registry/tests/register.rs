//! `AssistStreamRegistry`: register / take / unregister_gen / holds_running_gen / is_cancelled_early.

use super::super::AssistStreamRegistry;
use super::support::RecordingCanceller;

#[test]
fn register_then_take_returns_and_forgets_it() {
    let r = AssistStreamRegistry::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1"));
    assert_eq!(r.take("req-1"), Some("job-1".to_string()));
    assert_eq!(r.take("req-1"), None, "take also forgets the mapping");
}

#[test]
fn unregister_gen_on_an_unknown_req_id_is_a_no_op() {
    let r = AssistStreamRegistry::default();
    r.unregister_gen("never-registered", 0); // must not panic
    assert_eq!(r.take("never-registered"), None);
}

#[test]
fn register_overwrites_a_prior_mapping_for_the_same_req_id() {
    let r = AssistStreamRegistry::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1"));
    assert!(r.register("req-1", r#gen, "job-2"));
    assert_eq!(
        r.take("req-1"),
        Some("job-2".to_string()),
        "a re-registration under the same reqId must replace, not duplicate"
    );
}

/// The retry's registry contract: a request's SECOND attempt rebinds the
/// SAME entry onto its fresh job, keeping the generation `begin` minted, so
/// the request's ONE `unregister_gen` still owns — and so still frees — the
/// entry.
///
/// The pre-fix `register` fell to a `_` arm for an existing `Running` entry
/// and minted a FRESH generation, which made the request's own cleanup a
/// silent no-op: the entry then leaked until the socket dropped, and the
/// connection's `cancel_all` flipped an already-completed job to
/// `Cancelled`.
///
/// Mutation check (executed): drop `StreamEntry::Running(g, _)` from
/// `register`'s first match arm (restoring the fresh-generation `_` arm) and
/// both of the last two assertions fail.
#[test]
fn a_second_register_rebinds_the_same_entry_and_keeps_its_generation() {
    let r = AssistStreamRegistry::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1"));
    assert!(r.register("req-1", r#gen, "job-2"));

    assert!(
        r.holds_running_gen("req-1", r#gen),
        "the rebind must keep the generation the request was handed"
    );
    r.unregister_gen("req-1", r#gen);
    assert!(
        !r.contains("req-1"),
        "…so the request's own single cleanup still frees the entry"
    );
}

/// The other half of the same fix: a cancel between two attempts REMOVES
/// the entry, and the second attempt's bind must refuse rather than
/// resurrect it. The pre-fix `register` minted a fresh generation for a
/// missing entry and returned `true` — starting a full billable generation
/// for a request the client had already given up on.
///
/// Mutation check (executed): restore the old `_ => { mint a fresh gen }`
/// arm and the first assertion fails.
#[test]
fn register_refuses_a_req_id_whose_entry_a_cancel_already_removed() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1"));
    r.cancel(&canceller, "req-1"); // Running -> job cancelled AND removed

    assert!(
        !r.register("req-1", r#gen, "job-2"),
        "a second attempt must never resurrect an entry a cancel removed"
    );
    assert!(
        !r.contains("req-1"),
        "and the refusal must not leave anything behind either"
    );
}

/// Generation scoping on the bind, not just on the cleanup: a stale attempt
/// must never clobber a reused `reqId`'s successor entry.
#[test]
fn register_refuses_a_generation_that_is_not_its_own() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let gen_a = r.begin("req-1").expect("A's begin succeeds");
    assert!(r.register("req-1", gen_a, "job-a"));
    r.cancel(&canceller, "req-1");

    let gen_b = r.begin("req-1").expect("B may reuse req-1");
    assert!(r.register("req-1", gen_b, "job-b"));

    assert!(
        !r.register("req-1", gen_a, "job-a2"),
        "A's late bind must never take B's entry over"
    );
    assert!(r.holds_running_gen("req-1", gen_b));
}

/// `holds_running_gen` is the spend guard `compose_with_length_retry` checks
/// before paying for a retry, so every way the client can go away must read
/// as `false` here.
#[test]
fn holds_running_gen_is_true_only_for_this_requests_own_running_entry() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(
        !r.holds_running_gen("req-1", r#gen),
        "a Pending entry has no job yet — nothing is running"
    );

    assert!(r.register("req-1", r#gen, "job-1"));
    assert!(r.holds_running_gen("req-1", r#gen));
    assert!(
        !r.holds_running_gen("req-1", r#gen + 1),
        "another generation's entry is not this request's"
    );
    assert!(!r.holds_running_gen("req-2", r#gen), "nor another reqId's");

    r.cancel(&canceller, "req-1");
    assert!(
        !r.holds_running_gen("req-1", r#gen),
        "an assist.cancel takes the entry away"
    );
}

#[test]
fn holds_running_gen_is_false_once_the_connection_dropped() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1"));

    r.cancel_all(&canceller); // the read loop exited — the socket is gone

    assert!(
        !r.holds_running_gen("req-1", r#gen),
        "a dropped connection must not buy a retry"
    );
}

/// `is_cancelled_early` is the spend guard a caller doing pre-register billable grounding
/// (company-brief) checks before paying for it — a non-consuming peek, so `register` still
/// finds (and consumes) the SAME marker afterwards.
#[test]
fn is_cancelled_early_peeks_the_pending_cancel_without_consuming_it() {
    let r = AssistStreamRegistry::default();
    let canceller = RecordingCanceller::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(
        !r.is_cancelled_early("req-1", r#gen),
        "a fresh Pending entry was never cancelled"
    );

    r.cancel(&canceller, "req-1"); // still Pending (no job yet) -> CancelledEarly
    assert!(r.is_cancelled_early("req-1", r#gen));
    assert!(
        !r.is_cancelled_early("req-1", r#gen + 1),
        "another generation's marker is not this request's"
    );
    assert!(!r.is_cancelled_early("req-2", r#gen), "nor another reqId's");

    // The peek must not have consumed the marker: `register` still sees it and refuses.
    assert!(!r.register("req-1", r#gen, "job-1"));
}

#[test]
fn is_cancelled_early_is_false_for_a_running_or_never_cancelled_entry() {
    let r = AssistStreamRegistry::default();
    let r#gen = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", r#gen, "job-1"));
    assert!(
        !r.is_cancelled_early("req-1", r#gen),
        "a Running entry was never cancelled early"
    );
}

#[test]
fn unregister_gen_then_register_again_reflects_the_new_mapping() {
    let r = AssistStreamRegistry::default();
    let first = r.begin("req-1").expect("a fresh reqId");
    assert!(r.register("req-1", first, "job-1"));
    r.unregister_gen("req-1", first);

    // The entry is gone, so a reuse must `begin` again for its own
    // generation — a bind is only ever accepted against an entry that
    // already exists.
    let second = r.begin("req-1").expect("req-1 is free again");
    assert!(r.register("req-1", second, "job-2"));
    assert_eq!(r.take("req-1"), Some("job-2".to_string()));
}
