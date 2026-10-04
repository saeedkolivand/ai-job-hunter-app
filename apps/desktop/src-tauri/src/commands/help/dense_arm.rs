//! The dense (embedding) arm of `help_search`: query + entry embeds through the
//! caller's config snapshot, the `help_vectors` cache, and the all-or-nothing
//! ranking rule.

use std::sync::Arc;

use async_trait::async_trait;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use super::HELP_EMBED_MISSES_MAX;
use crate::commands::ai_provider::EmbeddingVector;
use crate::commands::hybrid_search::{dense_pair, ArmStatus};
use crate::documents::{embed_with_config, sha256_hex, DocumentStore, Embedder, EmbeddingConfig};
use crate::error::AppResult;
use crate::ipc_contracts::help::HelpSearchRequestEntry;
use crate::observability::sanitize_reason;
use crate::retrieval::dense;

/// One embed round-trip on the caller's config snapshot, charged against the
/// active provider's daily ceiling.
///
/// Routed through `documents::embed_with_config` — never `embed`/`AppEmbedder`
/// (`documents::embed`), which independently RE-READS `embedding_config()` on
/// every call. `cfg` is read ONCE by [`run_dense`] and threaded through both
/// the charge closure (which provider's budget) and the dispatch (which
/// provider actually receives the request), so `ai_set_embedding_config`
/// landing mid-search can never charge one provider while dispatching to
/// another — the #1087 finding-2 shape.
///
/// Implements the crate's existing [`Embedder`] seam rather than being a bare
/// function, so [`run_dense_arm`] below takes no `AppHandle` and "how many
/// provider calls did this search make" is a plain unit test.
struct ChargedEmbedder<'a> {
    app: &'a AppHandle,
    limiter: Arc<crate::limits::Limiter>,
    cfg: &'a EmbeddingConfig,
}

#[async_trait]
impl Embedder for ChargedEmbedder<'_> {
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector> {
        let limiter = self.limiter.clone();
        let provider = self.cfg.provider.clone();
        let charge_fn =
            move || limiter.charge_provider_daily(&provider, crate::limits::PROVIDER_DAILY_MAX);
        let charge: &(dyn Fn() -> AppResult<()> + Send + Sync) = &charge_fn;
        // `embed_with_config` logs its own failure (sanitized); `.ok()` here
        // only discards the error into the degrade-to-keyword-only signal.
        embed_with_config(self.app, self.cfg, text, Some(charge))
            .await
            .ok()
    }
}

/// Resolve the app-side state the dense arm needs, then run it. The ONLY
/// `AppHandle`-touching part of the dense path — the cache/embed/rank logic
/// itself lives in [`run_dense_arm`], behind a store + an [`Embedder`].
///
/// Missing managed state is [`ArmStatus::Unavailable`], not a panic: this is
/// a degradable arm, and a `help_search` that 500s because the store was not
/// registered would be strictly worse than keyword-only results.
pub(super) async fn run_dense(
    app: &AppHandle,
    query: &str,
    entries: &[HelpSearchRequestEntry],
    token: &CancellationToken,
) -> (Vec<String>, ArmStatus) {
    let (Some(store), Some(limiter)) = (
        app.try_state::<DocumentStore>(),
        app.try_state::<Arc<crate::limits::Limiter>>(),
    ) else {
        return (Vec::new(), ArmStatus::Unavailable);
    };
    // ONE read, shared by the charge closure and every dispatch below.
    let cfg: EmbeddingConfig = store.embedding_config();
    let embedder = ChargedEmbedder {
        app,
        limiter: limiter.inner().clone(),
        cfg: &cfg,
    };
    run_dense_arm(
        &store,
        &cfg,
        &embedder,
        query,
        entries,
        crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
        token,
    )
    .await
}

/// One embed round-trip, raced against the cancellation token.
///
/// Wrapping [`Embedder::embed_one`] at the CALL SITE rather than putting the
/// token inside the trait: `Embedder` is implemented elsewhere in the crate
/// (`documents::embedding`) and a token parameter there would touch every
/// implementor for one caller's benefit. `biased`, so an already-cancelled
/// token wins before the embed is even polled.
///
/// The race is the point: with a bare `.await` a `jobs_cancel(queryId)` does
/// nothing once a call has started — it would sit in the registry cancelled
/// while this task waited out the provider's per-attempt timeout, which is
/// tens of seconds. Same shape, same reason as
/// `hybrid_search::candidates::embed_or_cancel`.
///
/// ONE function for both call sites (the query embed and each entry's), so
/// the query-embed tests that mutation-check the race cover the entry embeds'
/// too — including
/// `a_cancel_of_an_in_flight_embed_returns_without_waiting_the_provider_out`,
/// which observes the in-flight half with a fake that never returns and turns
/// the mutation's "hangs forever" into a failed `tokio::time::timeout`.
///
/// **A cancelled round-trip is CHARGED but never RECORDED.** The daily
/// per-vendor ceiling is charged before dispatch (inside `embed_with_config`,
/// see [`ChargedEmbedder`]) while `record_usage` runs only after the provider
/// answers, so dropping this future in the middle spends one unit of the
/// user's daily budget that never reaches the usage ledger. Never twice —
/// the charge happens once per attempt — and never a REFUND either, which is
/// the safe direction for a spend cap. This is a pre-existing shape inherited
/// from `hybrid_search::candidates::embed_or_cancel` (same select, same ordering) and is
/// deliberately NOT fixed here: the seam that could fix it is
/// `commands::ai_provider::embed_text`, where the charge and the ledger write
/// live, and making them atomic under cancellation is a provider-layer change
/// affecting every embedding caller — not something to smuggle into a help
/// search.
async fn embed_or_cancel<E: Embedder + ?Sized>(
    embedder: &E,
    text: &str,
    token: &CancellationToken,
) -> Option<EmbeddingVector> {
    tokio::select! {
        biased;
        () = token.cancelled() => None,
        embedded = embedder.embed_one(text) => embedded,
    }
}

/// Embed the query, resolve every entry's vector (cache first), and rank by
/// cosine similarity.
///
/// The query is embedded on EVERY request — it is different text each time,
/// so there is nothing to cache. Entry bodies are cached by
/// `sha256_hex(body)` in `help_vectors`, so once the cache is warm an
/// unchanged answer costs at most one embed per embedding space.
///
/// **Not exactly-once across concurrent calls.** The cache is read and
/// written under separate acquisitions of the store's connection lock (never
/// held across the embed, which is a network round trip) and there is no
/// in-flight registry, so two `help_search` calls that race on a COLD entry
/// each miss and each embed it. The upsert is idempotent, so the cache
/// converges once the first of them lands; what bounds the duplicates is not
/// a lock but the two budgets every embed here already passes through —
/// [`HELP_EMBED_MISSES_MAX`] misses per REQUEST, and
/// `limits::PROVIDER_DAILY_MAX` charged per embed before dispatch (see
/// [`ChargedEmbedder`]).
///
/// **All-or-nothing, by design.** The arm reports [`ArmStatus::Ran`] only
/// when EVERY requested entry came back RANKED; anything less — the
/// wall-clock bound below firing, the miss budget running out, a CANCEL, an
/// entry whose embed failed, a vector that came back in another embedding
/// space, a degenerate zero vector `dense::cosine` cannot score — returns
/// [`ArmStatus::Unavailable`] with NO ranks. Ranked, not merely paired: the
/// two counts differ for the last of those cases. Reporting `Ran`
/// on a partial pool would put `mode: "hybrid"` on the wire for a reply the
/// dense arm only half-ranked, and keeping the partial ranks while reporting
/// `Unavailable` would be the same lie from the other side: the fused order
/// would still be part-semantic under a `keyword` label. The embeds are not
/// wasted — every one of them is cached, so the next question is warm.
///
/// Returns [`ArmStatus::Unavailable`] when the query embed fails too. In
/// every one of these cases the caller still returns the lexical results, so
/// an unreachable embedding provider degrades the search rather than failing
/// it.
///
/// Takes a store + an [`Embedder`] rather than an `AppHandle` so the cache
/// decisions (hit by hash, miss on changed text, miss on a changed embedding
/// space), the embed COUNT and both bounds are unit tests over a real
/// `DocumentStore`.
///
/// `budget` is production's `timeouts::DENSE_ARM_TIMEOUT`, passed in by
/// [`run_dense`] rather than read here so the bound itself is testable: a test
/// that had to wait out the real one would be a 100-second test nobody runs.
///
/// `token` is the request's cancellation token (module doc). Passing it here
/// rather than only at the command boundary is what makes cancellation a
/// TEST: this function takes no `AppHandle`, so "a pre-cancelled token embeds
/// nothing" and "a cancel after the Nth embed stops there" are unit tests
/// over a counting fake `Embedder` — the exact assertions
/// `hybrid_search::candidates::run_dense_arm` cannot make about itself.
async fn run_dense_arm<E: Embedder + ?Sized>(
    store: &DocumentStore,
    active: &EmbeddingConfig,
    embedder: &E,
    query: &str,
    entries: &[HelpSearchRequestEntry],
    budget: std::time::Duration,
    token: &CancellationToken,
) -> (Vec<String>, ArmStatus) {
    // No separate "already cancelled?" check before the query embed: the
    // `biased` race inside [`embed_or_cancel`] IS that check — an
    // already-cancelled token wins before `embed_one` is polled at all, so a
    // cancel that landed before this arm started costs ZERO provider calls
    // (`a_pre_cancelled_token_embeds_nothing_and_reports_unavailable` asserts
    // the COUNT, not just the status). A second guard here would be one no
    // test could tell apart from its absence.
    //
    // Started BEFORE the query embed, exactly like `hybrid_search`'s own dense
    // arm: a slow query embed eats into the SAME budget rather than getting a
    // separate one.
    let started = std::time::Instant::now();
    let Some(query_vector) = embed_or_cancel(embedder, query, token).await else {
        return (Vec::new(), ArmStatus::Unavailable);
    };
    let query_f32: Vec<f32> = query_vector.values.iter().map(|v| *v as f32).collect();

    let mut pairs: Vec<(String, Vec<f32>)> = Vec::with_capacity(entries.len());
    let mut misses = 0usize;
    for entry in entries {
        // The whole loop is bounded by ELAPSED time, the same constant and
        // the same shape `hybrid_search::candidates::run_dense_arm` uses (a
        // `tokio::time::timeout` around the loop would DROP the future and
        // throw away the vectors already cached inside it), and it is the
        // only bound an ID-less (uncancellable) caller gets.
        //
        // The token is checked BETWEEN entries, alongside it, for the same
        // reason: a `break` leaves the vectors already written to the cache
        // in place. Each individual embed is raced separately
        // ([`embed_or_cancel`]), so a cancel arriving mid-embed does not have
        // to wait out the provider — but one that arrives just after an embed
        // RESOLVES still paid for it. Bounded and deliberate, the same
        // tradeoff `hybrid_search`'s candidate loop makes.
        //
        // This check is NOT redundant with the race. Every remaining entry
        // could be a cache HIT, which needs no embed and therefore never
        // touches the token: without this `break`, a search cancelled during
        // its one cold embed would pair every entry anyway and report `Ran`
        // — a `mode: "hybrid"` reply for a search the user cancelled
        // (`a_cancel_mid_arm_is_unavailable_even_when_every_remaining_entry_is_cached`).
        if started.elapsed() >= budget || token.is_cancelled() {
            break;
        }
        let hash = sha256_hex(&entry.body);
        let vector = match store.get_help_vector(&hash, active) {
            Some(cached) => Some(cached),
            None => {
                if misses >= HELP_EMBED_MISSES_MAX {
                    // Budget spent. Nothing further can restore `Ran` (this
                    // entry can no longer be paired), so there is nothing to
                    // gain by walking the rest of the list.
                    break;
                }
                misses += 1;
                let embedded = embed_or_cancel(embedder, &entry.body, token).await;
                if let Some(v) = &embedded {
                    // Best-effort cache write: the embed already succeeded,
                    // so a failed upsert must not fail the search — it only
                    // means the next question re-embeds (and re-charges) this
                    // entry. Logged rather than dropped, because a
                    // persistently failing write is otherwise invisible and
                    // reads downstream as "the cache never hits". Neither the
                    // hash nor the entry text is logged; the reason goes
                    // through the same sanitizer every other store-write
                    // warning uses (a rusqlite error can carry a path).
                    if let Err(e) = store.upsert_help_vector(&hash, v) {
                        log::warn!(
                            "[help_search] help vector not cached: {}",
                            sanitize_reason(&e.to_string())
                        );
                    }
                }
                embedded
            }
        };
        // `dense_pair` (shared with `hybrid_search`) is what keeps two
        // vectors from different embedding spaces from ever being scored
        // together — belt and braces with `get_help_vector`'s own space
        // check, and the only guard on a FRESH embed.
        if let Some(pair) = vector
            .as_ref()
            .and_then(|v| dense_pair(&entry.id, &query_vector.space, v))
        {
            pairs.push(pair);
        }
    }
    // The all-or-nothing rule (see this fn's doc), checked on the RANKED list
    // rather than on `pairs`: `rank_by_similarity` drops a candidate `cosine`
    // cannot score, so a paired-but-unscorable vector (a zero-magnitude one —
    // `dense_pair` compares embedding SPACES, and a degenerate all-zero vector
    // is in the right space) used to pass a `pairs.len()` check and then be
    // dropped one line later, which is exactly the partial ranking reported as
    // `hybrid` this rule exists to prevent. Also still the ONE check that
    // covers every early `break` above: a loop that stopped short leaves the
    // ranking short too, so neither bound needs its own reporting path.
    let ranked = dense::rank_by_similarity(&query_f32, &pairs);
    if ranked.len() < entries.len() {
        return (Vec::new(), ArmStatus::Unavailable);
    }
    (ranked, ArmStatus::Ran)
}

#[cfg(test)]
mod tests;
