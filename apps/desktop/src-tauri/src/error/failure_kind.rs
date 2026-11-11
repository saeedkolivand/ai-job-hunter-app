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

    /// The wire token back to the enum; `None` for anything else, so a value read from storage
    /// (a restored backup can hold anything) never passes through as a kind.
    pub fn parse(token: &str) -> Option<Self> {
        [
            Self::Billing,
            Self::Auth,
            Self::RateLimited,
            Self::Unavailable,
            Self::ModelOutput,
        ]
        .into_iter()
        .find(|k| k.as_str() == token)
    }

    /// A fixed, app-authored sentence for the kind. This — never the provider's own text, which
    /// can echo prompt or résumé content or carry injected instructions — is what a run reports
    /// when the kind is known.
    pub fn summary(self) -> &'static str {
        match self {
            Self::Billing => "The AI provider reports the account is out of credit or over its quota. Add credit or check the plan, then retry.",
            Self::Auth => "The AI provider rejected the API key. Check the key in Settings.",
            Self::RateLimited => "The AI provider is rate limiting requests. Wait a moment, then retry.",
            Self::Unavailable => "The AI provider could not be reached or is overloaded. Retry shortly.",
            Self::ModelOutput => "The model answered with nothing usable. Retry, or pick a different model.",
        }
    }

    /// Classify `e`; `None` for anything that is not a recognisable provider failure
    /// (validation, storage, a cancel, an uncategorized message, an unrecognised provider text).
    ///
    /// The typed variants are the source of truth — `friendly_api_error` has the status and body
    /// and types an out-of-credit account as [`AppError::Billing`]. Text is a FALLBACK for
    /// transports that only have a string (the web-search call, a CLI agent), and it needs a
    /// status code or a phrase only an upstream error uses; an unrecognised message is `None`,
    /// never a guess.
    pub fn of(e: &AppError) -> Option<Self> {
        match e {
            AppError::Billing(_) => Some(Self::Billing),
            AppError::Parse(_) | AppError::Refusal(_) | AppError::OutputLimit(_) => {
                Some(Self::ModelOutput)
            }
            AppError::RateLimited(_) => Some(Self::RateLimited),
            AppError::Timeout(_) => Some(Self::Unavailable),
            AppError::Config(m) => from_text(m).filter(|k| *k == Self::Auth),
            // A `Network` error is either an HTTP status the mapper already put into its text, or
            // a transport failure (no response at all): the latter has no text to recognise, and
            // is unavailable by definition.
            AppError::Network(m) => from_text(m).or(Some(Self::Unavailable)),
            AppError::Provider(m) => from_text(m),
            _ => None,
        }
    }
}

/// Whether an upstream error body (or in-band error frame) says the account is out of credit or
/// quota. Phrases a provider uses for that and for nothing else. Lives here, in the lower layer,
/// so both the provider's status mapper and [`from_text`] share one list.
pub(crate) fn is_billing_text(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    [
        "insufficient_quota",
        "exceeded your current quota",
        "credit balance is too low",
        "out of credit",
        "insufficient credit",
        "usage credits",
        "payment required",
        "billing_hard_limit",
    ]
    .iter()
    .any(|k| t.contains(k))
}

/// Status-or-phrase sniffing over a message. Billing before auth before rate limit.
fn from_text(message: &str) -> Option<FailureKind> {
    let t = message.to_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| t.contains(n));
    if has_status(&t, "402") || is_billing_text(&t) {
        Some(FailureKind::Billing)
    } else if has_status(&t, "401")
        || has_status(&t, "403")
        || any(&["invalid or unauthorized api key"])
    {
        Some(FailureKind::Auth)
    } else if has_status(&t, "429") || any(&["rate limit or quota reached"]) {
        Some(FailureKind::RateLimited)
    } else if (500..=599).any(|c| has_status(&t, &c.to_string()))
        || any(&[
            "service error (",
            "service busy or rate limited",
            "unreachable",
            "the stream ended before",
        ])
    {
        Some(FailureKind::Unavailable)
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
