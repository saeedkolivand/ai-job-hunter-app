//! Reusable validation infrastructure for generated content.

use async_trait::async_trait;

use super::Completer;
use crate::error::AppResult;

/// A single problem found in generated content.
pub struct ValidationIssue {
    pub kind: String,
    pub detail: String,
}

/// The outcome of validating a draft.
pub struct ValidationReport {
    pub passed: bool,
    /// Short machine-ish verdict (e.g. "PASS" / "FAIL" / "SKIPPED").
    pub verdict: String,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn skipped() -> Self {
        Self {
            passed: true,
            verdict: "SKIPPED".to_string(),
            issues: Vec::new(),
        }
    }
}

/// Validates a generated draft. Implementations run through the centralized
/// provider via [`Completer`]; a hard infra/network error is `Err`, while a
/// content failure is `Ok(report)` with `passed = false`.
#[async_trait]
pub trait Validator: Send + Sync {
    fn name(&self) -> &'static str;
    async fn validate(&self, completer: &Completer, draft: &str) -> AppResult<ValidationReport>;
}
