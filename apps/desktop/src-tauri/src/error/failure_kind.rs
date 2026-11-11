//! Why a provider-backed operation failed, in one of five words a caller can act on.
//!
//! The wire form of "the account is out of credit" versus "the key is wrong" versus
//! "try again later" versus "the model answered garbage" (#1496, #1500). Derived from the
//! typed [`AppError`] and, because most providers surface their HTTP status only inside the
//! message, from that text. Content-free by construction (ADR-032 point 7): the result is a
//! fixed token, never the message.

use super::AppError;

/// The closed vocabulary. `snake_case` on the wire; mirrored by `PipelineErrorKind` in
/// `packages/shared/src/ipc/contracts/resumePipeline.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Out of credit, payment required, or a plan that does not cover the call.
    Billing,
    /// A missing, invalid, or unauthorized key.
    Auth,
    /// A request-rate or quota limit; retry later.
    RateLimited,
    /// The provider or network could not serve the call (outage, timeout, unreachable).
    Unavailable,
    /// The provider answered, but not with usable output (refusal, truncation, empty analysis).
    ModelOutput,
}

impl FailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Billing => "billing",
            Self::Auth => "auth",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
            Self::ModelOutput => "model_output",
        }
    }

    /// Classify `e`; `None` for an error that is not a provider failure (validation, storage,
    /// a cancel, an uncategorized message).
    pub fn of(e: &AppError) -> Option<Self> {
        match e {
            AppError::Parse(_) | AppError::Refusal(_) | AppError::OutputLimit(_) => {
                Some(Self::ModelOutput)
            }
            AppError::RateLimited(_) => Some(Self::RateLimited),
            AppError::Timeout(_) => Some(Self::Unavailable),
            AppError::Config(m) => from_text(m).filter(|k| *k == Self::Auth),
            AppError::Network(m) | AppError::Provider(m) => {
                Some(from_text(m).unwrap_or(Self::Unavailable))
            }
            _ => None,
        }
    }
}

/// Billing before auth before rate limit: "429 ... check your plan" is a rate limit, but a 402
/// that mentions an API key is still billing.
fn from_text(message: &str) -> Option<FailureKind> {
    let t = message.to_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| t.contains(n));
    if has_status(&t, "402")
        || any(&[
            "payment required",
            "usage credit",
            "out of credit",
            "insufficient credit",
            "insufficient_quota",
            "billing",
        ])
    {
        Some(FailureKind::Billing)
    } else if has_status(&t, "401")
        || has_status(&t, "403")
        || any(&["unauthorized", "forbidden", "api key", "authentication"])
    {
        Some(FailureKind::Auth)
    } else if has_status(&t, "429") || any(&["rate limit", "too many requests"]) {
        Some(FailureKind::RateLimited)
    } else {
        None
    }
}

/// `code` as a whole number, not a fragment of a longer one ("1402", "4020 chars").
fn has_status(text: &str, code: &str) -> bool {
    text.match_indices(code).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + code.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_digit()) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests;
