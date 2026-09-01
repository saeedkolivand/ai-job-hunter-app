//! Embedding round-trips + the posting-vector cache resolver.
//!
//! Split out of `documents/mod.rs` (R8's hard LOC cap) — a self-contained
//! concern: routes through the centralized provider layer using the
//! persisted embedding config, so embeddings are provider-aware (Ollama /
//! OpenAI / Gemini) and every vector is tagged with the space that produced
//! it. No provider/endpoint strings live here. Every item below is
//! re-exported at `documents::` (see the `mod embedding;` line in
//! `documents/mod.rs`) so this split is invisible to every existing
//! `crate::documents::X` call site.

use tauri::{AppHandle, Manager};

use super::{DocumentStore, EmbeddingConfig};
use crate::commands::ai_provider::{EmbeddingVector, ProviderId};
use crate::error::AppResult;
use crate::observability::sanitize_reason;

pub async fn embed(app: &AppHandle, text: &str) -> AppResult<EmbeddingVector> {
    let cfg = app.state::<DocumentStore>().embedding_config();
    embed_with_config(app, &cfg, text, None).await
}

/// [`embed`], but the embedding config is resolved by the CALLER (`cfg`)
/// instead of being re-read from the store here, and an optional `charge`
/// is threaded through to [`crate::commands::ai_provider::embed_text`] —
/// see its doc comment: it fires once per ACTUAL provider round-trip, not
/// once per call.
///
/// `ai_embed` is the only caller that passes both: it reads
/// `embedding_config()` exactly ONCE and hands that SAME snapshot to both
/// the charge (which provider's daily budget) and this function (which
/// provider actually receives the request) — reading the config a second
/// time here, as [`embed`] does, would let `ai_set_embedding_config` land in
/// between and charge one provider while dispatching to another (#1087
/// finding 2). Every other caller of [`embed`] (`AppEmbedder` — bulk
/// re-index + match-score resolution) is unaffected: it keeps re-reading
/// the store fresh via [`embed`] and passes `charge: None`.
pub(crate) async fn embed_with_config(
    app: &AppHandle,
    cfg: &EmbeddingConfig,
    text: &str,
    charge: Option<&(dyn Fn() -> AppResult<()> + Send + Sync)>,
) -> AppResult<EmbeddingVector> {
    let provider = ProviderId::parse(&cfg.provider).map_err(|e| {
        tracing::warn!(
            "embed failed: unknown embedding provider '{}': {e}",
            cfg.provider
        );
        e
    })?;
    let result = crate::commands::ai_provider::embed_text(
        app,
        provider,
        &cfg.model,
        cfg.base_url.clone(),
        text,
        charge,
    )
    .await;
    // The model, not just the provider — answering "which embedding model
    // actually ran?" used to require watching Ollama's API from outside.
    match &result {
        Ok(_) => tracing::debug!(provider = %cfg.provider, model = %cfg.model, "embed ok"),
        // `e` can be `AppError::Network` wrapping a raw `reqwest::Error` —
        // Gemini authenticates via a `?key=` query param, so an unsanitized
        // transport-failure message here could echo it straight into the log.
        Err(e) => tracing::warn!(
            provider = %cfg.provider,
            model = %cfg.model,
            "embed failed: {}",
            sanitize_reason(&e.to_string())
        ),
    }
    result
}

/// Lowercase-hex SHA-256 of `text`. Deterministic and stable across process
/// restarts (unlike `DefaultHasher`/`RandomState`), so it is safe as the
/// cross-session cache guard for both `posting_vectors.text_hash` and
/// `match_scores.job_text_hash`. Single source of the hash for both caches.
pub(crate) fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    h.finalize()
        .iter()
        .fold(String::with_capacity(64), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

/// Whether `id` belongs to a synthetic, content-addressed SCORING identity
/// (`adhoc:…` from the extension bridge, `autopilot:…` / `autopilot-resume:…`
/// from the headless re-rank) rather than a real `documents` row.
///
/// The app-wide convention for such an id is a `<namespace>:` prefix; a real
/// document id is `doc-<millis>-<uuid8>` (see `make_doc_id`) and never
/// contains a colon. Used by `upsert_vector_with_conn` to keep the DOCUMENT
/// index free of rows that have no document: nothing would ever delete them
/// (document delete and re-embed both iterate real documents; `prune_caches`
/// only touches `posting_vectors`/`match_scores`), and
/// `DocumentStore::count_vectors_in_space` counts every row — so one orphan
/// makes the Embeddings panel report "N/N indexed, stale 0" over an index that
/// is genuinely stale.
pub(crate) fn is_synthetic_scoring_id(id: &str) -> bool {
    id.contains(':')
}

/// Cache-precedence predicate for the posting-vector cache: a cached row is a
/// HIT iff its embedding space matches the `active` config AND it was stored for
/// the exact text we're requesting (`requested_hash == cached.text_hash`). A
/// `None` row (no cached vector) is always a miss. This is the single source of
/// the resolver's cache-hit decision — [`posting_vector_or_embed`] calls it so a
/// reverted/loosened check fails a unit test (see documents/test.rs).
pub(crate) fn posting_vector_is_fresh(
    active: &EmbeddingConfig,
    requested_hash: &str,
    cached: Option<&(EmbeddingVector, String)>,
) -> bool {
    match cached {
        Some((v, stored_hash)) => active.matches(&v.space) && stored_hash == requested_hash,
        None => false,
    }
}

/// ONE embedding round-trip, behind a seam.
///
/// The scoring kernel's only reach into the provider layer. It is a trait (not
/// a bare `AppHandle` call) because this crate has no `tauri::test` mock-app
/// harness: with the round-trip behind a seam, "how many provider calls did
/// this score make, on what bytes, and what did each one cost" is a plain unit
/// test over a real [`DocumentStore`] instead of a hand-retyped mirror of the
/// kernel's own cache logic — which is exactly the shape that let an
/// untranslated-hash charge predicate pass review.
/// `pub(crate)`, not `pub`: the choke point is only a guarantee while every
/// implementor is in this crate and reachable from `embed_charged`.
#[async_trait::async_trait]
pub(crate) trait Embedder: Send + Sync {
    /// `None` on any failure — the caller degrades to keyword-only scoring.
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector>;
}

/// A budget consulted immediately before each ACTUAL embedding round-trip.
///
/// The charge belongs HERE, at the call, evaluated on the exact bytes the call
/// consumes — never in a caller that predicts the call from earlier state. The
/// two answers diverge in practice: a caller sees the pre-translation blob (so
/// its hash never matches the cached row for a translated posting → it charges
/// on every total cache hit), and it cannot see the résumé-side embed at all
/// (an evicted résumé vector then makes a real, uncharged round-trip). Same
/// rule as `extension_bridge::answer_assist`: charge immediately before the
/// work that reaches the provider, once per round-trip, and not at all when a
/// cached path short-circuits.
///
/// `Err` means the ceiling refused: the round-trip must NOT happen.
/// `pub(crate)` for the same reason as [`Embedder`]: "charged exactly once" is
/// only a guarantee while every implementor is in this crate.
pub(crate) trait EmbedBudget: Send + Sync {
    fn charge_one_embed(&self) -> AppResult<()>;
}

/// Charge (if a budget is present) and then make one embedding round-trip.
///
/// The single choke point for every provider call the scoring kernel makes, so
/// "charged exactly once per actual embed" is a property of one function rather
/// than a convention each call site has to remember. A refused charge returns
/// `None` — the same degrade-to-keyword-only signal as a failed embed.
pub(crate) async fn embed_charged<E: Embedder + ?Sized>(
    embedder: &E,
    budget: Option<&dyn EmbedBudget>,
    text: &str,
) -> Option<EmbeddingVector> {
    if let Some(budget) = budget {
        if let Err(e) = budget.charge_one_embed() {
            log::info!("[embed] round-trip refused by the daily ceiling: {e}");
            return None;
        }
    }
    embedder.embed_one(text).await
}

/// Production [`Embedder`]: the app's configured embedding provider.
///
/// `embed` already logs its own failure (see its doc), so `.ok()` here only
/// discards the error from this `Option`-returning seam.
pub(crate) struct AppEmbedder<'a>(pub &'a AppHandle);

#[async_trait::async_trait]
impl Embedder for AppEmbedder<'_> {
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector> {
        embed(self.0, text).await.ok()
    }
}

/// Resolve the embedding for a job posting's (possibly translated) `text`,
/// using the persisted `posting_vectors` cache. A hit avoids the embed call
/// entirely. The cache is guarded by BOTH the active embedding space and a
/// `text_hash` of the exact `text` passed here, so a stale or wrong-language
/// row is a natural miss. Does NOT touch `PostingsCache` (raw-text vectors).
///
/// `budget` is charged only on the miss path, immediately before the call —
/// see [`EmbedBudget`]. `active` comes from the caller (which already resolved
/// it for its own cache key) so the hit decision and the score are computed
/// against one snapshot of the embedding space.
pub(crate) async fn posting_vector_or_embed<E: Embedder + ?Sized>(
    store: &DocumentStore,
    active: &EmbeddingConfig,
    embedder: &E,
    budget: Option<&dyn EmbedBudget>,
    job_id: &str,
    text: &str,
) -> Option<EmbeddingVector> {
    // Snapshot everything from the store before any await — the store methods
    // each take/release the lock internally and return owned values, so no DB
    // lock is held across the embed call below.
    let hash = sha256_hex(text);
    let cached = store.get_posting_vector(job_id);
    // Single cache-hit decision (space + text_hash), shared with its unit test.
    if posting_vector_is_fresh(active, &hash, cached.as_ref()) {
        return cached.map(|(v, _)| v); // cache hit — no embed, no charge
    }
    let v = embed_charged(embedder, budget, text).await?;
    // Best-effort cache write: the embed already succeeded, so a failed upsert
    // must not fail the score — it only means the NEXT call re-embeds (and
    // re-charges) this posting. Logged rather than dropped, because a
    // persistently failing write is invisible otherwise and reads downstream as
    // "the cache never hits". Never the text — but `job_id` is NOT always the
    // opaque `board:external-id` shape it looks like: `breezy`/`pinpoint`/
    // `themuse` build their [`crate::scraping::JobPosting::id`] as
    // `format!("{BOARD_ID}:{url}")`, embedding the posting's full URL. Redact
    // it with the same shape-based classifier the error already goes through,
    // rather than hashing — that would cost every OTHER board's (the
    // majority) debuggable `greenhouse:12345`-style id to close a leak only
    // these three have.
    if let Err(e) = store.upsert_posting_vector(job_id, &hash, &v) {
        log::warn!(
            "[documents] posting vector not cached for {}: {}",
            crate::observability::redact_token(job_id),
            sanitize_reason(&e.to_string())
        );
    }
    Some(v)
}
