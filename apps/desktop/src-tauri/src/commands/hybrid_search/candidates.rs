//! Candidate generation for hybrid search: the posting rows read off the live
//! cache, the eligibility filter, the lexical arm that ranks them, and the
//! optional dense arm that embeds the top of that list (cache first) and
//! re-ranks by cosine similarity.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use super::ArmStatus;
use crate::documents::{embed_with_config, DocumentStore, EmbeddingConfig};
use crate::error::AppResult;
use crate::postings::PostingsCache;
use crate::retrieval::dense;
use crate::retrieval::lexical::{LexicalDoc, LexicalIndex};

// ── Corpus extraction ────────────────────────────────────────────────────────

/// The fields this search reads off a cached posting `Value` — extracted
/// once so lexical indexing, dense embedding and rerank fencing all read the
/// SAME text instead of three separate ad-hoc field pulls.
pub(super) struct PostingRow {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) company: String,
    location: String,
    pub(super) description: String,
}

fn to_posting_row(item: &Value) -> Option<PostingRow> {
    let id = item.get("id")?.as_str()?.to_string();
    let field = |name: &str| {
        item.get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Some(PostingRow {
        id,
        title: field("title"),
        company: field("company"),
        location: field("location"),
        description: field("description"),
    })
}

/// The subset of `items` this search ranks over: everything, or — when
/// `eligible_ids` is present and non-empty — only the rows whose id is in
/// it. An id in `eligible_ids` absent from `items` is silently dropped
/// (never trusted): this is renderer-supplied input crossing the IPC
/// boundary, and the live cache is the only source of truth for what
/// actually exists to rank.
pub(super) fn eligible_subset(items: &[Value], eligible_ids: Option<&[String]>) -> Vec<PostingRow> {
    let allow: Option<std::collections::HashSet<&str>> = eligible_ids
        .filter(|ids| !ids.is_empty())
        .map(|ids| ids.iter().map(String::as_str).collect());
    items
        .iter()
        .filter_map(to_posting_row)
        .filter(|row| {
            allow
                .as_ref()
                .is_none_or(|set| set.contains(row.id.as_str()))
        })
        .collect()
}

pub(super) fn to_lexical_doc(row: &PostingRow) -> LexicalDoc<'_> {
    LexicalDoc {
        id: &row.id,
        title: &row.title,
        company: &row.company,
        location: &row.location,
        description: &row.description,
    }
}

pub(super) fn corpus_generation(app: &AppHandle) -> u64 {
    app.state::<Mutex<PostingsCache>>().lock().generation()
}

/// Run the lexical arm end-to-end (build the FTS5 index over `docs`, then
/// query it) and collapse a build/search failure to `ArmStatus::Unavailable`
/// — the ONE place that reporting decision is made. `retrieval::lexical`
/// returns a `Result` precisely so an L3 caller can tell "zero hits" apart
/// from "FTS5 itself failed" (see `LexicalIndex::search`'s own doc for the
/// empirically-verified NUL-byte trigger); swallowing that distinction back
/// into a bare `Vec` here would report a genuine failure as a successful
/// keyword search that happened to find nothing, contradicting this
/// module's whole "degrade, never silently claim more than ran" contract.
///
/// Pure (no app/network), so this exact mapping is a unit test, not a claim.
pub(super) fn run_lexical_arm(
    docs: &[LexicalDoc<'_>],
    query: &str,
    limit: usize,
) -> (Vec<String>, ArmStatus) {
    let result = LexicalIndex::build(docs).and_then(|index| index.search(query, limit));
    match result {
        Ok(ranks) => (ranks, ArmStatus::Ran),
        Err(_) => (Vec::new(), ArmStatus::Unavailable),
    }
}

/// How many of the LEXICAL arm's top-ranked postings get embedded for the
/// dense arm, per search.
///
/// A COST bound, not a recall claim: it caps one search's dense-arm spend at
/// 40 embeds (plus the query) regardless of how large the live cache grows —
/// the same shape `commands::autopilot::rerank::SEMANTIC_RERANK_MAX` uses to
/// bound ITS own re-rank phase.
///
/// **Recall limitation, stated plainly rather than hidden.** When the
/// lexical arm found ANYTHING, the dense arm only RE-SCORES those same
/// top-40 lexical hits (see [`dense_candidate_pool`]'s `if` branch) — it
/// never embeds a posting lexical search missed, so it cannot surface one.
/// Dense search only RETRIEVES beyond lexical's own results in the one case
/// lexical found NOTHING at all (the `else` branch), where it instead embeds
/// the first 40 eligible postings in cache order. So "hybrid search finds
/// what keyword search cannot" is only true when keyword search finds
/// literally zero matches; whenever it finds anything, dense can only
/// RE-ORDER that same set, never widen it. Retrieving a broader dense
/// candidate set independently of the lexical hits (embedding more of the
/// corpus even when lexical found something) is a real spend/latency
/// trade-off, deliberately left out of scope here.
const DENSE_CANDIDATE_MAX: usize = 40;

// ── Dense arm ────────────────────────────────────────────────────────────────

/// One embed round-trip, raced against cancellation and routed through
/// `documents::embed_with_config` — never `embed_charged`/`AppEmbedder`
/// (`documents::embed`), which independently RE-READS `embedding_config()`
/// on every call. `cfg` is read ONCE by the caller (`run_dense_arm`) and
/// threaded through every call this makes, so the config that gets CHARGED
/// here is provably the config that gets DISPATCHED to, even if
/// `ai_set_embedding_config` lands mid-search — the exact #1087 finding 2
/// shape `ai_embed`'s own doc comment (`commands/ai/mod.rs`) describes fixing
/// for the direct `ai_embed` IPC path.
///
/// Racing against `token.cancelled()` (rather than a bare `.await`) means a
/// cancel mid-embed aborts promptly instead of waiting out the resolved
/// provider's own internal per-attempt timeout
/// (`timeouts::OLLAMA_EMBED`/`EMBED`, up to 30s each).
async fn embed_or_cancel(
    app: &AppHandle,
    limiter: &Arc<crate::limits::Limiter>,
    cfg: &EmbeddingConfig,
    text: &str,
    token: &CancellationToken,
) -> Option<crate::commands::ai_provider::EmbeddingVector> {
    let limiter = limiter.clone();
    let provider = cfg.provider.clone();
    let charge_fn =
        move || limiter.charge_provider_daily(&provider, crate::limits::PROVIDER_DAILY_MAX);
    let charge: &(dyn Fn() -> AppResult<()> + Send + Sync) = &charge_fn;
    tokio::select! {
        biased;
        () = token.cancelled() => None,
        result = embed_with_config(app, cfg, text, Some(charge)) => result.ok(),
    }
}

/// Which posting ids the dense arm embeds, in order — see
/// [`DENSE_CANDIDATE_MAX`]'s doc for the bound, the empty-lexical fallback,
/// and the recall limitation this pool shape carries. Pure (no app/network)
/// so the fallback path — cache order, NOT a `HashMap`'s unspecified
/// iteration order — is a unit test rather than a claim.
fn dense_candidate_pool<'a>(
    eligible: &'a [PostingRow],
    lexical_ranks: &'a [String],
) -> Vec<&'a str> {
    if lexical_ranks.is_empty() {
        eligible
            .iter()
            .take(DENSE_CANDIDATE_MAX)
            .map(|row| row.id.as_str())
            .collect()
    } else {
        lexical_ranks
            .iter()
            .take(DENSE_CANDIDATE_MAX)
            .map(String::as_str)
            .collect()
    }
}

/// Convert a candidate embedding into the `(id, Vec<f32>)` pair the dense arm
/// scores — but ONLY when it shares the query vector's embedding space.
///
/// Pure, so "two vectors from different embedding spaces are never scored
/// together" — `commands::ai_provider::compare`'s own rule ("incomparable
/// vectors are never silently scored") — is a unit test at the one L3
/// boundary where an `EmbeddingSpace` is still in scope: `retrieval::dense`
/// never sees one (it works on bare `&[f32]`, by design — see its module
/// doc) and could not enforce this itself.
///
/// `pub(crate)` because `commands::help`'s dense arm has the identical
/// boundary and must make the identical decision — one implementation of
/// "incomparable vectors are never silently scored", not two.
pub(crate) fn dense_pair(
    id: &str,
    query_space: &crate::commands::ai_provider::EmbeddingSpace,
    candidate: &crate::commands::ai_provider::EmbeddingVector,
) -> Option<(String, Vec<f32>)> {
    if candidate.space != *query_space {
        return None;
    }
    Some((
        id.to_string(),
        candidate.values.iter().map(|v| *v as f32).collect(),
    ))
}

/// Bounds the WHOLE candidate loop by ELAPSED time (checked alongside the
/// existing cancellation check, same shape), not a `tokio::time::timeout`
/// wrapping the loop — a `timeout` DROPS the wrapped future on expiry, which
/// would throw away every pair already collected; measuring elapsed time
/// lets the loop `break` and rank whatever it has, the friendlier of the two
/// treatments the rerank arm's own `tokio::time::timeout` doesn't need
/// (rerank has nothing partial to salvage — one JSON completion either
/// finishes or it doesn't). Started BEFORE the query embed so a slow query
/// embed also eats into the same budget, not a separate one.
pub(super) async fn run_dense_arm(
    app: &AppHandle,
    query: &str,
    eligible: &[PostingRow],
    eligible_by_id: &HashMap<&str, &PostingRow>,
    lexical_ranks: &[String],
    token: &CancellationToken,
    generation0: u64,
) -> (Vec<String>, ArmStatus) {
    let started = std::time::Instant::now();
    let (Some(doc_store), Some(limiter_state)) = (
        app.try_state::<DocumentStore>(),
        app.try_state::<Arc<crate::limits::Limiter>>(),
    ) else {
        return (Vec::new(), ArmStatus::Unavailable);
    };
    // ONE read, shared by every charge closure and every `embed_with_config`
    // dispatch below — see `embed_or_cancel`'s own doc for why re-reading it
    // per call (the #1087 finding 2 shape) is exactly what this avoids.
    let active_cfg: EmbeddingConfig = doc_store.embedding_config();
    let limiter = limiter_state.inner().clone();

    let Some(query_vector) = embed_or_cancel(app, &limiter, &active_cfg, query, token).await else {
        return (Vec::new(), ArmStatus::Unavailable);
    };
    let query_f32: Vec<f32> = query_vector.values.iter().map(|v| *v as f32).collect();

    let pool = dense_candidate_pool(eligible, lexical_ranks);
    let mut pairs: Vec<(String, Vec<f32>)> = Vec::with_capacity(pool.len());
    for id in pool {
        if token.is_cancelled()
            || started.elapsed() >= crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT
        {
            break;
        }
        let Some(row) = eligible_by_id.get(id) else {
            continue;
        };
        let cached = app.state::<Mutex<PostingsCache>>().lock().get_embedding(id);
        let vector = match cached {
            Some(v) if active_cfg.matches(&v.space) => Some(v),
            _ => {
                let Some(blob) = crate::documents::keywords::posting_text_blob(
                    &row.title,
                    Some(&row.description),
                    None,
                ) else {
                    continue;
                };
                let embedded = embed_or_cancel(app, &limiter, &active_cfg, &blob, token).await;
                if let Some(v) = &embedded {
                    // Re-check the corpus generation under the SAME lock as
                    // the write: `clear_all()` (a replace-scrape's first
                    // streamed item, or `privacy_reset_app`) may have wiped
                    // this posting's text while the embed above was in
                    // flight — writing a vector derived from text that no
                    // longer exists would resurrect it into a cleared cache.
                    let cache_state = app.state::<Mutex<PostingsCache>>();
                    let mut guard = cache_state.lock();
                    if guard.generation() == generation0 {
                        guard.set_embedding(id.to_string(), v.clone());
                    }
                }
                embedded
            }
        };
        if let Some(pair) = vector
            .as_ref()
            .and_then(|v| dense_pair(id, &query_vector.space, v))
        {
            pairs.push(pair);
        }
    }
    if pairs.is_empty() {
        return (Vec::new(), ArmStatus::Unavailable);
    }
    (
        dense::rank_by_similarity(&query_f32, &pairs),
        ArmStatus::Ran,
    )
}

#[cfg(test)]
mod tests;
