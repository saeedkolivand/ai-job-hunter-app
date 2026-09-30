//! `GeminiClient::default_embedding_model`/`max_embedding_input_chars` and
//! `build_embed_body`'s output-dimensionality wire shape.

use serde_json::json;

use super::super::super::AiProvider;
use super::super::body::build_embed_body;
use super::super::GeminiClient;
use super::super::EMBED_OUTPUT_DIMENSIONALITY;

#[test]
fn default_embedding_model_is_not_the_retired_text_embedding_004() {
    // text-embedding-004 was retired by Google (shutdown Jan 14, 2026) —
    // the exact "model or endpoint not found" error this app was seeing.
    let model = GeminiClient.default_embedding_model().unwrap();
    assert_ne!(model, "text-embedding-004");
    assert_eq!(model, "gemini-embedding-2");
}

#[test]
fn embed_body_requests_the_reduced_output_dimensionality() {
    // Without this, gemini-embedding-2 defaults to 3072 dims (4x the
    // retired text-embedding-004's 768), quadrupling stored-vector size
    // for no accuracy benefit this app uses. Must be NESTED inside
    // `embedContentConfig` (camelCase): the v1beta discovery document
    // (revision 20260806) marks `EmbedContentRequest.outputDimensionality`
    // `"deprecated": true` — *"Please use
    // EmbedContentConfig.output_dimensionality instead"* — while
    // `embedContentConfig` carries no such marker. See `build_embed_body`'s
    // doc comment; "move it up one level" is a recurring review suggestion
    // that targets the DEPRECATED location.
    let body = build_embed_body("gemini-embedding-2", "hello");
    assert_eq!(
        body["embedContentConfig"]["outputDimensionality"],
        json!(EMBED_OUTPUT_DIMENSIONALITY)
    );
    assert_eq!(EMBED_OUTPUT_DIMENSIONALITY, 768);
    // Never at the deprecated top-level location.
    assert!(body.get("output_dimensionality").is_none());
    assert!(body.get("outputDimensionality").is_none());
    assert_eq!(body["model"], json!("models/gemini-embedding-2"));
    assert_eq!(body["content"]["parts"][0]["text"], json!("hello"));
}

#[test]
fn embedding_cap_is_within_the_documented_token_limit_range() {
    // Drift-pinning only, NOT proof of token safety on its own (that would
    // need a real tokenizer, which this crate doesn't have) — it just
    // pins the char cap to a sane range relative to gemini-embedding-2's
    // documented 8,192-token limit, so a future edit can't silently set
    // it absurdly high or low. The real per-language safety net is
    // `embed_chunk_adaptive`'s halve-and-retry on an actual provider
    // context-length error, which this test does not exercise.
    let cap = GeminiClient.max_embedding_input_chars();
    assert!(cap <= 8192, "char cap {cap} can exceed 8192 tokens");
    assert!(cap >= 4_000, "cap {cap} truncates too aggressively");
}
