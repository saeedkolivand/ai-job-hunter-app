//! `output_config.effort` support/levels gates and the native
//! structured-output support gate.

use super::super::super::AiProvider;
use super::super::capabilities::{
    anthropic_effort_levels, anthropic_supports_effort, anthropic_supports_structured_outputs,
};
use super::super::AnthropicClient;
use super::support::caps_for;

#[test]
fn effort_gate_matches_the_documented_model_list() {
    for m in [
        "claude-opus-4-5",
        "claude-opus-4.5",
        "claude-opus-4-6",
        "claude-opus-4-7",
        "claude-opus-4-8",
        "claude-sonnet-4-6",
        "claude-sonnet-5",
        "claude-opus-5",
        "claude-fable-5",
        "claude-mythos-5",
        "claude-mythos-preview",
    ] {
        assert!(anthropic_supports_effort(m), "{m} should support effort");
    }
    // Models NOT in Anthropic's documented effort list — including thinking-
    // capable ones (classic 3.7/4.x, and Sonnet 4.5 which is adjacent to but
    // distinct from the documented Sonnet 4.6) — must stay off.
    for m in [
        "claude-3-7-sonnet-20250219",
        "claude-sonnet-4",
        "claude-sonnet-4-5",
        "claude-haiku-4-5",
        "claude-3-5-sonnet-20241022",
    ] {
        assert!(!anthropic_supports_effort(m), "{m} must not support effort");
    }
}

#[test]
fn effort_gate_needles_are_boundary_aware_not_raw_substring() {
    // The classic prefix-collision trap version-needle gates guard elsewhere:
    // a longer version number must not falsely match a shorter documented
    // needle.
    assert!(!anthropic_supports_effort("claude-sonnet-50"));
    assert!(!anthropic_supports_effort("claude-opus-4-50"));
    // A bare "mythos" with no recognized suffix, or an unlisted future
    // Mythos version, must not guess `true` — only the two currently
    // documented names ("Mythos 5", "Mythos Preview") match.
    assert!(!anthropic_supports_effort("claude-mythos"));
    assert!(!anthropic_supports_effort("claude-mythos-6"));
}

#[test]
fn effort_levels_mirror_the_effort_gate() {
    assert_eq!(
        AnthropicClient.effort_levels("claude-opus-5"),
        vec!["low", "medium", "high", "max", "xhigh"]
    );
    assert!(AnthropicClient
        .effort_levels("claude-3-5-sonnet-20241022")
        .is_empty());
}

#[test]
fn effort_levels_are_looked_up_per_model_tier_not_binary() {
    // Full 5-level tier.
    for m in [
        "claude-fable-5",
        "claude-mythos-5",
        "claude-opus-5",
        "claude-opus-4-8",
        "claude-opus-4-7",
        "claude-sonnet-5",
    ] {
        assert_eq!(
            anthropic_effort_levels(m),
            vec!["low", "medium", "high", "max", "xhigh"],
            "{m} should support the full 5-level set"
        );
    }
    // `max` but no `xhigh`.
    for m in [
        "claude-mythos-preview",
        "claude-opus-4-6",
        "claude-sonnet-4-6",
    ] {
        assert_eq!(
            anthropic_effort_levels(m),
            vec!["low", "medium", "high", "max"],
            "{m} should support max but not xhigh"
        );
    }
    // Neither `max` nor `xhigh` — the one extended-thinking-only exception.
    assert_eq!(
        anthropic_effort_levels("claude-opus-4-5"),
        vec!["low", "medium", "high"]
    );
    // Not an effort-capable model at all -> no levels.
    assert!(anthropic_effort_levels("claude-3-5-sonnet-20241022").is_empty());
}

#[test]
fn capabilities_supports_reasoning_mirrors_the_effort_gate() {
    assert_eq!(
        caps_for("claude-opus-5").supports_reasoning,
        anthropic_supports_effort("claude-opus-5")
    );
    assert_eq!(
        caps_for("claude-3-5-sonnet-20241022").supports_reasoning,
        anthropic_supports_effort("claude-3-5-sonnet-20241022")
    );
}

#[test]
fn structured_outputs_gate_covers_the_4_5_generation_and_later_plus_opus_4_1() {
    for model in [
        "claude-opus-4-1-20250805",
        "claude-opus-4-5",
        "claude-sonnet-4-5-20250929",
        "claude-haiku-4-5",
        "claude-sonnet-4-6",
        "claude-opus-4-8",
        "claude-opus-5",
        "claude-fable-5",
        // Vendor-prefixed (OpenRouter-style) ids normalize the same way every
        // other gate in this file does.
        "anthropic/claude-sonnet-5",
        // Dot-form version separators collapse to dashes too.
        "claude-opus-4.5",
    ] {
        assert!(
            anthropic_supports_structured_outputs(model),
            "{model} must be recognized as structured-output capable"
        );
    }
}

#[test]
fn structured_outputs_gate_rejects_pre_4_5_models_and_anything_unrecognized() {
    for model in [
        // Claude 3.x and the 4.0 models never got structured outputs.
        "claude-3-opus-20240229",
        "claude-3-5-sonnet-20241022",
        "claude-3-7-sonnet-20250219",
        "claude-opus-4-20250514",
        "claude-sonnet-4-20250514",
        // An unrecognized/future id defaults to unsupported — the fallback
        // always works, a wrongly-claimed capability does not.
        "claude-something-new",
        "",
        // Boundary check: `opus-4-50` must not match the `opus-4-5` needle.
        "claude-opus-4-50",
    ] {
        assert!(
            !anthropic_supports_structured_outputs(model),
            "{model} must NOT be claimed as structured-output capable"
        );
    }
}

#[test]
fn capabilities_reports_json_mode_from_the_structured_output_gate() {
    // The declared capability is the gate, not a hardcoded blanket value —
    // mutation check: pin `supports_json_mode` back to `false` and this fails.
    assert!(
        AnthropicClient
            .capabilities("claude-sonnet-4-5")
            .supports_json_mode
    );
    assert!(
        !AnthropicClient
            .capabilities("claude-3-5-sonnet-20241022")
            .supports_json_mode
    );
}
