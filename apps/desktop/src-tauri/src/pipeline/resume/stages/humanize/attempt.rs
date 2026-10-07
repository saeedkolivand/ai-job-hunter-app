//! One document's whole humanize attempt: the provider call, the usable-answer
//! gate, and the deterministic accept/revert decision, all injected so the
//! seam is testable without a live `Completer`.

use crate::error::{AppError, AppResult};
use crate::pipeline::resume::{prompts::HumanizeTier, RunDeadline};
use crate::validate::content::ContentReport;

use super::predicates::{exceeds_humanize_cap, humanize_is_worse, is_usable_rewrite};

/// What one document's humanize attempt did — everything the stage needs to
/// update `ctx` and record the ledger, so the attempt itself stays pure(-ish)
/// and testable with injected closures instead of a live `Completer`.
#[derive(Debug, Clone)]
pub(crate) struct HumanizeAttempt {
    /// The text to keep: the candidate on accept, the original on every other
    /// outcome.
    pub text: String,
    /// The report to keep, paired 1:1 with [`Self::text`].
    pub report: ContentReport,
    /// A provider call was actually made (charged and sent).
    pub called: bool,
    /// The candidate was graded and discarded as worse.
    pub reverted: bool,
    /// The provider call itself errored (network/provider failure) — distinct
    /// from a candidate that came back and was rejected.
    pub failed: bool,
    /// The run's deadline had already passed; nothing was attempted.
    pub timed_out: bool,
    /// `original_text` was over `HUMANIZE_DOCUMENT_CAP` — see
    /// [`exceeds_humanize_cap`]. Nothing was sent; a truncated prefix rewrite
    /// is never an acceptable substitute for the whole document.
    pub too_large: bool,
    /// The daily request ceiling refused the call; nothing was sent.
    pub capped: bool,
}

impl HumanizeAttempt {
    fn kept(text: String, report: ContentReport) -> Self {
        Self {
            text,
            report,
            called: false,
            reverted: false,
            failed: false,
            timed_out: false,
            too_large: false,
            capped: false,
        }
    }
}

/// One document's whole humanize attempt, with the PROVIDER CALL, the
/// deterministic projects re-normalization, and the REVALIDATION all injected
/// — the same seam shape as `super::super::repair::repair_loop`, and for the
/// same reason: the decisions here (deadline-first, empty-findings no-op,
/// usable-answer gate, accept/revert) must be provable by a test rather than
/// by reading the code, and this crate has no Tauri test harness to build a
/// real `Completer` from.
///
/// `findings` is ALREADY the filtered list of patchable flags
/// (`super::patches::flagged_lines`) — an empty list (every flag landed on a
/// link line, had no locatable line, or there were none) is a no-op, not a
/// call with nothing to ask about. It is generic: this seam only hands it back
/// to `complete`.
///
/// `complete` returns the CANDIDATE document (the stage applies the model's
/// line patches to `original_text` inside it). A patch answer that cannot be
/// read is an `Err` there and lands in the same fail-soft arm as a provider
/// error: original kept, `failed`.
///
/// `enforce_cap` applies the whole-document size cap: true when the whole
/// document is sent (rewrite mode), false for line patches (a small excerpt).
///
/// `normalize` is `|candidate| Option<String>`, exactly like `repair_loop`'s
/// own parameter: `Some` replaces the candidate with the re-rendered Projects
/// section, `None` means no change. The letter tier passes a closure that
/// always returns `None` — a letter has no Projects section to normalize.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn humanize_one<T, F, Fut, N, G, GFut>(
    deadline: RunDeadline,
    original_text: String,
    original_report: ContentReport,
    findings: Vec<T>,
    mut complete: F,
    normalize: N,
    mut revalidate: G,
    tier: HumanizeTier,
    enforce_cap: bool,
) -> AppResult<HumanizeAttempt>
where
    F: FnMut(String, Vec<T>) -> Fut,
    Fut: std::future::Future<Output = AppResult<String>>,
    N: Fn(&str) -> Option<String>,
    G: FnMut(String) -> GFut,
    GFut: std::future::Future<Output = AppResult<ContentReport>>,
{
    if enforce_cap && exceeds_humanize_cap(&original_text) {
        let mut attempt = HumanizeAttempt::kept(original_text, original_report);
        attempt.too_large = true;
        return Ok(attempt);
    }
    if deadline.passed() {
        let mut attempt = HumanizeAttempt::kept(original_text, original_report);
        attempt.timed_out = true;
        return Ok(attempt);
    }
    if findings.is_empty() {
        return Ok(HumanizeAttempt::kept(original_text, original_report));
    }

    match complete(original_text.clone(), findings).await {
        Err(error) => {
            // The daily ceiling refuses BEFORE the request: nothing was sent.
            let capped = matches!(error, AppError::RateLimited(_));
            let mut attempt = HumanizeAttempt::kept(original_text, original_report);
            attempt.called = !capped;
            attempt.failed = !capped;
            attempt.capped = capped;
            Ok(attempt)
        }
        Ok(candidate) => {
            // No patch survived: nothing to grade, nothing to revalidate.
            if candidate == original_text {
                let mut attempt = HumanizeAttempt::kept(original_text, original_report);
                attempt.called = true;
                return Ok(attempt);
            }
            if !is_usable_rewrite(&original_text, &candidate, tier) {
                let mut attempt = HumanizeAttempt::kept(original_text, original_report);
                attempt.called = true;
                return Ok(attempt);
            }
            let candidate = normalize(&candidate).unwrap_or(candidate);
            // A revalidate failure (a `spawn_blocking` join failure inside
            // `validate_documents` — the process, not the model) is a FAILED
            // ATTEMPT, exactly like a `complete()` error above: keep the
            // original, mark `failed`, never propagate. Before this, the `?`
            // here was the ONE path through `humanize_one` that could fail
            // the WHOLE pipeline run over what is, by design, this stage's
            // own best-effort cleanup pass.
            let candidate_report = match revalidate(candidate.clone()).await {
                Ok(report) => report,
                Err(_) => {
                    let mut attempt = HumanizeAttempt::kept(original_text, original_report);
                    attempt.called = true;
                    attempt.failed = true;
                    return Ok(attempt);
                }
            };
            if humanize_is_worse(
                &original_report,
                &original_text,
                &candidate_report,
                &candidate,
            ) {
                let mut attempt = HumanizeAttempt::kept(original_text, original_report);
                attempt.called = true;
                attempt.reverted = true;
                Ok(attempt)
            } else {
                Ok(HumanizeAttempt {
                    text: candidate,
                    report: candidate_report,
                    called: true,
                    reverted: false,
                    failed: false,
                    timed_out: false,
                    too_large: false,
                    capped: false,
                })
            }
        }
    }
}
