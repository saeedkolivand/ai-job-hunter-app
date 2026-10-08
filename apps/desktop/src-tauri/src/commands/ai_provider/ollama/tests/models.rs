//! `/api/tags` parsing + the embedding-vs-chat name heuristic (the
//! `qwen3-embedding:8b` → `/api/chat` 400 regression: `reachable_model`
//! returned `/api/tags`'s FIRST entry unconditionally, and a user whose
//! first entry was an embedding model took a guaranteed 400).

use serde_json::{json, Value};

use super::super::models::{
    first_chat_model, is_embedding_only_model, parse_model_list, preferred_chat_model,
};
use crate::error::AppError;

#[test]
fn parse_model_list_maps_tag_names() {
    let body = json!({
        "models": [{ "name": "llama3.1:8b" }, { "name": "gpt-oss:20b" }]
    });
    let names: Vec<String> = parse_model_list(&body)
        .unwrap()
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["llama3.1:8b", "gpt-oss:20b"]);
}

#[test]
fn parse_model_list_normalizes_modified_at_to_millis_including_a_non_utc_offset() {
    // Ollama's `modified_at` may carry a non-UTC offset — epoch is
    // offset-independent, so `-07:00` shifts the millis value by +7h vs the
    // same wall-clock time at `Z`.
    let body = json!({
        "models": [{ "name": "llama3.1:8b", "modified_at": "2024-01-01T00:00:00-07:00" }]
    });
    let page = parse_model_list(&body).unwrap();
    assert_eq!(
        page,
        vec![json!({
            "name": "llama3.1:8b",
            "createdAt": 1_704_067_200_000i64 + 7 * 3_600_000,
        })]
    );
}

#[test]
fn parse_model_list_omits_optional_fields_the_provider_does_not_return_and_keeps_name_unchanged() {
    // `name` must stay byte-identical to the pre-widening shape — a stored
    // model preference matches against it. `/api/tags` never returns
    // `displayName`/`contextLength` (context length is only on `/api/show`).
    let body = json!({ "models": [{ "name": "llama3.1:8b" }] });
    let page = parse_model_list(&body).unwrap();
    assert_eq!(page, vec![json!({ "name": "llama3.1:8b" })]);
}

#[test]
fn parse_model_list_ok_empty_on_genuinely_empty_catalogue() {
    let body = json!({ "models": [] });
    assert_eq!(parse_model_list(&body).unwrap(), Vec::<Value>::new());
}

#[test]
fn parse_model_list_errors_when_models_field_is_missing() {
    let body = json!({ "unexpected": "shape" });
    assert!(matches!(
        parse_model_list(&body),
        Err(AppError::Provider(_))
    ));
}

#[test]
fn embedding_only_models_are_recognized_by_name() {
    for name in [
        "qwen3-embedding:8b",
        "nomic-embed-text:latest",
        "mxbai-embed-large",
        "snowflake-arctic-embed2",
        "EMBEDDINGGEMMA:300M", // case-insensitive
    ] {
        assert!(
            is_embedding_only_model(name),
            "{name} must be excluded from chat"
        );
    }
}

#[test]
fn real_chat_models_are_not_mistaken_for_embedding_models() {
    for name in [
        "gemma4:31b",
        "gpt-oss:20b",
        "llama3.1:8b",
        "qwen3:32b",
        "deepseek-v4-flash:0731",
        "mistral-small:24b",
    ] {
        assert!(
            !is_embedding_only_model(name),
            "{name} must stay eligible for chat"
        );
    }
}

#[test]
fn first_chat_model_skips_a_leading_embedding_model() {
    // Exactly the reported shape: the embedding model sorts first in /api/tags.
    let body = json!({
        "models": [
            { "name": "qwen3-embedding:8b" },
            { "name": "gemma4:31b" },
        ]
    });
    assert_eq!(first_chat_model(&body).as_deref(), Some("gemma4:31b"));
}

#[test]
fn first_chat_model_is_none_when_only_embedding_models_are_installed() {
    // Better than returning one anyway: the caller skips translation instead of
    // sending a request the daemon is guaranteed to reject.
    let body = json!({ "models": [{ "name": "qwen3-embedding:8b" }] });
    assert_eq!(first_chat_model(&body), None);
    assert_eq!(first_chat_model(&json!({})), None);
}

#[test]
fn preferred_chat_model_wins_when_listed_else_falls_back_to_first_chat_model() {
    // #1365: the chip named the first listed model, not the one the user runs.
    let body = json!({ "models": [
        { "name": "qwen3.8-4090:latest" }, { "name": "qwen3.8:latest" }, { "name": "nomic-embed-text" }
    ]});
    assert_eq!(
        preferred_chat_model(&body, Some("qwen3.8:latest")).as_deref(),
        Some("qwen3.8:latest")
    );
    // Not listed, an embedding model, or unset: the old first-chat-model pick.
    for p in [Some("gone:1b"), Some("nomic-embed-text"), None] {
        assert_eq!(
            preferred_chat_model(&body, p).as_deref(),
            Some("qwen3.8-4090:latest")
        );
    }
}

#[test]
fn preferred_chat_model_treats_a_bare_name_and_its_latest_tag_as_one_model() {
    let body = json!({ "models": [{ "name": "other:1b" }, { "name": "qwen3:latest" }] });
    assert_eq!(
        preferred_chat_model(&body, Some("qwen3")).as_deref(),
        Some("qwen3:latest")
    );
}
