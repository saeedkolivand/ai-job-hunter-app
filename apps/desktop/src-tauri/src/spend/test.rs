use super::*;
use tempfile::TempDir;

// ── Pure rate/cost fns ──────────────────────────────────────────────────────

#[test]
fn estimate_cost_matches_known_model_rate() {
    // 1M input + 1M output tokens on gpt-4o-mini == the table's per-1M rates.
    let cost = estimate_cost("gpt-4o-mini", 1_000_000, 1_000_000);
    assert!(
        (cost - (0.15 + 0.60)).abs() < 1e-9,
        "expected 0.75, got {cost}"
    );
}

#[test]
fn estimate_cost_prefers_the_more_specific_prefix() {
    // "gpt-4o-mini" must NOT match the shorter "gpt-4o" prefix's higher rate.
    let mini = estimate_cost("gpt-4o-mini-2024-07-18", 1_000_000, 0);
    let full = estimate_cost("gpt-4o-2024-08-06", 1_000_000, 0);
    assert!(
        (mini - 0.15).abs() < 1e-9,
        "mini should be 0.15, got {mini}"
    );
    assert!(
        (full - 2.50).abs() < 1e-9,
        "full should be 2.50, got {full}"
    );
    assert!(mini < full, "mini must be cheaper than full gpt-4o");
}

#[test]
fn estimate_cost_falls_back_to_default_rate_for_unknown_model() {
    let cost = estimate_cost(
        "some-brand-new-model-nobody-has-heard-of",
        1_000_000,
        1_000_000,
    );
    assert!(
        (cost - (DEFAULT_RATE.0 + DEFAULT_RATE.1)).abs() < 1e-9,
        "unrecognized model must use DEFAULT_RATE, got {cost}"
    );
    assert!(
        cost > 0.0,
        "an unknown PAID model must never silently cost $0"
    );
}

#[test]
fn estimate_cost_zero_tokens_is_zero_cost() {
    assert_eq!(estimate_cost("gpt-4o", 0, 0), 0.0);
}

#[test]
fn estimate_cost_strips_a_leading_models_prefix() {
    // Gemini ids can arrive as "models/gemini-2.5-flash" — must match the
    // same rate as the bare id, not fall through to DEFAULT_RATE.
    let prefixed = estimate_cost("models/gemini-2.5-flash", 1_000_000, 0);
    let bare = estimate_cost("gemini-2.5-flash", 1_000_000, 0);
    assert!((prefixed - bare).abs() < 1e-9);
    assert!(
        (prefixed - 0.30).abs() < 1e-9,
        "expected the gemini-2.5-flash rate, got {prefixed} (looks like DEFAULT_RATE)"
    );
}

#[test]
fn is_free_provider_covers_local_and_cli_agents_only() {
    for p in [
        "ollama",
        "claude-code",
        "codex",
        "gemini-cli",
        "antigravity",
    ] {
        assert!(is_free_provider(p), "{p} must be free");
    }
    for p in [
        "ollama-cloud",
        "openai",
        "anthropic",
        "gemini",
        "openai-compatible",
    ] {
        assert!(!is_free_provider(p), "{p} must NOT be free");
    }
}

#[test]
fn is_localhost_url_recognizes_common_local_forms() {
    for url in [
        "http://localhost:1234/v1",
        "http://127.0.0.1:1234/v1",
        "http://0.0.0.0:8080",
        "https://localhost/v1",
        "localhost:11434",
        "http://[::1]:1234/v1",
    ] {
        assert!(is_localhost_url(url), "{url} should be recognized as local");
    }
    for url in [
        "https://openrouter.ai/api/v1",
        "https://api.groq.com/openai/v1",
        "http://my-localhost-lookalike.example.com/v1",
    ] {
        assert!(!is_localhost_url(url), "{url} must NOT be treated as local");
    }
}

#[test]
fn is_free_call_treats_openai_compatible_localhost_as_free() {
    assert!(is_free_call(
        "openai-compatible",
        Some("http://localhost:1234/v1")
    ));
    assert!(is_free_call(
        "openai-compatible",
        Some("http://127.0.0.1:8080")
    ));
}

#[test]
fn is_free_call_still_charges_openai_compatible_remote_gateways() {
    assert!(!is_free_call(
        "openai-compatible",
        Some("https://openrouter.ai/api/v1")
    ));
    assert!(!is_free_call("openai-compatible", None));
}

#[test]
fn is_free_call_ignores_base_url_for_non_openai_compatible_providers() {
    // A localhost-looking base_url must never make a genuinely paid cloud
    // provider id look free.
    assert!(!is_free_call("openai", Some("http://localhost:1234")));
    assert!(is_free_call("ollama", Some("https://not-actually-checked")));
}

// ── Store round-trip ─────────────────────────────────────────────────────────

fn rec(provider: &str, model: &str, input: u32, output: u32) -> SpendRecord {
    SpendRecord {
        provider: provider.to_string(),
        model: model.to_string(),
        input_tokens: input,
        output_tokens: output,
        run_id: None,
        base_url: None,
    }
}

#[test]
fn record_then_list_round_trips_real_usage_and_computed_cost() {
    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();

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
    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();

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
    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();

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
    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();

    store.record(SpendRecord {
        provider: "openai-compatible".to_string(),
        model: "llama-3.1-8b-instruct".to_string(),
        input_tokens: 5000,
        output_tokens: 2000,
        run_id: None,
        base_url: Some("http://localhost:1234/v1".to_string()),
    });
    // A remote OpenAI-compatible gateway (OpenRouter et al.) still costs money.
    store.record(SpendRecord {
        provider: "openai-compatible".to_string(),
        model: "some-model".to_string(),
        input_tokens: 1000,
        output_tokens: 1000,
        run_id: None,
        base_url: Some("https://openrouter.ai/api/v1".to_string()),
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
    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();
    store.record(rec("openai", "gpt-4o", 100, 100));
    assert_eq!(store.list().len(), 1);

    store.clear_all();
    assert!(store.list().is_empty());
}

#[test]
fn data_store_export_import_round_trips_rows() {
    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();
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
fn data_store_key_is_spend() {
    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();
    assert_eq!(store.key(), "spend");
}
