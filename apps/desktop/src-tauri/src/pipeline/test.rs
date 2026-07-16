use async_trait::async_trait;
use tempfile::TempDir;

use super::cache::KvCache;
use super::{Completer, Pipeline, Stage};
use crate::ai_config::ActiveAiConfig;
use crate::error::{AppError, AppResult};

// ── Pipeline ordering / abort ───────────────────────────────────────────────────

struct Ctx {
    log: Vec<&'static str>,
}

struct Step(&'static str);
#[async_trait]
impl Stage<Ctx> for Step {
    fn name(&self) -> &'static str {
        self.0
    }
    async fn run(&self, ctx: &mut Ctx) -> AppResult<()> {
        ctx.log.push(self.0);
        Ok(())
    }
}

struct Boom;
#[async_trait]
impl Stage<Ctx> for Boom {
    fn name(&self) -> &'static str {
        "boom"
    }
    async fn run(&self, ctx: &mut Ctx) -> AppResult<()> {
        ctx.log.push("boom");
        Err(AppError::Message("boom".to_string()))
    }
}

#[test]
fn pipeline_runs_stages_in_order() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        Pipeline::new("t")
            .add(Step("a"))
            .add(Step("b"))
            .add(Step("c"))
            .run(&mut ctx)
            .await
            .unwrap();
        assert_eq!(ctx.log, vec!["a", "b", "c"]);
    });
}

#[test]
fn pipeline_aborts_on_first_error() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        let res = Pipeline::new("t")
            .add(Step("a"))
            .add(Boom)
            .add(Step("c"))
            .run(&mut ctx)
            .await;
        assert!(res.is_err());
        // "c" must not run after the failing stage.
        assert_eq!(ctx.log, vec!["a", "boom"]);
    });
}

// ── KvCache ──────────────────────────────────────────────────────────────────────

#[test]
fn kv_cache_roundtrip_ttl_and_namespace_isolation() {
    let dir = TempDir::new().unwrap();
    let cache = KvCache::open(dir.path()).unwrap();

    cache.set("ns1", "acme", "brief-v1");
    assert_eq!(cache.get("ns1", "acme", 3600), Some("brief-v1".to_string()));

    // Different namespace, same key → miss.
    assert_eq!(cache.get("ns2", "acme", 3600), None);

    // ttl = 0 → the entry is considered expired immediately.
    assert_eq!(cache.get("ns1", "acme", 0), None);

    // Overwrite.
    cache.set("ns1", "acme", "brief-v2");
    assert_eq!(cache.get("ns1", "acme", 3600), Some("brief-v2".to_string()));

    // Key match is case-insensitive (COLLATE NOCASE).
    assert_eq!(cache.get("ns1", "ACME", 3600), Some("brief-v2".to_string()));
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
    let (_provider, model, base_url) = Completer::from_config(cfg).expect("should resolve");
    assert_eq!(model, "local-model");
    assert_eq!(base_url.as_deref(), Some("http://127.0.0.1:1234/v1"));
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
    let (_provider, model, _base_url) = Completer::from_config(cfg).expect("should resolve");
    assert_eq!(model, "claude-3-5-sonnet");
}
