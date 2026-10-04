use std::sync::atomic::{AtomicUsize, Ordering};

use tempfile::TempDir;

use super::super::tests::{corpus, entry};
use super::*;
use crate::commands::ai_provider::{EmbeddingSpace, EMBEDDING_VECTOR_VERSION};

// ── Fixtures ─────────────────────────────────────────────────────────────────

fn cfg(provider: &str, model: &str) -> EmbeddingConfig {
    EmbeddingConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        base_url: None,
    }
}

fn vector(cfg: &EmbeddingConfig, values: Vec<f64>) -> EmbeddingVector {
    let dim = values.len();
    EmbeddingVector {
        values,
        space: EmbeddingSpace {
            provider: cfg.provider.clone(),
            model: cfg.model.clone(),
            dim,
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
}

fn store() -> (TempDir, DocumentStore) {
    let dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&dir.path().to_path_buf()).unwrap();
    (dir, store)
}

/// A fresh store plus the embedding config every test below treats as active.
fn fixture() -> (TempDir, DocumentStore, EmbeddingConfig) {
    let (dir, store) = store();
    (dir, store, cfg("ollama", "nomic-embed-text"))
}

/// Pre-seed `help_vectors` for `entries` in `space`, exactly as a previous run
/// would have.
fn seed(
    store: &DocumentStore,
    space: &EmbeddingConfig,
    entries: &[HelpSearchRequestEntry],
    values: &[f64],
) {
    for e in entries {
        let v = vector(space, values.to_vec());
        store.upsert_help_vector(&sha256_hex(&e.body), &v).unwrap();
    }
}

/// An [`Embedder`] whose answer to the i-th call (0 is the query) is
/// `script(i)`; `None` stands in for a failed round-trip.
struct CallScript<F>(AtomicUsize, EmbeddingConfig, F);

#[async_trait]
impl<F: Fn(usize) -> Option<Vec<f64>> + Send + Sync> Embedder for CallScript<F> {
    async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
        let i = self.0.fetch_add(1, Ordering::SeqCst);
        (self.2)(i).map(|values| vector(&self.1, values))
    }
}

/// A scripted [`Embedder`] that counts its round-trips — the seam that makes
/// "how many provider calls did this search make" a test rather than a claim.
///
/// `values` is keyed by call ORDER, not by text: the first call is always the
/// query, so a fixed vector per call index is enough to control the cosine
/// ordering deterministically without re-implementing an embedding model.
struct ScriptedEmbedder {
    calls: AtomicUsize,
    /// One vector per call, in order. A call past the end returns the last.
    values: Vec<Vec<f64>>,
    cfg: EmbeddingConfig,
    /// When true every call fails, standing in for an unreachable provider.
    fails: bool,
}

impl ScriptedEmbedder {
    fn new(cfg: &EmbeddingConfig, values: Vec<Vec<f64>>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            values,
            cfg: cfg.clone(),
            fails: false,
        }
    }

    fn failing(cfg: &EmbeddingConfig) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            values: Vec::new(),
            cfg: cfg.clone(),
            fails: true,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Embedder for ScriptedEmbedder {
    async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
        let i = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fails {
            return None;
        }
        let values = self
            .values
            .get(i)
            .or_else(|| self.values.last())
            .cloned()
            .unwrap_or_else(|| vec![1.0, 0.0]);
        Some(vector(&self.cfg, values))
    }
}

/// `enable_time` because the in-flight-cancel test below needs a timer for
/// both halves of what it asserts: the `tokio::time::timeout` that turns
/// "hangs forever" into a failed assertion, and the sleep that lands the
/// cancel AFTER the embed is already in flight. Harmless for every other test
/// here — none of them arm a timer.
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(f)
}

/// [`run_dense_arm`] on PRODUCTION's own wall-clock budget — the two tests
/// that measure the budget call `run_dense_arm` directly with a short one
/// instead (see [`run_dense_arm`]'s own doc for why it is a parameter).
fn arm_with<E: Embedder + ?Sized>(
    store: &DocumentStore,
    active: &EmbeddingConfig,
    embedder: &E,
    query: &str,
    entries: &[HelpSearchRequestEntry],
    token: &CancellationToken,
) -> (Vec<String>, ArmStatus) {
    block_on(run_dense_arm(
        store,
        active,
        embedder,
        query,
        entries,
        crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
        token,
    ))
}

/// [`arm_with`] on an unregistered token nobody can fire — what a caller that
/// sends no `queryId` gets. The cancellation tests below pass their own.
fn arm<E: Embedder + ?Sized>(
    store: &DocumentStore,
    active: &EmbeddingConfig,
    embedder: &E,
    query: &str,
    entries: &[HelpSearchRequestEntry],
) -> (Vec<String>, ArmStatus) {
    arm_with(
        store,
        active,
        embedder,
        query,
        entries,
        &CancellationToken::new(),
    )
}

/// An [`Embedder`] that CANCELS its own token after `cancel_after` calls —
/// the only way to observe "a cancel arriving mid-arm" deterministically,
/// since a real one arrives from another task at an unpredictable moment.
/// Counts calls like [`ScriptedEmbedder`] does.
struct CancellingEmbedder {
    calls: AtomicUsize,
    cfg: EmbeddingConfig,
    token: CancellationToken,
    cancel_after: usize,
}

#[async_trait]
impl Embedder for CancellingEmbedder {
    async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= self.cancel_after {
            self.token.cancel();
        }
        Some(vector(&self.cfg, vec![1.0, 0.0]))
    }
}

/// An [`Embedder`] whose every round-trip takes `delay` — a slow provider,
/// without a slow test: the wall-clock budget is injected, not waited out.
struct SlowEmbedder {
    calls: AtomicUsize,
    cfg: EmbeddingConfig,
    delay: std::time::Duration,
}

#[async_trait]
impl Embedder for SlowEmbedder {
    async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        // `std::thread::sleep`, not `tokio::time::sleep`: the bound this fake
        // exercises is `std::time::Instant::elapsed`, which only real time
        // advances, and blocking IS what a slow embed does to this arm anyway.
        std::thread::sleep(self.delay);
        Some(vector(&self.cfg, vec![1.0, 0.0]))
    }
}

mod cache;
mod cancellation;
