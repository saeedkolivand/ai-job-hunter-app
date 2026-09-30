//! `normalize_show` — `/api/show` response normalization.

use serde_json::json;

use super::super::inspect::normalize_show;

#[test]
fn normalize_extracts_context_and_details_by_architecture() {
    // `context_length` is keyed by architecture — scan for the suffix, not a
    // hardcoded `llama.` prefix, so qwen2/phi3/etc. all work unchanged.
    let data = json!({
        "model_info": { "qwen2.context_length": 32768, "qwen2.embedding_length": 3584 },
        "details": { "parameter_size": "7.6B", "quantization_level": "Q4_K_M", "family": "qwen2" }
    });
    let out = normalize_show(&data);
    assert_eq!(out["contextLength"], json!(32768));
    assert_eq!(out["parameterSize"], json!("7.6B"));
    assert_eq!(out["quantization"], json!("Q4_K_M"));
    assert_eq!(out["family"], json!("qwen2"));
}

#[test]
fn normalize_omits_missing_fields() {
    let data = json!({
        "model_info": { "llama.context_length": 8192 },
        "details": { "parameter_size": "8B" }
    });
    let out = normalize_show(&data);
    assert_eq!(out["contextLength"], json!(8192));
    assert_eq!(out["parameterSize"], json!("8B"));
    // Absent fields are omitted (not null), so the TS optional schema accepts it.
    assert!(out.get("quantization").is_none());
    assert!(out.get("family").is_none());
}

#[test]
fn normalize_returns_null_when_nothing_usable() {
    assert!(normalize_show(&json!({})).is_null());
    assert!(normalize_show(&json!({ "model_info": {}, "details": {} })).is_null());
}
