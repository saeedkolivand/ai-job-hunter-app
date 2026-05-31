//! Reusable research / enrichment stage infrastructure.
//!
//! An [`Enricher`] fetches external context for a workflow (e.g. company research
//! for a cover letter) and is expected to degrade gracefully — returning an empty
//! result rather than failing the pipeline when the source is unavailable.

use async_trait::async_trait;

use super::Completer;

/// The product of an enrichment pass: a `key` identifying the subject (e.g. a
/// company name) and the enrichment `content` (empty when nothing was found).
pub struct EnrichmentResult {
    pub key: String,
    pub content: String,
}

impl EnrichmentResult {
    pub fn empty() -> Self {
        Self {
            key: String::new(),
            content: String::new(),
        }
    }
}

/// Fetches and (optionally) caches external context for a workflow. Reaches
/// managed state / credentials via `completer.app()`; runs LLM synthesis through
/// the centralized provider via `completer`.
#[async_trait]
pub trait Enricher: Send + Sync {
    async fn enrich(&self, completer: &Completer, input: &str) -> EnrichmentResult;
}
