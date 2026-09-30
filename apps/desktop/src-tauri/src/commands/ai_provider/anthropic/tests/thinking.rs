//! Thinking-mode classification: which models get classic vs adaptive
//! thinking, the unrecognized-family fail-safe, and the boundary-aware
//! version-needle matcher they all build on.

use serde_json::json;

use super::super::body::build_chat_stream_body;
use super::super::capabilities::{
    anthropic_supports_effort, anthropic_supports_structured_outputs,
    anthropic_supports_temperature,
};
use super::super::thinking::{
    anthropic_supports_thinking, anthropic_uses_adaptive_thinking, classic_thinking_engages,
};
use super::support::{base_request, caps_for, sampling_for};

#[test]
fn thinking_gate_enables_only_extended_thinking_models() {
    for m in [
        "claude-3-7-sonnet-20250219",
        "claude-3.7-sonnet",
        "claude-opus-4-20250514",
        "claude-sonnet-4-5",
        "claude-haiku-4",
    ] {
        assert!(anthropic_supports_thinking(m), "{m} should enable thinking");
    }
    // Pre-3.7 models 400 on a `thinking` block — must stay off.
    for m in [
        "claude-3-haiku-20240307",
        "claude-3-5-sonnet-20241022",
        "claude-3-opus-20240229",
        "claude-2.1",
    ] {
        assert!(
            !anthropic_supports_thinking(m),
            "{m} must not request thinking (it 400s)"
        );
    }
}

#[test]
fn thinking_gates_normalize_dot_form_version_ids() {
    // A dot-form id ("claude-opus-4.7") must normalize the same as its
    // dash-form equivalent — otherwise it misses the "opus-4-7" needle,
    // falls through to the classic "claude-opus-4" gate, and 400s (Opus
    // 4.7 is adaptive-only; it rejects the classic `thinking.enabled` shape).
    assert!(anthropic_uses_adaptive_thinking("claude-opus-4.7"));
    assert!(anthropic_uses_adaptive_thinking("claude-opus-4.8"));
    assert!(!anthropic_supports_thinking("claude-opus-4.7"));
    assert!(!anthropic_supports_thinking("claude-opus-4.8"));
}

#[test]
fn thinking_gate_excludes_the_claude_5_family() {
    // The 5 family (Opus 5, Sonnet 5, Fable 5) replaced classic
    // budget-token thinking with adaptive thinking — sending the classic
    // `thinking` block to them would 400, so the gate must stay off.
    for m in [
        "claude-opus-5",
        "claude-sonnet-5",
        "claude-fable-5",
        "claude-fable-5-20260201",
    ] {
        assert!(
            !anthropic_supports_thinking(m),
            "{m} must not request classic thinking (it 400s; uses adaptive thinking instead)"
        );
    }
}

#[test]
fn thinking_gate_excludes_opus_4_7_and_4_8_despite_matching_claude_opus_4() {
    // Opus 4.7/4.8 are adaptive-ONLY per Anthropic's per-model table
    // ("Extended thinking: No") — they must not fall into the classic
    // gate just because they match the "claude-opus-4" substring.
    for m in ["claude-opus-4-7", "claude-opus-4-8"] {
        assert!(
            !anthropic_supports_thinking(m),
            "{m} is adaptive-only; must not receive classic thinking"
        );
    }
}

#[test]
fn adaptive_gate_matches_opus_4_7_4_8_and_the_5_family() {
    for m in [
        "claude-opus-4-7",
        "claude-opus-4-8",
        "claude-opus-5",
        "claude-opus-5-20260201",
        "claude-sonnet-5",
        "claude-fable-5",
        "claude-fable-5-20260201",
        // Every real Mythos id shape must keep passing the gate after the bare
        // `contains("mythos")` was replaced with the boundary-aware needle:
        // the two documented releases, a dated build, and a vendor prefix.
        "claude-mythos-5",
        "claude-mythos-preview",
        "claude-mythos-5-20260201",
        "anthropic/claude-mythos-5",
        // …and the bare family word too. Unlike `anthropic_supports_effort`
        // (a closed list of documented VERSIONS, which asserts the opposite for
        // this exact id), this gate covers the Mythos FAMILY, so an unlisted
        // future point release stays adaptive with no code change. Guessing
        // wrong is safe here and 400s there — see both doc comments.
        "claude-mythos",
        "claude-mythos-6",
    ] {
        assert!(
            anthropic_uses_adaptive_thinking(m),
            "{m} should use adaptive thinking"
        );
    }
    // Pre-5/pre-4.7 models must never be misclassified as adaptive — in
    // particular "claude-sonnet-4-5" (Sonnet 4.5) must not match on a bare
    // "-5" substring.
    for m in [
        "claude-opus-4-20250514",
        "claude-sonnet-4-5",
        "claude-3-5-sonnet-20241022",
        "claude-3-haiku-20240307",
    ] {
        assert!(
            !anthropic_uses_adaptive_thinking(m),
            "{m} must not be classified as adaptive"
        );
    }
}

#[test]
fn unrecognized_claude_family_fails_safe_to_no_temperature() {
    // A future Anthropic family not yet in either needle list (e.g. a
    // hypothetical "Zephyr" release) must default to NO temperature, not
    // a blanket "true" — sending a non-default temperature 400s on every
    // adaptive model, while omitting it is always accepted. Defaulting
    // an unrecognized "claude-"-prefixed id to the safe direction is what
    // restores the zero-code-change promise: a brand-new Claude model
    // works safely even before this adapter learns its needle.
    assert!(!anthropic_supports_temperature("claude-zephyr-6"));
    assert!(!caps_for("claude-zephyr-6").supports_temperature);

    let mut req = base_request("claude-zephyr-6");
    req.max_tokens = Some(4096);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("temperature").is_none(),
        "an unrecognized claude- family must not receive a non-default temperature"
    );
    assert!(body.get("thinking").is_none());
}

#[test]
fn unrecognized_claude_family_fails_safe_even_behind_a_vendor_prefix() {
    // A vendor-prefixed id (as seen through an OpenRouter-style gateway) must
    // classify identically to its bare form — before stripping the prefix in
    // `normalize_model_id`, "anthropic/claude-zephyr-6" failed the
    // `starts_with("claude-")` check (it starts with "anthropic/" instead),
    // silently disarming the new-family fail-safe and sending a non-default
    // temperature that would 400 if this actually were a new adaptive family.
    assert!(!anthropic_supports_temperature("anthropic/claude-zephyr-6"));
    assert!(!caps_for("anthropic/claude-zephyr-6").supports_temperature);

    let mut req = base_request("anthropic/claude-zephyr-6");
    req.max_tokens = Some(4096);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("temperature").is_none(),
        "a vendor-prefixed unrecognized claude- family must still fail safe"
    );
}

#[test]
fn version_needles_are_boundary_aware_not_raw_substring() {
    // "claude-opus-4-70" contains "opus-4-7" as a raw substring but is NOT
    // Opus 4.7 (a different, unclassified point release) — a boundary-aware
    // match must not misclassify it as the adaptive-only opus-4-7/4-8 shape.
    assert!(!anthropic_uses_adaptive_thinking("claude-opus-4-70"));
    // It's still a plain 4.x id, so it's fine (and correct) for it to keep
    // matching the broader classic "claude-opus-4" gate.
    assert!(anthropic_supports_thinking("claude-opus-4-70"));

    // Same class of bug for "opus-5": "claude-opus-50" must not be treated
    // as the Opus 5 (adaptive) family.
    assert!(!anthropic_uses_adaptive_thinking("claude-opus-50"));
}

#[test]
fn version_needles_reject_a_needle_glued_to_a_neighbouring_component() {
    // The trailing-DIGIT-only boundary above left the same collision reachable
    // from the other two sides, and both fail OPEN — an unrecognized id
    // silently classified as a known family, the one direction these gates
    // promise they never fail in. Mutation check: restore the
    // `!b.is_ascii_digit()` suffix-only check in `contains_version_needle` and
    // every assertion here fails.
    //
    // 1. A glued PREFIX: "notopus-4-5" contains "opus-4-5" outright.
    assert!(!anthropic_supports_structured_outputs("claude-notopus-4-5"));
    assert!(!anthropic_uses_adaptive_thinking("claude-notopus-5"));
    assert!(!anthropic_supports_effort("claude-notopus-4-5"));
    // 2. A glued non-digit SUFFIX: only a digit used to be rejected.
    assert!(!anthropic_supports_structured_outputs(
        "claude-sonnet-4-5alpha"
    ));
    assert!(!anthropic_uses_adaptive_thinking("claude-opus-5x"));
    // …including the adaptive gate's one BARE FAMILY WORD, which was still a
    // raw `m.contains("mythos")` after the version needles were fixed — the
    // same fail-open direction, one needle later.
    assert!(!anthropic_uses_adaptive_thinking("claude-notmythos-9"));
    assert!(!anthropic_uses_adaptive_thinking("claude-mythos9"));
    // 3. Both at once.
    assert!(!anthropic_supports_structured_outputs(
        "xclaude-notopus-4-5beta"
    ));

    // …while every SEPARATOR a real id uses still matches: the dash-dated
    // form, a Bedrock-style dotted vendor id (dots fold to dashes), a Vertex
    // `@`-suffixed one, and an OpenRouter vendor prefix.
    for model in [
        "claude-sonnet-4-5-20250929",
        "anthropic.claude-opus-4-5-v1:0",
        "claude-opus-4-5@20251101",
        "anthropic/claude-opus-4-5",
    ] {
        assert!(
            anthropic_supports_structured_outputs(model),
            "{model} is a real id shape and must still match its needle"
        );
    }
}

#[test]
fn legacy_pre_thinking_models_keep_temperature_despite_matching_neither_gate() {
    // These also match neither `anthropic_supports_thinking` nor
    // `anthropic_uses_adaptive_thinking` (they predate thinking entirely)
    // — the new-family fail-safe above must NOT catch them too, or every
    // long-shipped Claude 3.x/2.x call silently loses its temperature.
    for m in [
        "claude-3-haiku-20240307",
        "claude-3-5-sonnet-20241022",
        "claude-3-opus-20240229",
        "claude-2.1",
    ] {
        assert!(
            anthropic_supports_temperature(m),
            "{m} is a known legacy model and must keep normal temperature support"
        );
    }
    let req = base_request("claude-3-5-sonnet-20241022");
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["temperature"], json!(0.8));
}

/// The gate's own threshold, pinned at the boundary now that it has a name.
/// The body-level tests around this one only ever observe 1000 (off) and 4096
/// (on), so the literal itself was unguarded — mutation-checked: moving it to
/// `>= 1024` leaves every other test in this file green, and only this one
/// goes red. Callers outside this module size their budgets against this
/// number (see [`classic_thinking_engages`]' own doc).
#[test]
fn classic_thinking_engages_exactly_at_the_2048_boundary() {
    assert!(
        !classic_thinking_engages(2047),
        "one token below the gate must leave classic thinking off"
    );
    assert!(
        classic_thinking_engages(2048),
        "the gate engages AT 2048, not above it"
    );
}

/// The one cross-module relationship the extension bridge's compose budgets
/// were SIZED by, asserted HERE — next to the predicate they are sized
/// against — rather than in that module, so no test-only re-export of this
/// gate has to exist for it: on a classic-thinking Claude model,
/// `build_chat_stream_body` switches extended thinking on once `max_tokens`
/// reaches the gate and then adds a thinking budget on top. The bridge's
/// first attempt must stay under it — that path exists to buy LESS reasoning,
/// and crossing the gate also forces `temperature` to 1.0 — while its ONE
/// retry deliberately crosses it, because that attempt only happens after a
/// model proved it needs room to think AND answer, which is exactly what
/// classic mode then budgets separately.
///
/// Asserted against the predicate rather than a re-typed 2048, so the two can
/// never drift apart silently (the threshold is Anthropic's, not ours).
///
/// Mutation check (executed): set `ANSWER_ASSIST_MAX_TOKENS` to 2048 — the
/// first assertion fails.
#[test]
fn the_extension_bridge_compose_budget_stays_under_the_classic_thinking_gate() {
    use crate::extension_bridge::answer_assist::{
        ANSWER_ASSIST_MAX_TOKENS, ANSWER_ASSIST_RETRY_MAX_TOKENS,
    };

    assert!(
        !classic_thinking_engages(ANSWER_ASSIST_MAX_TOKENS),
        "the first attempt must not switch Anthropic classic extended \
         thinking on — that path is here to buy LESS reasoning"
    );
    assert!(
        classic_thinking_engages(ANSWER_ASSIST_RETRY_MAX_TOKENS),
        "the retry crossing the gate is the one DELIBERATE exception (see \
         ANSWER_ASSIST_RETRY_MAX_TOKENS' doc): it runs only after the model \
         spent the whole first budget thinking, so on Anthropic it wants the \
         separately-budgeted thinking the gate turns on"
    );
}
