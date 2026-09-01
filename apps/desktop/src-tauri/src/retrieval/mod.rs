//! Hybrid (lexical + dense) search ranking — pure algorithms, no Tauri, no
//! network, no AI-provider types (L1; see `docs/architecture-rules.md`).
//!
//! This module answers ONE question — "given a query and a corpus, what
//! order should the results come back in?" — and deliberately does not
//! answer two others that sound like they belong here:
//!
//! * **Where does the corpus live?** Nowhere in this module. There is no
//!   persisted posting-text store (`postings::PostingsCache` is in-memory and
//!   ephemeral by design — see its module doc); [`lexical::LexicalIndex`] is
//!   rebuilt fresh, in memory, per search, over whatever slice of the live
//!   cache the caller hands it.
//! * **Who computes an embedding?** Not this module either. [`dense`] ranks
//!   already-embedded `&[f32]` vectors; it never calls a provider and never
//!   names `EmbeddingVector` or `Completer` (both L2/L3). The caller —
//!   `commands::hybrid_search` — does its own embedding (reviving
//!   `PostingsCache::{get_embedding,set_embedding}`) and hands this module
//!   plain vectors. Same story for [`rerank::Reranker`]: a port declared
//!   here, implemented at L3 against `Completer`, mirroring the
//!   `pipeline::Stage`/`StageHooks` port-at-a-lower-layer pattern.
//!
//! [`fusion::reciprocal_rank_fusion`] combines the lexical and dense arms'
//! RANKS (not their incomparable raw scores — a BM25 value and a cosine
//! similarity live on different scales), so a search that only ran one arm
//! (semantic scoring off, or an embedding failure) degrades to that arm's
//! order for free, with no special-casing at the fusion call site.

pub mod dense;
pub mod fusion;
pub mod lexical;
pub mod rerank;

#[cfg(test)]
mod test;
