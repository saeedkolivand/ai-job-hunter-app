//! `charge_compose_budget` (no longer touches the registry — single
//! unregister owner is `unregister_after_request`) + `unregister_after_request`
//! itself (the SOLE unregister owner, called UNCONDITIONALLY — both Ok and
//! Err — exactly once, at `handle_answer_assist`'s single return point,
//! GENERATION-scoped).

use crate::extension_bridge::stream::AssistStreamRegistry;

use super::super::compose::charge_compose_budget;
use super::super::unregister_after_request;
use super::support::NoopCanceller;

// ── charge_compose_budget ────────────────────────────────────────────────

#[test]
fn charge_compose_budget_succeeds_and_leaves_the_registry_entry_in_place() {
    let limiter = crate::limits::Limiter::new();
    let registry = AssistStreamRegistry::default();
    registry.begin("req-1");

    let result = charge_compose_budget(&limiter, "openai");

    assert!(result.is_ok());
    assert!(
        registry.contains("req-1"),
        "a successful charge must leave the Pending entry for compose_draft_stream to register"
    );
}

#[test]
fn charge_compose_budget_leaves_the_pending_entry_in_place_on_a_rejected_charge_too() {
    // CodeRabbit consolidation: `charge_compose_budget` used to `unregister`
    // on a rejected charge itself — now it NEVER touches the registry at
    // all (single-owner fix), so a rejected charge must leave the entry
    // exactly as `charge_compose_budget_succeeds_and_leaves_the_registry_
    // entry_in_place` does; `unregister_after_request` (below) is the
    // ONLY thing that ever cleans it up, at `handle_answer_assist`'s
    // single return point.
    let limiter = crate::limits::Limiter::new();
    // Exhaust the SAME per-provider daily ceiling this call charges against.
    for _ in 0..crate::limits::PROVIDER_DAILY_MAX {
        limiter
            .charge_provider_daily("openai", crate::limits::PROVIDER_DAILY_MAX)
            .expect("charge within the daily ceiling");
    }
    let registry = AssistStreamRegistry::default();
    registry.begin("req-1");

    let result = charge_compose_budget(&limiter, "openai");

    assert!(result.is_err());
    assert!(
        registry.contains("req-1"),
        "charge_compose_budget must never unregister — that would reintroduce the \
             multi-site clobber this consolidation closes"
    );
}

// ── unregister_after_request ─────────────────────────────────────────────

#[test]
fn unregister_after_request_removes_a_pending_entry_left_by_an_early_gate_failure() {
    // Mirrors EVERY one of `resolve_answer_assist`'s early gates (ai-assist
    // off, empty question, no provider/résumé, limiter rejection, a
    // rejected daily-budget charge) and `handle_answer_assist`'s own
    // store-unavailable branch: `begin` already ran (via
    // `spawn_answer_assist`'s synchronous `begin_or_reject_duplicate`,
    // simulated here directly), then the call fails before ever reaching
    // `compose_draft_stream` — nothing else would ever clean up this entry.
    let registry = AssistStreamRegistry::default();
    let r#gen = registry.begin("req-1").expect("a fresh reqId");

    unregister_after_request(&registry, "req-1", r#gen);

    assert!(
        !registry.contains("req-1"),
        "an early-gate failure must unregister the Pending entry, not leak it for the \
             rest of this connection's lifetime"
    );
    assert!(
        registry.begin("req-1").is_some(),
        "a client retrying the SAME reqId after a failed attempt must not be \
             wrongly rejected as \"already in progress\" forever after"
    );
}

#[test]
fn unregister_after_request_also_removes_a_running_entry_on_a_successful_outcome() {
    // The single-owner fix's key behavior change: unlike the old
    // Err-only `unregister_on_err`, this runs on EVERY outcome — a
    // successful compose (which already `register`ed a Running job via
    // `compose_draft_stream`, which no longer unregisters itself) must
    // still be cleaned up here, or a successful reqId would leak forever.
    let registry = AssistStreamRegistry::default();
    let r#gen = registry.begin("req-1").expect("a fresh reqId");
    assert!(registry.register("req-1", r#gen, "job-1")); // the Pending -> Running move

    unregister_after_request(&registry, "req-1", r#gen);

    assert!(
        !registry.contains("req-1"),
        "a successful outcome must ALSO be unregistered — this is now the only \
             cleanup site for req-1, on every outcome"
    );
}

#[test]
fn unregister_after_request_is_a_no_op_when_already_unregistered() {
    // Double-unregister safety: an `assist.cancel` may already have
    // consumed the entry (a Running job cancelled + removed, or a
    // Pending -> CancelledEarly -> consumed by a later register) by the
    // time `handle_answer_assist` reaches this call — must never panic.
    let registry = AssistStreamRegistry::default();
    unregister_after_request(&registry, "never-registered", 0); // must not panic
    assert!(!registry.contains("never-registered"));
}

#[test]
fn unregister_after_request_then_a_fresh_begin_for_the_same_req_id_succeeds() {
    // The retry-after-cleanup case: once a request completes (either
    // outcome) and this runs, the reqId is fully free again — a client
    // reusing it for a brand-new request must succeed, and there must be
    // no SECOND unregister anywhere else that could reach in and remove
    // that NEW entry out from under it (the exact clobber the single-owner
    // + generation-scoping fixes close together).
    let registry = AssistStreamRegistry::default();
    let r#gen = registry.begin("req-1").expect("a fresh reqId");
    unregister_after_request(&registry, "req-1", r#gen);

    assert!(
        registry.begin("req-1").is_some(),
        "req-1 must be fully free once its one owner cleaned it up"
    );
    assert!(
        registry.contains("req-1"),
        "the fresh begin's Pending entry must still be there — nothing else \
             may reach in and remove it"
    );
}

#[test]
fn unregister_after_request_never_clobbers_a_reused_req_ids_successor_entry() {
    // The security-review finding on top of the single-owner fix: A
    // registers Running, an `assist.cancel` removes A's entry (job
    // cancelled) WHILE A's own request is still resolving, a client
    // reuses the SAME reqId for a brand-new request B which begins +
    // registers successfully — and only THEN does A reach
    // `unregister_after_request`. Generation scoping must make A's call a
    // no-op against B's fresh, higher-generation entry.
    let registry = AssistStreamRegistry::default();
    let canceller = NoopCanceller;
    let gen_a = registry.begin("req-1").expect("A's begin succeeds");
    assert!(registry.register("req-1", gen_a, "job-a"));
    registry.cancel(&canceller, "req-1"); // removes A's entry, cancels job-a

    let gen_b = registry.begin("req-1").expect("B may reuse req-1");
    assert!(registry.register("req-1", gen_b, "job-b"));

    // A's tail cleanup arrives LATE — after B has already registered.
    unregister_after_request(&registry, "req-1", gen_a);

    assert!(
        registry.contains("req-1"),
        "A's stale, lower-generation cleanup must never remove B's fresh entry"
    );
}
