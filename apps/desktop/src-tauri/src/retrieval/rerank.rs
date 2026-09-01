//! The rerank PORT (L1) — declared here so the search algorithm can depend on
//! "something that can re-order candidates against a query" without naming
//! `Completer` or any AI-provider type (both L2/L3). Implemented at L3
//! (`commands::hybrid_search`), the same port/adapter split
//! `pipeline::Stage`/`StageHooks` already uses for the identical reason.

use async_trait::async_trait;

use crate::error::AppResult;

/// How many of the fused ranking's TOP candidates get sent to the optional
/// LLM rerank step, per search.
///
/// A COST bound, not a quality/recall claim: it caps one search at 20 fenced
/// candidates in one prompt regardless of corpus size, the same shape
/// `commands::autopilot::rerank::SEMANTIC_RERANK_MAX` uses to bound ITS
/// re-rank phase (see that constant's own doc: "the real ceiling is cost").
/// Whether 20 is enough for a given corpus/query is unmeasured.
pub const RERANK_TOP_K: usize = 20;

/// One candidate handed to a [`Reranker`] — RAW, UNFENCED posting text.
///
/// This is scraped, attacker-controlled text (OWASP LLM01: a job ad can
/// contain instructions aimed at whatever reads it). The port carries no
/// fencing itself because building the actual prompt string — the fence tag,
/// the surrounding instructions, the schema — is an L3-only concern
/// (`prompt_fence` is reachable from L1, but there is nothing to fence
/// *into* here); every implementor MUST fence `text` before it reaches a
/// prompt.
pub struct RerankCandidate {
    pub id: String,
    pub text: String,
}

/// A provider-backed re-rank of `candidates` against `query`, returning ids
/// BEST-FIRST.
///
/// The returned list is untrusted output and must be validated by the
/// CALLER, never trusted blindly: an id the caller didn't supply, a
/// duplicate, or a partial list are all things a small local model can
/// produce under this contract, and the caller's fallback (the pre-rerank
/// fused order) is what covers every one of them.
#[async_trait]
pub trait Reranker: Send + Sync {
    async fn rerank(&self, query: &str, candidates: &[RerankCandidate]) -> AppResult<Vec<String>>;
}
