//! Research / enrichment result type.
//!
//! An enricher fetches external context for a workflow (e.g. company research for
//! a cover letter) and degrades gracefully — returning an empty result rather
//! than failing when the source is unavailable.

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
