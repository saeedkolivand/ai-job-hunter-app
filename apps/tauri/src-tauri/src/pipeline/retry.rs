//! Reusable generate → validate → regenerate loop.

use async_trait::async_trait;

use super::validation::{ValidationReport, Validator};
use super::Completer;
use crate::error::AppResult;

/// How many generation attempts a [`generate_validated`] loop may make.
pub struct RetryPolicy {
    pub max_attempts: u8,
}

impl RetryPolicy {
    pub fn new(max_attempts: u8) -> Self {
        Self { max_attempts: max_attempts.max(1) }
    }
}

/// Produces a draft for a given attempt index, through the centralized provider.
/// Implementations own their prompt-building and emit any feature progress events
/// (via `completer.app()`); the loop stays generic.
#[async_trait]
pub trait DraftGenerator: Send + Sync {
    async fn generate(&self, completer: &Completer, attempt: u8) -> AppResult<String>;
}

/// Result of a generate/validate loop.
pub struct ValidatedDraft {
    pub text: String,
    pub report: Option<ValidationReport>,
    /// How many times the draft was regenerated after a failed validation (0–N).
    pub retries: u8,
}

/// Generate a draft and, if a `validator` is supplied, validate it — regenerating
/// up to `policy.max_attempts` times while validation fails. A validator *infra*
/// error is non-fatal: the current draft is kept with a `SKIPPED` report.
pub async fn generate_validated(
    completer: &Completer,
    policy: RetryPolicy,
    generator: &dyn DraftGenerator,
    validator: Option<&dyn Validator>,
) -> AppResult<ValidatedDraft> {
    let mut text = String::new();
    let mut report: Option<ValidationReport> = None;
    let mut retries = 0u8;
    let max = policy.max_attempts;

    for attempt in 0..max {
        text = generator.generate(completer, attempt).await?;

        let Some(validator) = validator else {
            break;
        };

        match validator.validate(completer, &text).await {
            Ok(r) => {
                let passed = r.passed;
                if !passed {
                    for issue in &r.issues {
                        log::debug!(
                            "[pipeline] {} issue [{}]: {}",
                            validator.name(),
                            issue.kind,
                            issue.detail
                        );
                    }
                }
                report = Some(r);
                if passed || attempt + 1 == max {
                    break;
                }
                retries += 1;
            }
            Err(e) => {
                log::warn!("[pipeline] validator '{}' error (non-fatal): {e}", validator.name());
                report = Some(ValidationReport::skipped());
                break;
            }
        }
    }

    Ok(ValidatedDraft { text, report, retries })
}
