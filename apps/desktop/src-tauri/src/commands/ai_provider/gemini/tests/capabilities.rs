//! `gemini_effort_levels` per-model tier table and the
//! `GeminiClient::capabilities`/`effort_levels` trait methods built on it.

use super::super::super::AiProvider;
use super::super::capabilities::gemini_effort_levels;
use super::super::GeminiClient;

#[test]
fn capabilities_supports_reasoning_mirrors_the_v3_gate() {
    assert!(
        GeminiClient
            .capabilities("gemini-3-pro-preview")
            .supports_reasoning
    );
    assert!(
        !GeminiClient
            .capabilities("gemini-2.5-pro")
            .supports_reasoning
    );
}

#[test]
fn effort_levels_are_looked_up_per_model_tier_not_per_provider() {
    // gemini-3-pro-preview is SHUT DOWN (checked 2026-08-04) — this
    // assertion is intentional, not stale: it locks in the row's kept
    // historical value (see the doc comment on `gemini_effort_levels`),
    // not a claim the model is selectable.
    assert_eq!(
        gemini_effort_levels("gemini-3-pro-preview"),
        vec!["low", "high"]
    );
    assert_eq!(
        gemini_effort_levels("gemini-3.1-pro-preview"),
        vec!["low", "medium", "high"]
    );
    assert_eq!(
        gemini_effort_levels("gemini-3.1-flash-lite-image"),
        vec!["minimal", "high"]
    );
    assert_eq!(
        gemini_effort_levels("gemini-3-flash-preview"),
        vec!["minimal", "low", "medium", "high"]
    );
    assert_eq!(
        gemini_effort_levels("gemini-3.6-flash"),
        vec!["minimal", "low", "medium", "high"]
    );
    // `gemini-3.1-flash-lite` (the TEXT model, distinct from `-image`) has no
    // row in the live thinking table — this locks in the safe universal
    // fallback so a future "fix" doesn't silently guess it belongs in the
    // full-level branch (see the doc comment above `gemini_effort_levels`).
    assert_eq!(gemini_effort_levels("gemini-3.1-flash-lite"), vec!["high"]);
    // An unrecognized future v3+ id falls back to the one universally-safe
    // level, never a guess that could 400.
    assert_eq!(gemini_effort_levels("gemini-4-pro"), vec!["high"]);
    // Pre-v3 models get no levels at all.
    assert!(gemini_effort_levels("gemini-2.5-pro").is_empty());
    assert!(gemini_effort_levels("gemini-1.5-flash").is_empty());
}

#[test]
fn capabilities_effort_levels_matches_the_free_function() {
    assert_eq!(
        GeminiClient.effort_levels("gemini-3-pro-preview"),
        gemini_effort_levels("gemini-3-pro-preview")
    );
}
