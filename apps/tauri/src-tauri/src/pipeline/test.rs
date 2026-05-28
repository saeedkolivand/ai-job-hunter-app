use async_trait::async_trait;
use tempfile::TempDir;

use super::cache::KvCache;
use super::{Pipeline, Stage};

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
    async fn run(&self, ctx: &mut Ctx) -> Result<(), String> {
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
    async fn run(&self, ctx: &mut Ctx) -> Result<(), String> {
        ctx.log.push("boom");
        Err("boom".to_string())
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
