//! Unit tests for `timeouts.rs`: `stream_deadline`/`ollama_completion_deadline`
//! effort scaling, `research_deadline` + the interactive search-box
//! rerank/dense-arm bounds, and `quality_run_deadline`.

use super::*;

#[test]
fn stream_deadline_matches_the_baseline_for_no_or_low_effort() {
    assert_eq!(stream_deadline(None), STREAM);
    assert_eq!(stream_deadline(Some("")), STREAM);
    assert_eq!(stream_deadline(Some("minimal")), STREAM);
    assert_eq!(stream_deadline(Some("low")), STREAM);
}

#[test]
fn stream_deadline_scales_up_for_higher_effort() {
    assert_eq!(stream_deadline(Some("medium")), Duration::from_secs(450));
    assert_eq!(stream_deadline(Some("high")), Duration::from_secs(600));
    // `xhigh` then `max` — vendors' ascending order, see `effort_multiplier`.
    assert_eq!(stream_deadline(Some("xhigh")), Duration::from_secs(750));
    assert_eq!(stream_deadline(Some("max")), Duration::from_secs(900));
}

/// A caller must be able to trust the ordering, not just the individual
/// values — this is what "scales with effort" actually promises.
///
/// The array below must stay in the VENDORS' ascending tier order
/// (`… high < xhigh < max`), not in the order the match arms happen to be
/// written. A previous version listed `max` before `xhigh`, which made this
/// test pass against a table that gave the top tier the shortest deadline.
#[test]
fn stream_deadline_is_monotonically_nondecreasing_by_effort_tier() {
    let tiers = [
        None,
        Some("minimal"),
        Some("low"),
        Some("medium"),
        Some("high"),
        Some("xhigh"),
        Some("max"),
    ];
    let mut prev = Duration::from_secs(0);
    for effort in tiers {
        let d = stream_deadline(effort);
        assert!(
            d >= prev,
            "stream_deadline({effort:?}) = {d:?} must be >= the previous tier's {prev:?}"
        );
        prev = d;
    }
}

/// An effort string outside the known vocabulary must fall back to the
/// baseline — never explode a typo/future-provider string into an
/// unbounded multiplier.
#[test]
fn stream_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
    assert_eq!(stream_deadline(Some("ultra-mega-think")), STREAM);
}

// ── ollama_completion_deadline ──────────────────────────────────────────
//
// Same contract as `stream_deadline` — this is the fix for the incident
// where a per-call non-streaming deadline stayed flat regardless of
// effort, so a local model at a HIGHER effort got no more time on
// `analyze_job`/`strategy` than the baseline did.

#[test]
fn ollama_completion_deadline_matches_the_baseline_for_no_or_low_effort() {
    assert_eq!(ollama_completion_deadline(None), OLLAMA_COMPLETION_BASELINE);
    assert_eq!(
        ollama_completion_deadline(Some("")),
        OLLAMA_COMPLETION_BASELINE
    );
    assert_eq!(
        ollama_completion_deadline(Some("minimal")),
        OLLAMA_COMPLETION_BASELINE
    );
    assert_eq!(
        ollama_completion_deadline(Some("low")),
        OLLAMA_COMPLETION_BASELINE
    );
}

#[test]
fn ollama_completion_deadline_scales_up_for_higher_effort() {
    assert_eq!(
        ollama_completion_deadline(Some("medium")),
        Duration::from_secs(450)
    );
    assert_eq!(
        ollama_completion_deadline(Some("high")),
        Duration::from_secs(600)
    );
    assert_eq!(
        ollama_completion_deadline(Some("xhigh")),
        Duration::from_secs(750)
    );
    assert_eq!(
        ollama_completion_deadline(Some("max")),
        Duration::from_secs(900)
    );
}

/// Scales `OLLAMA_COMPLETION_BASELINE` — NOT [`STREAM`] — by the shared
/// multiplier table. Asserted against `OLLAMA_COMPLETION_BASELINE`, never
/// [`stream_deadline`]/[`STREAM`] directly: the two baselines are
/// documented as separate constants free to drift independently (they
/// only share a value today because they happen to start equal).
/// Comparing directly against `stream_deadline` would go red the moment
/// either baseline changed on its own, blaming the multiplier table for a
/// baseline edit it had nothing to do with. The multiplier lookup is
/// reproduced inline (not via [`effort_multiplier`]) so this stays a
/// check on the shared table, not a tautology against the function under
/// test. Mutation check: give either baseline its own multiplier table
/// and this fails.
#[test]
fn ollama_completion_deadline_scales_its_own_baseline_by_the_shared_multiplier_table() {
    for effort in [
        None,
        Some("minimal"),
        Some("low"),
        Some("medium"),
        Some("high"),
        Some("xhigh"),
        Some("max"),
    ] {
        let multiplier = match effort {
            Some(e) => EFFORT_TIMEOUT_MULTIPLIER
                .iter()
                .find(|(tier, _)| *tier == e)
                .map_or(1.0, |(_, mult)| *mult),
            None => 1.0,
        };
        assert_eq!(
            ollama_completion_deadline(effort),
            Duration::from_secs_f64(OLLAMA_COMPLETION_BASELINE.as_secs_f64() * multiplier),
            "ollama_completion_deadline({effort:?}) must scale OLLAMA_COMPLETION_BASELINE by the shared multiplier table"
        );
    }
}

#[test]
fn ollama_completion_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
    assert_eq!(
        ollama_completion_deadline(Some("ultra-mega-think")),
        OLLAMA_COMPLETION_BASELINE
    );
}

// ── research_deadline ───────────────────────────────────────────────────
//
// Same contract as `stream_deadline`, and for the same reason: the thing it
// bounds ends in a model call. A flat 25s here meant every research call in
// a reported reasoning-model session timed out, and each cover letter was
// written with no company knowledge and no visible failure.

/// The rerank const assertion bounds `OLLAMA_EMBED` from ABOVE only, so a
/// revert to the old 15s would satisfy it and silently restore the budget the
/// field measurement showed was too small. Pin the value here, and pin the
/// doc's own claim ("the same bound as cloud EMBED, not a tighter one") next
/// to it so the two cannot drift apart silently.
#[test]
fn ollama_embed_is_pinned_at_thirty_seconds_and_matches_the_cloud_bound() {
    assert_eq!(
        OLLAMA_EMBED,
        Duration::from_secs(30),
        "15s could not survive a local chat model holding the GPU; 30s is also the              ceiling that keeps RERANK_DEGRADE_BREAKER able to fire"
    );
    assert_eq!(
        OLLAMA_EMBED, EMBED,
        "OLLAMA_EMBED's doc claims local is not bounded tighter than cloud — keep              these equal or reword it"
    );
}

#[test]
fn research_deadline_exceeds_the_inner_search_bounds_it_wraps() {
    // It wraps a web search AND a synthesis completion. If the outer bound
    // isn't clear of the inner ones, it fires first and the actionable inner
    // error never surfaces. The old flat 25s was EQUAL to `WEB_SEARCH`.
    assert!(
        research_deadline(None) > WEB_SEARCH,
        "the outer research bound must clear the cloud web-search bound"
    );
    assert!(research_deadline(None) > OLLAMA_WEB_SEARCH);
}

#[test]
fn hybrid_search_rerank_cloud_sits_between_ollama_embed_and_completion() {
    // If a future OLLAMA_EMBED bump ever closes this gap, the cloud
    // rerank tier is no longer "generous for cloud, short enough not to
    // hang a search box" — it has drifted into embed-timeout territory.
    assert!(
        HYBRID_SEARCH_RERANK_CLOUD > OLLAMA_EMBED,
        "the cloud rerank bound must clear the local-embed bound"
    );
    assert!(
        HYBRID_SEARCH_RERANK_CLOUD < COMPLETION,
        "the cloud rerank bound must stay well under a full cloud completion's own bound"
    );
}

#[test]
fn hybrid_search_rerank_local_clears_its_own_derived_token_math_and_the_completion_baseline() {
    // The floor computed from the SAME constants (and the SAME ~4
    // chars/token, ~20 tok/s CPU-only estimates) the doc's derivation
    // states — never a re-typed literal: raising RERANK_TOP_K or
    // RERANK_ITEM_CHAR_BUDGET must move this floor with them, or a
    // widened prompt could silently make the documented derivation false
    // while this test stayed green.
    let prompt_chars = crate::retrieval::rerank::RERANK_TOP_K
        * crate::commands::hybrid_search::RERANK_ITEM_CHAR_BUDGET;
    let derived_floor = Duration::from_secs((prompt_chars / 4 / 20) as u64);
    assert!(
        HYBRID_SEARCH_RERANK_LOCAL > derived_floor,
        "the local rerank bound must clear its own documented derivation (RERANK_TOP_K * RERANK_ITEM_CHAR_BUDGET / 4 chars-per-token / 20 tok/s = {derived_floor:?})"
    );
    // Still an INTERACTIVE bound, not a generation one: strictly under
    // the flat baseline a full (uncapped-effort) local completion gets.
    assert!(
        HYBRID_SEARCH_RERANK_LOCAL < OLLAMA_COMPLETION_BASELINE,
        "the local rerank bound must stay under the full-generation local baseline"
    );
    // Local hardware is the materially slower regime the split exists
    // for — the local tier must be more generous than the cloud one, or
    // the split bought nothing.
    assert!(HYBRID_SEARCH_RERANK_LOCAL > HYBRID_SEARCH_RERANK_CLOUD);
}

#[test]
fn hybrid_search_rerank_deadline_selects_by_provider_class() {
    assert_eq!(
        hybrid_search_rerank_deadline(true),
        HYBRID_SEARCH_RERANK_LOCAL
    );
    assert_eq!(
        hybrid_search_rerank_deadline(false),
        HYBRID_SEARCH_RERANK_CLOUD
    );
}

#[test]
fn dense_arm_timeout_clears_three_worst_case_embed_round_trips() {
    // The floor this bound's doc derives from: three sequential
    // worst-case embeds ([`OLLAMA_EMBED`]/[`EMBED`], identical for both
    // provider classes — see that constant's own doc). Someone loosening
    // `OLLAMA_EMBED` without revisiting this bound must fail a test, not
    // silently shrink the number of round-trips it actually covers.
    assert!(
        DENSE_ARM_TIMEOUT > OLLAMA_EMBED * 3,
        "the dense-arm bound must clear three worst-case embed round-trips with real margin"
    );
    // Not a generation-class bound: strictly under the shortest
    // completion baseline, or this "embed budget" would out-live an
    // entire cloud completion.
    assert!(DENSE_ARM_TIMEOUT < COMPLETION);
}

#[test]
fn research_deadline_is_monotonically_nondecreasing_by_effort_tier() {
    // Vendors' ascending order — `max` is the TOP tier, above `xhigh`.
    let tiers = [
        None,
        Some("minimal"),
        Some("low"),
        Some("medium"),
        Some("high"),
        Some("xhigh"),
        Some("max"),
    ];
    let mut prev = Duration::from_secs(0);
    for effort in tiers {
        let d = research_deadline(effort);
        assert!(
            d >= prev,
            "research_deadline({effort:?}) = {d:?} must be >= the previous tier's {prev:?}"
        );
        prev = d;
    }
    // Not vacuously true: the top tier must actually exceed the baseline.
    assert!(research_deadline(Some("max")) > research_deadline(None));
}

#[test]
fn research_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
    assert_eq!(
        research_deadline(Some("ultra-mega-think")),
        RESEARCH_BASELINE
    );
    assert_eq!(research_deadline(None), RESEARCH_BASELINE);
}

// ── quality_run_deadline ────────────────────────────────────────────────

/// The cross-language lock. `packages/shared/src/ai-timeouts.test.ts` pins
/// the identical seven values against `qualityRunDeadlineSecs`; the two
/// constants are generated, but the ARITHMETIC is spelled out on both
/// sides, so only a matched pair of pinned tables catches a drift in the
/// formula itself. Mutation check: change either side's `fixed + …` to
/// `baseline × …` and one of the two tables fails.
#[test]
fn quality_run_deadline_pins_the_derived_table() {
    for (effort, secs) in [
        (None, 4_800),
        (Some("minimal"), 4_800),
        (Some("low"), 4_800),
        (Some("medium"), 5_700),
        (Some("high"), 6_600),
        (Some("xhigh"), 7_500),
        (Some("max"), 8_400),
    ] {
        assert_eq!(
            quality_run_deadline(effort),
            Duration::from_secs(secs),
            "quality_run_deadline({effort:?})"
        );
    }
}

/// The outer bound must EQUAL the inner bounds it wraps, at EVERY tier —
/// deliberately an exact match here, NOT the "clear it with headroom" rule
/// `research_deadline_exceeds_the_inner_search_bounds_it_wraps` states,
/// because the two deadlines enforce themselves in structurally different
/// ways. `research_deadline` wraps a LIVE future in
/// `tokio::time::timeout` (`cover_letter::research::mod::enrich_with`), so
/// an outer bound that does not clear the inner one really can cancel a
/// call mid-flight and race away its own error. [`quality_run_deadline`]
/// instead backs `pipeline::resume::RunDeadline`/`guard_deadline`, which is
/// only ever POLLED — `deadline.passed()`, checked between calls (stage
/// boundaries, a JSON stage's guard before its one re-ask, the repair
/// loop's per-section check, `humanize_one`'s per-document check) and
/// never raced against an in-flight completion (the backend observes its
/// deadline BETWEEN calls, never mid-call, because cancelling one would
/// throw away work the run already paid for — see
/// `QUALITY_RUN_CLIENT_MARGIN_SECS`'s own doc in
/// `packages/shared/src/ai-timeouts.ts`). A call already dispatched always
/// finishes or times out on its OWN bound; the run deadline can only
/// refuse the NEXT call, so there is no live race for a margin to protect.
///
/// **Why exact equality still lets an in-flight call's own timeout win.**
/// Before dispatching call *k*, elapsed time is the sum of calls
/// `1..k-1`'s ACTUAL durations, each strictly under its own bound (a call
/// that hit its own bound already returned — see below), so elapsed is
/// always strictly less than the sum of bounds `1..k-1`, which is always
/// strictly less than `quality_run_deadline` by at least bound(k) > 0.
/// The pre-dispatch check in front of the run's LAST call therefore never
/// sees `elapsed >= quality_run_deadline`, headroom or not.
///
/// **Why it matters that this is exact rather than generous.** The stages
/// whose own timeout is user-facing — `analyze_job`/`strategy` (their
/// `complete_json`'s `?` propagates) and `draft`/`cover_letter` (their
/// streamed call's `?` propagates) — run FIRST and together spend at most
/// `json_half + generation`, strictly less than this deadline by exactly
/// `QUALITY_RUN_FIXED_SECS`: the share reserved for `repair`/`humanize`,
/// which run AFTER and have not spent it yet. So none of those four
/// stages' pre-dispatch checks can ever fire early.
/// `repair`/`humanize`, which run last, do the opposite by design (see
/// their own module docs: "No error here fails the run" / a per-call
/// timeout there is a FAILED ATTEMPT) — a hung call is swallowed and
/// reverted, never surfaced as a message, so there is no actionable
/// per-call error left for the generic `RunTimeout` to steal even in the
/// one place this deadline's own margin is genuinely zero.
///
/// The inner bounds are computed from the FAN-OUT CONSTANTS themselves, not
/// from the deadline's own terms: two JSON stages each allowed one
/// re-ask (now bounded by [`ollama_completion_deadline`], scaled), plus
/// `max_repair_attempts × MAX_SECTIONS_PER_ROUND` section rewrites and
/// `humanize`'s allowance — both still bounded by the FLAT
/// `OLLAMA_COMPLETION_BASELINE` — plus the one streamed draft. Two
/// INDEPENDENT derivations landing on the same number is what makes this a
/// real guard rather than a tautology: raising either repair constant
/// without raising the deadline fails here.
///
/// **One scaled call per JSON-stage round-trip, not `MAX_ATTEMPTS` of
/// them.** That holds only because `retry::send_with_retry` bounds the
/// whole retry sequence by the caller's timeout; the guard for THAT half
/// lives next to it (`a_one_shot_call_stops_once_its_own_timeout_is_spent`),
/// because it is a property of the loop's wall clock, not of this
/// arithmetic. Multiplying the term by `MAX_ATTEMPTS` here instead would
/// pin a dependency the code no longer has — an identity of exactly the
/// kind this test was rebuilt to stop being.
///
/// Mutation checks (applied and reverted): `QUALITY_RUN_FIXED_SECS` back to
/// 1_800 ⇒ every tier fails; `MAX_SECTIONS_PER_ROUND` 4 → 6 ⇒ every tier
/// fails; `DEFAULT_MAX_REPAIR_ATTEMPTS` 2 → 3 ⇒ every tier fails;
/// `QUALITY_RUN_GENERATION_PASSES` 2 → 1 ⇒ every tier fails (only the draft
/// would be covered, not the letter); `quality_run_deadline`'s `json_stages`
/// term dropped back to a flat `OLLAMA_COMPLETION_BASELINE` ⇒ every tier
/// above the bottom fails.
#[test]
fn quality_run_deadline_equals_the_inner_per_call_bounds() {
    const JSON_STAGES: u32 = 2;
    const ROUND_TRIPS_PER_JSON_STAGE: u32 = 2; // the one budgeted re-ask
                                               // `humanize` makes at most one flat `complete` call per FLAGGED
                                               // document (résumé, letter) — the worst case this deadline has to
                                               // cover, exactly like every other term here.
    const HUMANIZE_MAX_CALLS: u32 = 2;
    // The repair fan-out and `humanize` are bounded by the SAME flat
    // per-call constant — both go through `Completer::complete`, never a
    // stream, and neither has an `effort` to scale by — so they stay
    // effort-invariant.
    let repair_half = OLLAMA_COMPLETION_BASELINE
        * crate::pipeline::budget::Budget::RESUME_QUALITY.max_repair_attempts as u32
        * crate::pipeline::resume::stages::MAX_SECTIONS_PER_ROUND as u32;
    let humanize_half = OLLAMA_COMPLETION_BASELINE * HUMANIZE_MAX_CALLS;
    for effort in [
        None,
        Some("medium"),
        Some("high"),
        Some("xhigh"),
        Some("max"),
    ] {
        // The JSON stages now scale exactly like the streamed calls do.
        let json_half =
            ollama_completion_deadline(effort) * JSON_STAGES * ROUND_TRIPS_PER_JSON_STAGE;
        // The draft and the cover letter are the only streamed calls the
        // run makes — see `QUALITY_RUN_GENERATION_PASSES`.
        let generation = stream_deadline(effort)
            * crate::ipc_contracts::ai_timeouts::QUALITY_RUN_GENERATION_PASSES as u32;
        // `assert_eq!`, not `>=`: the two derivations are provably
        // identical at every tier (see this test's own doc comment for
        // why exact equality — no headroom — is what the discrete,
        // never-mid-call `RunDeadline` check actually needs), so pin the
        // equality itself rather than a one-sided bound a wider
        // `quality_run_deadline` could also satisfy without anyone
        // noticing the two sides had drifted apart.
        assert_eq!(
            quality_run_deadline(effort),
            json_half + repair_half + humanize_half + generation,
            "quality_run_deadline({effort:?}) must equal the calls it wraps"
        );
    }
}

/// The budget constant is the FLOOR the effort-blind path falls back to, so
/// the two must agree at the bottom tier — otherwise `run_deadline`'s
/// `max()` silently picks whichever is larger and the derivation in
/// `ai-timeouts.ts` stops describing what actually runs.
#[test]
fn quality_run_deadline_agrees_with_the_budget_floor_at_the_bottom_tier() {
    assert_eq!(
        quality_run_deadline(None),
        crate::pipeline::budget::Budget::RESUME_QUALITY.run_timeout
    );
}

#[test]
fn quality_run_deadline_is_monotonically_nondecreasing_by_effort_tier() {
    let tiers = [
        None,
        Some("minimal"),
        Some("low"),
        Some("medium"),
        Some("high"),
        Some("xhigh"),
        Some("max"),
    ];
    let mut prev = Duration::from_secs(0);
    for effort in tiers {
        let d = quality_run_deadline(effort);
        assert!(
            d >= prev,
            "quality_run_deadline({effort:?}) = {d:?} < {prev:?}"
        );
        prev = d;
    }
    assert!(quality_run_deadline(Some("max")) > quality_run_deadline(None));
}

#[test]
fn quality_run_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
    assert_eq!(
        quality_run_deadline(Some("ultra-mega-think")),
        quality_run_deadline(None)
    );
}
