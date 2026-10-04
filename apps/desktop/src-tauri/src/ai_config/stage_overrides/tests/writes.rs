use super::{support::*, *};

// ── Round-trip ──────────────────────────────────────────────────────────────

/// Mutation check: make `set_stage_override` a no-op and the read is empty;
/// drop `context_window` from the upsert and the last assertion fails.
#[test]
fn a_set_override_round_trips_with_every_field() {
    let (_dir, store) = new_store();
    store
        .set_stage_override(
            "strategy",
            StageOverride {
                provider: "openai-compatible".to_string(),
                model: "big-model".to_string(),
                context_window: Some(32_768),
            },
        )
        .expect("set override");

    let stored = store.stage_override("strategy").expect("row present");
    assert_eq!(stored.provider, "openai-compatible");
    assert_eq!(stored.model, "big-model");
    assert_eq!(stored.context_window, Some(32_768));
    assert_eq!(store.stage_overrides().len(), 1);
}

/// The load-bearing default: a stage nobody configured has NO row, so the
/// resolver falls through to the active provider instead of a guess.
///
/// Mutation check: seed a row for every stage in `open` and this fails.
#[test]
fn an_unset_stage_has_no_override() {
    let (_dir, store) = new_store();
    store
        .set_stage_override("draft", over("ollama", "small"))
        .unwrap();
    assert!(store.stage_override("draft").is_some());
    for stage in PIPELINE_STAGES.iter().filter(|s| **s != "draft") {
        assert!(
            store.stage_override(stage).is_none(),
            "{stage} must stay on the active provider until the user says otherwise",
        );
    }
}

/// Mutation check: make `clear_stage_override` a no-op and this fails.
#[test]
fn clearing_an_override_returns_the_stage_to_the_active_provider() {
    let (_dir, store) = new_store();
    store
        .set_stage_override("repair", over("ollama", "small"))
        .unwrap();
    store.clear_stage_override("repair").unwrap();
    assert!(store.stage_override("repair").is_none());
    // Clearing a stage that has no row is a no-op, not an error — the Settings
    // UI must be able to call it without first reading.
    store.clear_stage_override("repair").unwrap();
}

// ── Write-time validation (the SAME chain as the active config) ─────────────

/// The stage vocabulary is closed. Mutation check: delete the
/// `is_pipeline_stage` guard from `validate_stage_override` and this passes a
/// row nothing will ever read.
#[test]
fn an_unknown_stage_is_rejected() {
    let (_dir, store) = new_store();
    for stage in ["", "rewrite", "Draft", "draft ", "header"] {
        assert!(
            store
                .set_stage_override(stage, over("ollama", "small"))
                .is_err(),
            "{stage:?} is not a pipeline stage and must not be storable",
        );
    }
    assert!(store.stage_overrides().is_empty());
}

/// Mutation check: swap `ProviderId::parse` for a bare `to_string` and this
/// stores a provider nothing can resolve.
#[test]
fn an_unknown_provider_is_rejected() {
    let (_dir, store) = new_store();
    assert!(store
        .set_stage_override("draft", over("not-a-provider", "m"))
        .is_err());
    assert!(store.set_stage_override("draft", over("", "m")).is_err());
}

/// An override with no model is the one thing an override cannot be — unless
/// the provider is a CLI agent, which has its own configured default.
///
/// Mutation check: drop the `None =>` arm's error and an empty-model row
/// persists, failing at generation time instead of at the click.
#[test]
fn an_empty_model_is_rejected_except_for_cli_agents() {
    let (_dir, store) = new_store();
    assert!(store
        .set_stage_override("draft", over("ollama", "  "))
        .is_err());
    store
        .set_stage_override("draft", over("claude-code", ""))
        .expect("a CLI agent runs on its own default model");
    assert_eq!(store.stage_override("draft").unwrap().model, "");
}

/// A per-stage base URL is not merely validated — it is UNREPRESENTABLE, on
/// the wire and in the table. The endpoint follows the named provider's own
/// stored row, so an override can never point at an endpoint Settings does not
/// display.
///
/// The import path is the interesting one: a bundle is untrusted input, and it
/// is the only way a `baseUrl` key can still arrive. Serde drops the unknown
/// field, so the row lands with routing that resolves through the provider's
/// configured URL — a smuggled endpoint has nowhere to be stored.
///
/// Mutation check (executed): re-add a `base_url` field to `StageOverride` and
/// the import assertion below stops proving anything, because the smuggled URL
/// deserializes into it.
#[test]
fn an_import_bundle_cannot_smuggle_a_per_stage_base_url() {
    let (_dir, store) = new_store();
    let bundle = serde_json::json!({
        "providers": {},
        "stageOverrides": {
            "draft": {
                "provider": "openai-compatible",
                "model": "m",
                "baseUrl": "http://169.254.169.254/latest",
            },
        },
    });
    store.import(&bundle).unwrap();

    // The row is accepted on its provider+model, and carries no endpoint of its
    // own — the smuggled cloud-metadata URL is simply not part of the shape.
    let stored = store.stage_override("draft").expect("row present");
    assert_eq!(stored.provider, "openai-compatible");
    let json = serde_json::to_value(&stored).unwrap();
    assert!(
        json.get("baseUrl").is_none(),
        "an override must not carry an endpoint: {json}"
    );

    // And what it WILL resolve through is the provider's own row, which the
    // bundle left unset — not the smuggled value.
    assert_eq!(store.provider_base_url("openai-compatible"), None);
}

/// The endpoint FOLLOWS the provider's settings rather than snapshotting them
/// at write time: there is exactly one base URL per provider, so Settings can
/// never show one endpoint while a stage quietly uses another.
///
/// Mutation check (executed): make `provider_base_url` read a cached/copied
/// value instead of the live row and the post-change assertion fails.
#[test]
fn a_stage_override_follows_the_providers_current_base_url() {
    let (_dir, store) = new_store();
    let point_at = |url: &str| {
        store
            .set_provider_settings(crate::ai_config::ProviderSettingsPatch {
                provider: "openai-compatible".to_string(),
                model: Some(Some("m".to_string())),
                base_url: Some(Some(url.to_string())),
                context_window: None,
            })
            .expect("provider settings");
    };

    point_at("http://127.0.0.1:1234/v1");
    store
        .set_stage_override("draft", over("openai-compatible", "m"))
        .expect("set override");
    assert_eq!(
        store.provider_base_url("openai-compatible").as_deref(),
        Some("http://127.0.0.1:1234/v1")
    );

    // The user moves their local server. The override was never touched…
    point_at("http://127.0.0.1:9999/v1");
    assert_eq!(
        store.provider_base_url("openai-compatible").as_deref(),
        Some("http://127.0.0.1:9999/v1"),
        "the override must follow the provider, not a snapshot"
    );
    assert_eq!(
        store.stage_override("draft").unwrap().provider,
        "openai-compatible"
    );
}

/// `num_ctx` reaches a local inference server, where an absurd value is an
/// out-of-memory kill rather than a wrong answer.
///
/// Mutation check: remove the `validate_context_window` call from
/// `validate_settings` and both of these persist.
#[test]
fn an_out_of_range_context_window_is_rejected() {
    let (_dir, store) = new_store();
    for bad in [1_u32, 511, 131_073, u32::MAX] {
        let mut o = over("ollama", "small");
        o.context_window = Some(bad);
        assert!(
            store.set_stage_override("draft", o).is_err(),
            "{bad} is outside the supported context-window range",
        );
    }
    let mut ok = over("ollama", "small");
    ok.context_window = Some(512);
    store.set_stage_override("draft", ok).unwrap();
}
