//! `Completer` resolution: `low_effort_level`'s pure filter, and the
//! `AppHandle`-free `from_override`/`from_config` egress checks (base_url
//! SSRF re-validate, context-window bound re-validate). Plus
//! `Completer::admit_research` (the shared `"ai_research"` admission
//! bucket).

use crate::ai_config::ActiveAiConfig;
use crate::pipeline::{low_effort_level, Completer};

/// `Completer::low_effort`'s pure half. The lists themselves — and the
/// "entry 0 is the minimum" invariant `.first()` rests on — are pinned
/// against the REAL adapters in `commands::ai_provider::tests`; what belongs
/// here is the empty case (the one every provider without an effort lever
/// takes) and the CHEAP-tier filter on top of entry 0.
///
/// Mutation check (executed): return `levels.last().copied()` and the
/// three-tier case fails; return `Some("low")` for an empty list and the
/// first assertion fails; drop the `minimal`/`low` filter and the
/// `["high"]`/`["medium", …]` cases fail.
#[test]
fn the_low_effort_level_is_entry_zero_and_only_when_entry_zero_is_cheap() {
    assert_eq!(low_effort_level(&[]), None);
    assert_eq!(low_effort_level(&["low", "medium", "high"]), Some("low"));
    assert_eq!(low_effort_level(&["minimal", "low"]), Some("minimal"));

    // Entry 0 is this list's minimum, and it is still NOT cheap: the caller
    // wants "spend less of the output budget on thinking", so an expensive
    // lowest tier means there is nothing worth asking for — send nothing.
    assert_eq!(low_effort_level(&["high"]), None);
    assert_eq!(low_effort_level(&["medium", "high"]), None);
}

/// The stage override's resolve seam takes the SAME steps as the active
/// config's, in the same order — a per-stage row must not reach an endpoint the
/// active config would have refused.
///
/// The URL is no longer carried BY the override (it is the provider's own
/// stored one, passed in), but it is still store-supplied, so it still takes
/// the egress check: a tampered `ai_provider_config` row is the same threat
/// model as a tampered override row was.
///
/// Mutation check (executed): drop the `validate_provider_base_url` call from
/// `from_override` and the first case resolves.
#[test]
fn a_stage_override_is_validated_exactly_like_the_active_config() {
    use crate::ai_config::StageOverride;

    let over = |provider: &str, model: &str| StageOverride {
        provider: provider.to_string(),
        model: model.to_string(),
        context_window: None,
    };
    let with_window = |context_window: Option<u32>| StageOverride {
        provider: "ollama".to_string(),
        model: "m".to_string(),
        context_window,
    };

    let err = Completer::from_override(
        over("openai-compatible", "m"),
        Some("http://169.254.169.254/latest/meta-data/".to_string()),
    )
    .map(|_| ())
    .unwrap_err();
    assert!(
        format!("{err}").to_lowercase().contains("metadata"),
        "got {err}"
    );

    let err = Completer::from_override(over("anthropic", ""), None)
        .map(|_| ())
        .unwrap_err();
    assert!(format!("{err}").contains("No model selected"), "got {err}");

    // A stored context window is re-validated on the way OUT too — the same
    // hand-edited-store threat model as the base_url above, and the one stored
    // number whose absurd value is an OOM rather than a wrong answer.
    //
    // Matched on the MESSAGE, not merely `is_err`: "ollama"/"m" could also be
    // refused by `validate_model`, and today only the call order makes a bare
    // `is_err` sound. A reordering, or a tightened model rule, would keep both
    // assertions green while they stopped guarding the window at all.
    //
    // Mutation check (executed): drop the `validate_context_window` call from
    // `from_override` and both of these resolve.
    for bad in [9_999_999, 1] {
        let err = Completer::from_override(with_window(Some(bad)), None)
            .map(|_| ())
            .unwrap_err();
        assert!(
            format!("{err}").contains("context window"),
            "{bad} must be refused BY THE WINDOW CHECK, got {err}"
        );
    }

    // The endpoint the caller read off the PROVIDER's row is what the resolved
    // completer routes to — the override contributes provider + model only.
    let (_provider, model, base_url, context_window) = Completer::from_override(
        over("openai-compatible", "local-model"),
        Some("http://127.0.0.1:1234/v1".to_string()),
    )
    .expect("a good override resolves");
    assert_eq!(model, "local-model");
    assert_eq!(base_url.as_deref(), Some("http://127.0.0.1:1234/v1"));
    assert_eq!(context_window, None);

    let (_provider, _model, _base_url, context_window) =
        Completer::from_override(with_window(Some(8_192)), None)
            .expect("an in-range window resolves");
    assert_eq!(context_window, Some(8_192));
}

// ── Completer::from_config ────────────────────────────────────────────────────
//
// `from_config` is the `AppHandle`-free seam behind `Completer::from_active`'s
// store-driven resolve (see its doc comment) — no `tauri::test` mock app needed.
// These build an owned `ActiveAiConfig` directly, the same shape
// `AiConfigStore::active_config()` returns.

fn active_cfg(
    provider: Option<&str>,
    model: Option<&str>,
    base_url: Option<&str>,
) -> ActiveAiConfig {
    ActiveAiConfig {
        active_provider: provider.map(str::to_string),
        model: model.map(str::to_string),
        base_url: base_url.map(str::to_string),
        context_window: None,
        providers: Default::default(),
    }
}

#[test]
fn rejects_tampered_cloud_metadata_base_url() {
    // A metadata-endpoint base_url could only land here via a store row written
    // directly to SQLite (the writer/seed/import all reject it) — the defensive
    // re-validate must fail closed, never fall back to a default endpoint.
    let cfg = active_cfg(
        Some("openai-compatible"),
        Some("local-model"),
        Some("http://169.254.169.254/latest/meta-data/"),
    );
    let err = Completer::from_config(cfg).map(|_| ()).unwrap_err();
    assert!(
        format!("{err}").to_lowercase().contains("metadata"),
        "got {err}"
    );
}

#[test]
fn rejects_tampered_non_http_base_url_scheme() {
    let cfg = active_cfg(
        Some("openai-compatible"),
        Some("local-model"),
        Some("ftp://evil.test/v1"),
    );
    let err = Completer::from_config(cfg).map(|_| ()).unwrap_err();
    assert!(
        format!("{err}").to_lowercase().contains("scheme"),
        "got {err}"
    );
}

#[test]
fn resolves_openai_compatible_with_a_localhost_base_url() {
    let cfg = active_cfg(
        Some("openai-compatible"),
        Some("local-model"),
        Some("http://127.0.0.1:1234/v1"),
    );
    let (_provider, model, base_url, _window) =
        Completer::from_config(cfg).expect("should resolve");
    assert_eq!(model, "local-model");
    assert_eq!(base_url.as_deref(), Some("http://127.0.0.1:1234/v1"));
}

/// The stored context window is re-validated on the ACTIVE-CONFIG egress path,
/// exactly as it is for a stage override — the same hand-edited-store threat
/// model, and the same fail-closed answer. Without this the two paths could
/// drift, which is how the override path came to be the only guarded one.
///
/// Mutation check (executed): replace the `validate_context_window` call in
/// `from_config` with `cfg.context_window` and both rejections resolve.
#[test]
fn rejects_a_tampered_active_context_window() {
    let with_window = |context_window: Option<u32>| ActiveAiConfig {
        active_provider: Some("ollama".to_string()),
        model: Some("m".to_string()),
        base_url: None,
        context_window,
        providers: Default::default(),
    };

    // Absurdly large (the OOM case) and absurdly small (a window that cannot
    // hold a prompt) are both refused rather than clamped — and refused BY THE
    // WINDOW CHECK, which only the message proves (see the sibling override
    // test for why `is_err` alone is not enough).
    for bad in [9_999_999, 1] {
        let err = Completer::from_config(with_window(Some(bad)))
            .map(|_| ())
            .unwrap_err();
        assert!(
            format!("{err}").contains("context window"),
            "{bad} must be refused by the window check, got {err}"
        );
    }

    // An in-range window survives the gate and is what the completer carries.
    let (_provider, _model, _base_url, context_window) =
        Completer::from_config(with_window(Some(8_192))).expect("an in-range window resolves");
    assert_eq!(context_window, Some(8_192));
    // No stored window stays None — the provider default, not a substituted one.
    let (_provider, _model, _base_url, context_window) =
        Completer::from_config(with_window(None)).expect("no window resolves");
    assert_eq!(context_window, None);
}

#[test]
fn unseeded_provider_is_the_no_provider_error() {
    let cfg = active_cfg(None, None, None);
    let err = Completer::from_config(cfg).map(|_| ()).unwrap_err();
    assert!(
        format!("{err}").contains("No AI provider selected"),
        "got {err}"
    );
}

#[test]
fn empty_model_on_a_non_cli_provider_is_the_no_model_error() {
    let cfg = active_cfg(Some("anthropic"), None, None);
    let err = Completer::from_config(cfg).map(|_| ()).unwrap_err();
    assert!(format!("{err}").contains("No model selected"), "got {err}");
}

#[test]
fn a_good_native_provider_resolves_and_ignores_base_url() {
    // `base_url` is only ever wired into the boxed client for `OpenAiCompatible`
    // (see `commands::ai_provider::resolve`) — a native provider config still
    // passes the re-validate (it applies regardless of provider) but the value
    // plays no further part in what gets constructed.
    let cfg = active_cfg(
        Some("anthropic"),
        Some("claude-3-5-sonnet"),
        Some("https://example.com"),
    );
    let (_provider, model, _base_url, _window) =
        Completer::from_config(cfg).expect("should resolve");
    assert_eq!(model, "claude-3-5-sonnet");
}

// ── Completer::admit_research (shared "ai_research" bucket) ────────────────

/// `Completer::admit_research` is the résumé pipeline's ONLY way to reach the
/// `"ai_research"` bucket — the same one `commands::ai::admit_research` gates
/// `ai_research_company`/`ai_lookup_salary`/`ai_research_answer` behind (see
/// that method's own doc for why it exists rather than the pipeline reaching
/// the `pub(super)` command function directly). `Completer` needs a live
/// `AppHandle` this crate has no harness for, so — same shape as
/// `commands::resume_pipeline::test`'s
/// `the_regenerate_section_bucket_refuses_a_caller_past_its_concurrency_cap` —
/// the assertion is on the exact bucket/constants the method's source uses,
/// against a real bare `Limiter`.
///
/// Mutation check: raise `AI_RESEARCH_CONCURRENCY_MAX` and this still passes
/// (the loop bound is derived from that same constant, deliberately, so it
/// pins the MECHANISM, not the number); make `Limiter::acquire` return `Ok`
/// on a full gate and the refusal assertion fails; leak the guard's permit
/// and the re-open assertion does.
#[test]
fn the_ai_research_bucket_refuses_a_caller_past_its_concurrency_cap() {
    let limiter = std::sync::Arc::new(crate::limits::Limiter::default());
    let held: Vec<_> = (0..crate::limits::AI_RESEARCH_CONCURRENCY_MAX)
        .map(|index| {
            limiter
                .acquire(
                    crate::limits::AI_RESEARCH_BUCKET,
                    crate::limits::AI_RESEARCH_RATE_MAX,
                    crate::limits::AI_RESEARCH_CONCURRENCY_MAX,
                )
                .unwrap_or_else(|e| panic!("slot {index} must be admitted: {e}"))
        })
        .collect();

    let refused = limiter.acquire(
        crate::limits::AI_RESEARCH_BUCKET,
        crate::limits::AI_RESEARCH_RATE_MAX,
        crate::limits::AI_RESEARCH_CONCURRENCY_MAX,
    );
    assert!(
        matches!(refused, Err(crate::error::AppError::RateLimited(_))),
        "the shared bucket must refuse a caller past its concurrency cap"
    );
    drop(held);
    assert!(
        limiter
            .acquire(
                crate::limits::AI_RESEARCH_BUCKET,
                crate::limits::AI_RESEARCH_RATE_MAX,
                crate::limits::AI_RESEARCH_CONCURRENCY_MAX,
            )
            .is_ok(),
        "the guard is RAII — releasing it must re-open the slot"
    );
}

/// The source-level half of the lock above, PLUS the "one bucket, not two"
/// guarantee: `commands::ai::admit_research` (the three research commands'
/// admission) and `Completer::admit_research` (the résumé pipeline's
/// `cover_letter` research) must both acquire `crate::limits::AI_RESEARCH_BUCKET`
/// — never a second, independently-spelled bucket name, which is exactly the
/// drift that would reopen the unbounded-spend hole the shared bucket exists
/// to close. `Completer` needs a live `AppHandle` to actually run, so this is
/// provable only by reading the source, the same reason
/// `commands::resume_pipeline::test`'s
/// `every_provider_calling_command_admits_before_it_spends` is grep-shaped.
///
/// Mutation check: replace either site's constant with a literal
/// `"ai_research"` string (still functionally the same bucket, but no longer
/// PROVABLY the same one without reading both files) and this fails.
#[test]
fn the_pipeline_and_the_command_admit_against_the_same_named_bucket_constant() {
    let pipeline_source = include_str!("../completer.rs");
    assert!(
        pipeline_source.contains("fn admit_research(&self, who: &str)"),
        "Completer::admit_research must exist"
    );
    assert!(
        pipeline_source.contains("crate::limits::AI_RESEARCH_BUCKET"),
        "Completer::admit_research must acquire the SHARED bucket constant, not a literal"
    );

    let command_source = include_str!("../../commands/ai/research.rs");
    assert!(
        command_source.contains("crate::limits::AI_RESEARCH_BUCKET"),
        "commands::ai::admit_research must acquire the SAME shared bucket constant"
    );
}
