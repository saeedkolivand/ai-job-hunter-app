use super::{support::*, *};

// ── Store round-trip ─────────────────────────────────────────────────────────

#[test]
fn record_then_list_round_trips_real_usage_and_computed_cost() {
    let (_dir, store) = open_store();

    store.record(rec("openai", "gpt-4o-mini", 1000, 500));

    let rows = store.list();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].provider, "openai");
    assert_eq!(rows[0].model, "gpt-4o-mini");
    assert_eq!(rows[0].input_tokens, 1000);
    assert_eq!(rows[0].output_tokens, 500);
    // (1000/1e6)*0.15 + (500/1e6)*0.60 == 0.00015 + 0.0003 == 0.00045
    assert!((rows[0].est_cost_usd - 0.00045).abs() < 1e-9);
}

#[test]
fn record_zeroes_cost_for_local_and_cli_agent_providers_despite_real_tokens() {
    let (_dir, store) = open_store();

    // Ollama genuinely reports nonzero token counts, but has no metered API —
    // the estimated cost must stay $0.
    store.record(rec("ollama", "llama3.1:8b", 5000, 2000));
    store.record(rec("claude-code", "sonnet", 3000, 1000));

    let totals = store.today_totals();
    assert_eq!(totals.input_tokens, 8000, "real tokens are still recorded");
    assert_eq!(totals.output_tokens, 3000);
    assert_eq!(totals.est_cost_usd, 0.0, "local/CLI-agent calls cost $0");
}

#[test]
fn today_totals_and_by_provider_today_aggregate_correctly() {
    let (_dir, store) = open_store();

    store.record(rec("openai", "gpt-4o-mini", 1000, 1000));
    store.record(rec("openai", "gpt-4o-mini", 1000, 1000));
    store.record(rec("anthropic", "claude-3-5-sonnet-20241022", 2000, 2000));

    let totals = store.today_totals();
    assert_eq!(totals.input_tokens, 4000);
    assert_eq!(totals.output_tokens, 4000);
    assert!(totals.est_cost_usd > 0.0);

    let per_provider = store.by_provider_today();
    assert_eq!(per_provider.len(), 2);
    let openai = per_provider
        .iter()
        .find(|p| p.provider == "openai")
        .unwrap();
    assert_eq!(openai.input_tokens, 2000);
    assert_eq!(openai.output_tokens, 2000);
    let anthropic = per_provider
        .iter()
        .find(|p| p.provider == "anthropic")
        .unwrap();
    assert_eq!(anthropic.input_tokens, 2000);
    assert_eq!(anthropic.output_tokens, 2000);
}

#[test]
fn record_zeroes_cost_for_openai_compatible_localhost_despite_real_tokens() {
    let (_dir, store) = open_store();

    store.record(SpendRecord {
        base_url: Some("http://localhost:1234/v1".to_string()),
        ..rec("openai-compatible", "llama-3.1-8b-instruct", 5000, 2000)
    });
    // A remote OpenAI-compatible gateway (OpenRouter et al.) still costs money.
    store.record(SpendRecord {
        base_url: Some("https://openrouter.ai/api/v1".to_string()),
        ..rec("openai-compatible", "some-model", 1000, 1000)
    });

    let per_provider = store.by_provider_today();
    assert_eq!(per_provider.len(), 1, "both rows share the provider id");
    let row = &per_provider[0];
    assert_eq!(row.input_tokens, 6000, "real tokens still recorded");
    assert!(
        row.est_cost_usd > 0.0,
        "the remote-gateway row must still cost something"
    );
}

#[test]
fn clear_all_empties_the_store() {
    let (_dir, store) = open_store();
    store.record(rec("openai", "gpt-4o", 100, 100));
    assert_eq!(store.list().len(), 1);

    store.clear_all();
    assert!(store.list().is_empty());
}

#[test]
fn data_store_export_import_round_trips_rows() {
    let (_dir, store) = open_store();
    store.record(rec("gemini", "gemini-2.5-flash", 400, 200));

    let exported = store.export();
    store.clear_all();
    assert!(
        store.list().is_empty(),
        "precondition: cleared before import"
    );

    let imported = store.import(&exported).unwrap();
    assert_eq!(imported, 1);
    let rows = store.list();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].provider, "gemini");
    assert_eq!(rows[0].model, "gemini-2.5-flash");
    assert_eq!(rows[0].input_tokens, 400);
    assert_eq!(rows[0].output_tokens, 200);
}

#[test]
fn import_rejects_non_array_json_and_leaves_the_store_untouched() {
    let (_dir, store) = open_store();
    store.record(rec("openai", "gpt-4o", 100, 50));
    assert_eq!(store.list().len(), 1, "precondition: one row present");

    let result = store.import(&serde_json::json!({ "not": "an array" }));
    assert!(result.is_err(), "non-array JSON must be rejected");

    // A malformed/incorrectly-shaped bundle must never clear existing data —
    // factory-restore honesty depends on this being atomic, not partial.
    assert_eq!(
        store.list().len(),
        1,
        "a rejected import must leave prior rows intact"
    );
}

#[test]
fn import_rejects_a_row_that_fails_to_deserialize_and_leaves_the_store_untouched() {
    let (_dir, store) = open_store();
    store.record(rec("openai", "gpt-4o", 100, 50));
    assert_eq!(store.list().len(), 1, "precondition: one row present");

    // One well-formed row followed by one missing the required `id` field —
    // the whole import must abort, not partially apply the good row.
    let bundle = serde_json::json!([
        {
            "id": "spend-1",
            "createdAt": 1_000_u64,
            "provider": "gemini",
            "model": "gemini-2.5-flash",
            "inputTokens": 10,
            "outputTokens": 5,
            "estCostUsd": 0.001,
        },
        {
            "createdAt": 2_000_u64,
            "provider": "openai",
            "model": "gpt-4o",
            "inputTokens": 10,
            "outputTokens": 5,
            "estCostUsd": 0.001,
        }
    ]);
    let result = store.import(&bundle);
    assert!(
        result.is_err(),
        "a row failing to deserialize must error, not panic or silently succeed"
    );

    let rows = store.list();
    assert_eq!(rows.len(), 1, "the pre-existing row must survive intact");
    assert_eq!(rows[0].provider, "openai");
    assert_eq!(
        rows[0].model, "gpt-4o",
        "the aborted import's rows must never partially land"
    );
}

#[test]
fn data_store_key_is_spend() {
    let (_dir, store) = open_store();
    assert_eq!(store.key(), "spend");
}
