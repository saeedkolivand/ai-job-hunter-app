//! Budget + `StoppedReason` pins: the WIRE compatibility of the relocated enum,
//! the internal consistency of each shipped budget, and the security lock that
//! no budget is renderer-supplied.

use std::time::Duration;

use serde_json::json;

use super::{Budget, StoppedReason, DEFAULT_MAX_REPAIR_ATTEMPTS, DEFAULT_MAX_SECTIONS};

// ── Wire compatibility (the relocation must be invisible on the wire) ─────────

/// Every variant's wire string, in the order the enum declares them. The seven
/// leading entries are the EXACT strings `agent::controller::StoppedReason`
/// serialized to before this type moved into `pipeline::budget`; the renderer's
/// `STOPPED_SUFFIX` map (PrepApplicationPanel) keys on them, and an `agent.run`
/// job result carries one in its `stoppedReason` field. Changing one is a
/// breaking wire change, not a rename.
const WIRE: &[(StoppedReason, &str)] = &[
    (StoppedReason::Done, "done"),
    (StoppedReason::MaxSteps, "max_steps"),
    (StoppedReason::MaxTokens, "max_tokens"),
    (StoppedReason::Cancelled, "cancelled"),
    (StoppedReason::Truncated, "truncated"),
    (StoppedReason::Budgeted, "budgeted"),
    (StoppedReason::Timeout, "timeout"),
    // Added by the move — unreachable until Phase 3 wires them, but frozen now.
    (StoppedReason::RunTimeout, "run_timeout"),
    (StoppedReason::MaxToolCalls, "max_tool_calls"),
    (StoppedReason::MaxRepairs, "max_repairs"),
];

#[test]
fn stopped_reason_serializes_to_the_pinned_wire_strings() {
    for (variant, wire) in WIRE {
        assert_eq!(
            serde_json::to_value(variant).unwrap(),
            json!(wire),
            "{variant:?} must serialize to {wire:?} — the renderer keys on it"
        );
    }
}

/// WIRE is hand-maintained, and nothing above forces a variant into it — the
/// two loops iterate the TABLE, so a variant the table simply omits is never
/// tested. This closes that half: the `match` below is WILDCARD-FREE, so adding
/// a variant to `StoppedReason` fails to COMPILE here (E0004) until it is named
/// directly beneath the table it must also be added to, and the length pin fails
/// if a row is deleted or duplicated without the arm count moving with it.
///
/// The per-variant UNIQUENESS check is what closes the last gap between those
/// two: a row that is deleted and replaced by a DUPLICATE of another variant
/// keeps `WIRE.len()` at 10 and keeps every remaining row agreeing with the
/// match, so the omitted variant would silently stop being covered by every
/// loop in this file. One row per variant, exactly.
#[test]
fn the_wire_table_pins_every_variant() {
    for (variant, wire) in WIRE {
        assert_eq!(
            WIRE.iter().filter(|(other, _)| other == variant).count(),
            1,
            "{variant:?} has more than one WIRE row — a duplicate keeps the length \
             pin green while the variant it replaced drops out of every loop"
        );
        let expected = match variant {
            StoppedReason::Done => "done",
            StoppedReason::MaxSteps => "max_steps",
            StoppedReason::MaxTokens => "max_tokens",
            StoppedReason::Cancelled => "cancelled",
            StoppedReason::Truncated => "truncated",
            StoppedReason::Budgeted => "budgeted",
            StoppedReason::Timeout => "timeout",
            StoppedReason::RunTimeout => "run_timeout",
            StoppedReason::MaxToolCalls => "max_tool_calls",
            StoppedReason::MaxRepairs => "max_repairs",
        };
        assert_eq!(
            *wire, expected,
            "{variant:?}'s WIRE row disagrees with the exhaustive mapping"
        );
    }
    assert_eq!(
        WIRE.len(),
        10,
        "WIRE must carry exactly one row per StoppedReason variant — add the row \
         (and bump this count) alongside the match arm the compiler just demanded"
    );
}

#[test]
fn stopped_reason_round_trips_through_json() {
    for (variant, _) in WIRE {
        let encoded = serde_json::to_string(variant).unwrap();
        let decoded: StoppedReason = serde_json::from_str(&encoded).unwrap();
        assert_eq!(&decoded, variant);
    }
}

/// The re-export at the OLD path must be the SAME type, not a copy: a second
/// enum would serialize identically today and drift silently tomorrow.
#[test]
fn the_agent_re_export_is_this_exact_type() {
    let via_old_path: crate::agent::controller::StoppedReason = StoppedReason::MaxSteps;
    assert_eq!(via_old_path, StoppedReason::MaxSteps);
    assert_eq!(
        serde_json::to_value(via_old_path).unwrap(),
        json!("max_steps")
    );
}

/// A reason the shell has never emitted must not deserialize — an unknown value
/// arriving from a tampered bundle should fail loudly, not map to `Done`.
#[test]
fn an_unknown_wire_string_is_rejected() {
    assert!(serde_json::from_str::<StoppedReason>("\"finished_ok\"").is_err());
}

// ── Budget internal consistency ──────────────────────────────────────────────

/// Sanity for every budget we ship: no zero ceiling that would stop a run before
/// its first step, and no timeout ordering that makes a field unreachable.
#[test]
fn every_shipped_budget_is_internally_consistent() {
    for (label, b) in [
        ("AGENT_PREP", Budget::AGENT_PREP),
        ("RESUME_QUALITY", Budget::RESUME_QUALITY),
    ] {
        assert!(b.max_steps > 0, "{label}: a run must get at least one step");
        assert!(b.max_tokens > 0, "{label}: max_tokens must be positive");
        assert!(
            b.max_sections > 0,
            "{label}: a document must get at least one section"
        );
        assert!(
            !b.step_timeout.is_zero() && !b.run_timeout.is_zero(),
            "{label}: a zero timeout expires before the first await"
        );
        assert!(
            b.run_timeout >= b.step_timeout,
            "{label}: run_timeout below step_timeout makes step_timeout unreachable"
        );
    }
}

/// Literal pins for every `Duration` field of both shipped budgets.
///
/// The consistency test above checks only RELATIONS (non-zero, `run_timeout >=
/// step_timeout`), and a typo can satisfy every one of them: a
/// `confirm_timeout` of `from_secs(3)` instead of `from_secs(300)` is non-zero,
/// is below `run_timeout`, and is never compared to anything — it just
/// auto-denies a human-in-the-loop confirmation three seconds after asking,
/// which looks like the user declining. `confirm_timeout` is also the field
/// NOTHING else reads today (the pipeline suspends on nothing yet), so a literal
/// pin is its only guard. Each number's rationale is in the budget's doc.
#[test]
fn every_budget_timeout_is_pinned_to_its_documented_literal() {
    assert_eq!(
        Budget::AGENT_PREP.step_timeout,
        Duration::from_secs(360),
        "AGENT_PREP.step_timeout sits above the 300s OLLAMA_COMPLETION timeout on purpose"
    );
    assert_eq!(Budget::AGENT_PREP.run_timeout, Duration::from_secs(45 * 60));
    assert_eq!(
        Budget::AGENT_PREP.confirm_timeout,
        Duration::from_secs(300),
        "a shrunken confirm_timeout silently auto-denies confirmations"
    );

    assert_eq!(
        Budget::RESUME_QUALITY.step_timeout,
        Duration::from_secs(360),
        "RESUME_QUALITY.step_timeout deliberately matches AGENT_PREP's backstop"
    );
    // 75 min, and DERIVED rather than chosen: it is the effort-blind floor that
    // must equal `timeouts::quality_run_deadline(None)` — 4200 s of FLAT
    // per-call bounds (3 JSON stages × 2 round-trips, PLUS `max_repair_attempts`
    // × `MAX_SECTIONS_PER_ROUND` section rewrites, all at `OLLAMA_COMPLETION`)
    // + 300 s for the one streamed draft. The 45-minute version counted the
    // repair fan-out as one effort-scaled draft-equivalent per round, i.e. 600 s
    // instead of 2400 s, so the deadline sat ~1800 s below the calls it wraps.
    // See the budget's own doc and
    // `quality_run_deadline_agrees_with_the_budget_floor_at_the_bottom_tier`,
    // which is the guard that keeps the two from drifting apart again.
    assert_eq!(
        Budget::RESUME_QUALITY.run_timeout,
        Duration::from_secs(75 * 60)
    );
    assert_eq!(
        Budget::RESUME_QUALITY.confirm_timeout,
        Duration::from_secs(300),
        "a shrunken confirm_timeout silently auto-denies confirmations"
    );
}

// The budget-arithmetic relations (`max_steps` above the prep worst case,
// `max_tool_calls` below `max_steps`, `RESUME_QUALITY.max_steps` above the
// section count) are `const _: () = assert!(…)` items in `budget.rs` itself, NOT
// tests here: `#[cfg(test)]` code is not compiled by `cargo build --release`, so
// an assert placed in this file could never have failed a build.

/// The résumé pipeline runs no tools; zero is the deliberate bound, so a stage
/// that starts calling tools has to justify raising it rather than inheriting
/// the agent's allowance.
#[test]
fn resume_quality_allows_no_tool_calls_and_covers_every_section() {
    assert_eq!(Budget::RESUME_QUALITY.max_tool_calls, 0);
    assert_eq!(Budget::RESUME_QUALITY.max_sections, DEFAULT_MAX_SECTIONS);
    assert_eq!(
        Budget::RESUME_QUALITY.max_repair_attempts,
        DEFAULT_MAX_REPAIR_ATTEMPTS
    );
}

// ── Security lock: budgets are backend-owned, never renderer-supplied ─────────

/// Same lock shape as `commands::agent`'s routing lock (task #25): the wire
/// request struct has NO budget field, so a compromised renderer that appends
/// `maxSteps`/`maxTokens`/`maxToolCalls` cannot widen a paid run's ceiling —
/// serde drops the unknown keys because there is nowhere to bind them. The
/// anti-abuse limiter caps how OFTEN a run starts; only this struct's SHAPE caps
/// how much one run may spend. The gate is `gen:ipc:check` (Rust↔TS parity) plus
/// this assertion.
#[test]
fn the_run_request_carries_no_budget_field() {
    let req: crate::ipc_contracts::agent::AgentRunRequest = serde_json::from_value(json!({
        "resumeId": "res-1",
        "jobId": "job-9",
        // A compromised renderer's attempted budget escalation — ignored.
        "maxSteps": 100_000,
        "maxToolCalls": 100_000,
        "maxTokens": 100_000_000,
        "maxSections": 4_000,
        "maxRepairAttempts": 999,
        "stepTimeoutSecs": 86_400,
        "runTimeoutSecs": 86_400,
        "budget": { "maxSteps": 100_000 },
    }))
    .expect("deserializes from the identity-only wire shape, ignoring budget keys");
    assert_eq!(req.resume_id, "res-1");
    assert_eq!(req.job_id, "job-9");

    // Re-serializing the accepted request must not carry any budget key back —
    // proof the values were dropped, not stashed in a catch-all field.
    let echoed = serde_json::to_value(&req).unwrap();
    let keys: Vec<&str> = echoed
        .as_object()
        .expect("request serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    for forbidden in [
        "maxSteps",
        "maxToolCalls",
        "maxTokens",
        "maxSections",
        "maxRepairAttempts",
        "stepTimeoutSecs",
        "runTimeoutSecs",
        "budget",
    ] {
        assert!(
            !keys.contains(&forbidden),
            "AgentRunRequest must not carry a renderer-supplied budget key ({forbidden})"
        );
    }
}

/// The budgets themselves are compile-time constants, so there is no setter to
/// call from a command handler either. `Copy` (not `Clone`-into-a-cell) is the
/// mechanism: a caller gets a VALUE, and mutating it cannot affect the constant.
#[test]
fn a_budget_copy_cannot_mutate_the_shipped_constant() {
    let mut local = Budget::AGENT_PREP;
    local.max_tokens = usize::MAX;
    assert_eq!(local.max_tokens, usize::MAX, "the copy did change");
    assert_eq!(
        Budget::AGENT_PREP.max_tokens,
        120_000,
        "the shipped constant must be unaffected by a caller's copy"
    );
}
