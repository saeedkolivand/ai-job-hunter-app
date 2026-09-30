//! Pure key resolution + `/models` catalogue parsing/filtering (no HTTP —
//! see `list_models` for the wiremock transport tests) plus the two
//! embedding-catalogue gates built on the same per-provider-id classification.

use serde_json::{json, Value};

use super::super::super::{AiProvider, ProviderId};
use super::super::transport::{parse_model_list, resolve_openai_key, should_list_model};
use super::super::OpenAiClient;
use crate::error::AppError;

#[test]
fn resolve_openai_key_errors_on_missing_or_blank_key_for_native_openai() {
    assert!(matches!(
        resolve_openai_key(ProviderId::OpenAi, None),
        Err(AppError::Config(_))
    ));
    assert!(matches!(
        resolve_openai_key(ProviderId::OpenAi, Some("  ".to_string())),
        Err(AppError::Config(_))
    ));
}

#[test]
fn resolve_openai_key_errors_on_missing_key_for_ollama_cloud() {
    // Only `OpenAiCompatible` gets the keyless exemption — Ollama Cloud is a
    // hosted cloud service (via the same composed client) and still requires
    // its account key.
    assert!(matches!(
        resolve_openai_key(ProviderId::OllamaCloud, None),
        Err(AppError::Config(_))
    ));
}

#[test]
fn resolve_openai_key_accepts_a_real_key() {
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAi, Some("sk-real".to_string()))
            .unwrap()
            .as_deref(),
        Some("sk-real")
    );
}

#[test]
fn resolve_openai_key_trims_the_returned_key_not_just_the_checked_one() {
    // A pasted key with a trailing space/newline must reach `bearer_auth`
    // TRIMMED — checking `k.trim().is_empty()` but returning the padded `k`
    // is the exact bug: a trailing space just 401s, an embedded `\n` makes
    // the header value invalid and the request never builds at all.
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAi, Some(" sk-real \n".to_string()))
            .unwrap()
            .as_deref(),
        Some("sk-real")
    );
}

#[test]
fn resolve_openai_key_allows_a_missing_or_blank_key_for_openai_compatible() {
    // A keyless self-hosted deployment (LM Studio, vLLM, …) is explicitly
    // supported — generation already works with no key
    // (`chat_stream`/`chat_with_tools` default a missing key to `""`), so
    // listing/testing must not hard-error just because no key is stored.
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAiCompatible, None).unwrap(),
        None
    );
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAiCompatible, Some("   ".to_string())).unwrap(),
        None
    );
}

#[test]
fn resolve_openai_key_still_prefers_a_real_key_for_openai_compatible_when_present() {
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAiCompatible, Some("real-key".to_string()))
            .unwrap()
            .as_deref(),
        Some("real-key")
    );
}

#[test]
fn parse_model_list_applies_the_native_openai_chat_filter() {
    let body = json!({
        "data": [{ "id": "gpt-4o" }, { "id": "text-embedding-3-small" }]
    });
    let names: Vec<String> = parse_model_list(ProviderId::OpenAi, &body)
        .unwrap()
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["gpt-4o"]);
}

#[test]
fn parse_model_list_passes_through_unfiltered_for_openai_compatible() {
    let body = json!({ "data": [{ "id": "gpt-oss:120b" }] });
    let names: Vec<String> = parse_model_list(ProviderId::OpenAiCompatible, &body)
        .unwrap()
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["gpt-oss:120b"]);
}

#[test]
fn parse_model_list_normalizes_created_epoch_seconds_to_millis() {
    // OpenAI's `created` is unix-epoch SECONDS — 1704067200 is the well-known
    // 2024-01-01T00:00:00Z reference point; the normalized `createdAt` must
    // be that value in MILLISECONDS, this codebase's `createdAt` convention.
    let body = json!({
        "data": [{ "id": "gpt-4o", "created": 1_704_067_200i64 }]
    });
    let page = parse_model_list(ProviderId::OpenAi, &body).unwrap();
    assert_eq!(
        page,
        vec![json!({ "name": "gpt-4o", "createdAt": 1_704_067_200_000i64 })]
    );
}

#[test]
fn parse_model_list_omits_created_at_rather_than_overflow_on_a_pathological_value() {
    // `created` is provider-controlled — a bare `secs * 1000` can overflow
    // `i64` (panics in debug, silently wraps in release). Must degrade to
    // "omit the field", never fabricate a wrapped/garbage timestamp.
    let body = json!({
        "data": [{ "id": "gpt-4o", "created": i64::MAX }]
    });
    let page = parse_model_list(ProviderId::OpenAi, &body).unwrap();
    assert_eq!(page, vec![json!({ "name": "gpt-4o" })]);
}

#[test]
fn parse_model_list_omits_optional_fields_the_provider_does_not_return_and_keeps_name_unchanged() {
    // `name` must stay byte-identical to the pre-widening shape — a stored
    // model preference matches against it. OpenAI's `/v1/models` never
    // returns `displayName`/`contextLength` at all.
    let body = json!({ "data": [{ "id": "gpt-4o" }] });
    let page = parse_model_list(ProviderId::OpenAi, &body).unwrap();
    assert_eq!(page, vec![json!({ "name": "gpt-4o" })]);
}

#[test]
fn parse_model_list_ok_empty_on_genuinely_empty_catalogue() {
    let body = json!({ "data": [] });
    assert_eq!(
        parse_model_list(ProviderId::OpenAi, &body).unwrap(),
        Vec::<Value>::new()
    );
}

#[test]
fn parse_model_list_errors_when_data_field_is_missing() {
    let body = json!({ "unexpected": "shape" });
    assert!(matches!(
        parse_model_list(ProviderId::OpenAi, &body),
        Err(AppError::Provider(_))
    ));
}

#[test]
fn list_filter_only_restricts_native_openai() {
    // Native OpenAI exposes a large non-chat catalog — keep only chat families.
    assert!(should_list_model(ProviderId::OpenAi, "gpt-4o"));
    assert!(should_list_model(ProviderId::OpenAi, "o3-mini"));
    assert!(should_list_model(ProviderId::OpenAi, "chatgpt-4o-latest"));
    for non_chat in ["text-embedding-3-small", "dall-e-3", "whisper-1", "tts-1"] {
        assert!(
            !should_list_model(ProviderId::OpenAi, non_chat),
            "{non_chat} should be filtered out for native OpenAI"
        );
    }

    // Ollama Cloud + generic OpenAI-compatible servers return their own
    // curated catalog under arbitrary names — never filter those, so the
    // full Ollama Cloud list (not just gpt-oss:*) reaches the picker.
    for id in [
        "gpt-oss:120b",
        "qwen3-coder:480b",
        "deepseek-v3.1:671b",
        "kimi-k2:1t",
        "glm-4.6",
    ] {
        assert!(should_list_model(ProviderId::OllamaCloud, id), "{id}");
        assert!(should_list_model(ProviderId::OpenAiCompatible, id), "{id}");
    }
}

/// Regression: `text-embedding-3-small` is an OPENAI model id, and every
/// OpenAI-compatible client is built on this one — Ollama Cloud delegates this
/// method straight through, and `OpenAiCompatible` covers LM Studio / vLLM /
/// OpenRouter. Returning it for those posted an OpenAI model to a gateway that
/// has never heard of it, so embeddings failed with a model-not-found that read
/// as "embeddings are broken" instead of "pick an embedding model".
#[test]
fn default_embedding_model_is_offered_only_for_native_openai() {
    assert_eq!(
        OpenAiClient::new(ProviderId::OpenAi, None).default_embedding_model(),
        Some("text-embedding-3-small")
    );
    assert_eq!(
        OpenAiClient::new(
            ProviderId::OpenAiCompatible,
            Some("http://localhost:1234/v1".into())
        )
        .default_embedding_model(),
        None,
        "an openai-compatible gateway serves its own catalogue — never presume an OpenAI model id"
    );
    assert_eq!(
        OpenAiClient::new(ProviderId::OllamaCloud, None).default_embedding_model(),
        None,
        "ollama-cloud delegates this method, so the gate has to hold at the inner client"
    );
}

#[test]
fn embedding_cap_is_token_safe_for_every_language() {
    // text-embedding-3-* hard-error past 8191 tokens. Token-dense scripts (CJK)
    // run ≈1 char/token, so the char cap must itself stay under 8191 — otherwise
    // a full-cap CJK input exceeds the token limit and the request FAILS.
    let cap = OpenAiClient::new(ProviderId::OpenAi, None).max_embedding_input_chars();
    assert!(
        cap <= 8191,
        "char cap {cap} can exceed 8191 tokens for ~1-char/token languages"
    );
    // Sanity: still a useful amount of text (not collapsed to near-zero).
    assert!(cap >= 4_000, "cap {cap} truncates too aggressively");
}
