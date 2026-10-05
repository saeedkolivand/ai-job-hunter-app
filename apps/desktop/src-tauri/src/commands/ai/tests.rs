use tempfile::TempDir;

use super::*;
use crate::error::AppError;

// ── embedding_base_url_tests ──────────────────────────────────────────
// `ai_set_embedding_config` needs a live `AppHandle` this crate has no
// test harness for (see the same note on `AnswerSearcher` above), so its
// validation logic is pinned via the extracted pure
// `scrub_and_validate_embedding_base_url` — mirrors
// `ai_provider::tests`'s `resolve_by_name_*` base_url tests, which cover
// the same rule on the sibling probe path. "Persisted" is covered at the
// store level (`DocumentStore::set_embedding_config`), same idiom as
// `documents::tests`'s command-layer notes.

#[test]
fn rejects_the_cloud_metadata_ip_on_openai_compatible() {
    let err = scrub_and_validate_embedding_base_url(
        ProviderId::OpenAiCompatible,
        Some("http://169.254.169.254/latest/meta-data".to_string()),
    )
    .expect_err("the cloud-metadata IP literal must be rejected");
    assert!(matches!(err, AppError::Validation(_)));
}

#[test]
fn accepts_a_local_lm_studio_style_base_url() {
    // Sanity: the guard must not break the ordinary local-endpoint case
    // it exists to protect around (LM Studio / vLLM / Ollama).
    let url = scrub_and_validate_embedding_base_url(
        ProviderId::OpenAiCompatible,
        Some("http://localhost:1234/v1".to_string()),
    )
    .expect("a local endpoint must still be accepted");
    assert_eq!(url.as_deref(), Some("http://localhost:1234/v1"));
}

#[test]
fn accepted_local_base_url_round_trips_through_the_store() {
    // "Persisted", not just "accepted": the scrubbed/validated value must
    // survive a real `DocumentStore` write + read unchanged.
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let base_url = scrub_and_validate_embedding_base_url(
        ProviderId::OpenAiCompatible,
        Some("http://localhost:1234/v1".to_string()),
    )
    .unwrap();
    let cfg = EmbeddingConfig {
        provider: ProviderId::OpenAiCompatible.as_str().to_string(),
        model: "local-embed".to_string(),
        base_url,
    };
    store.set_embedding_config(&cfg).unwrap();
    assert_eq!(
        store.embedding_config().base_url.as_deref(),
        Some("http://localhost:1234/v1")
    );
}

#[test]
fn drops_base_url_for_a_non_openai_compatible_provider_instead_of_erroring() {
    // Mirrors `AiConfigStore::validate_settings`'s scrub: `base_url` is
    // inert for egress on every provider except `OpenAiCompatible`, so a
    // bogus value (here, one that WOULD fail validation if checked) must
    // be silently dropped, not surfaced as an error.
    let url = scrub_and_validate_embedding_base_url(
        ProviderId::Gemini,
        Some("http://169.254.169.254/latest/meta-data".to_string()),
    )
    .expect("a non-OpenAiCompatible provider must never error on base_url");
    assert_eq!(url, None);
}

// AI-spend visibility (`ai_spend_summary` and its pure helpers, #1161) moved
// to `commands::ai::spend`'s own `#[cfg(test)] mod test` alongside the code
// it covers — see that module for `spend_totals_json`, `spend_summary_value`,
// `spend_summary_from_store`, `resolve_window_days`, and
// `per_provider_with_zero_rows` coverage.
