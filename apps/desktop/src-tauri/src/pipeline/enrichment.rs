//! Research / enrichment result type.
//!
//! An enricher fetches external context for a workflow (e.g. company research for
//! a cover letter) and degrades gracefully — returning an empty result rather
//! than failing when the source is unavailable.

use serde_json::{json, Value};

use crate::error::FailureKind;

/// How one enrichment pass ended. `Empty` ("nothing found") and `Failed` ("could not ask")
/// are different facts: a letter written after a failed lookup is missing research the
/// user may have paid for, and the kind says whether the fix is credit, a key or time (#1500).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchStatus {
    Ok,
    Empty,
    /// `None` when the failure is not a classifiable provider error.
    Failed(Option<FailureKind>),
}

impl ResearchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Empty => "empty",
            Self::Failed(_) => "failed",
        }
    }

    /// `{ status, errorKind? }` — content-free, the shape a run's metrics carry.
    pub fn wire(self) -> Value {
        match self {
            Self::Failed(Some(kind)) => json!({ "status": "failed", "errorKind": kind.as_str() }),
            other => json!({ "status": other.as_str() }),
        }
    }
}

/// The product of an enrichment pass: a `key` identifying the subject (e.g. a
/// company name) and the enrichment `content` (empty when nothing was found).
pub struct EnrichmentResult {
    pub key: String,
    pub content: String,
    pub status: ResearchStatus,
}

impl EnrichmentResult {
    pub fn empty() -> Self {
        Self {
            key: String::new(),
            content: String::new(),
            status: ResearchStatus::Empty,
        }
    }
}
