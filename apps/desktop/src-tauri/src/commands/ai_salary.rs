//! Salary-lookup core, split out of [`super::ai`] purely to stay under the
//! R8 module-size cap (`docs/architecture-rules.md`) — not a new domain.
//! Reuses [`super::ai::admit_research`]/[`super::ai::AdmitOutcome`] (zero
//! business-logic duplication); the public `#[tauri::command]`
//! (`super::ai::ai_lookup_salary`) stays in `ai.rs` as a thin wrapper.

use tauri::{AppHandle, Manager};

use super::ai::{admit_research, AdmitOutcome};

/// Why [`ai_lookup_salary_reasoned`] found nothing — L-2: surfaced to
/// `agent::tools_quality::lookup_salary` as a distinct `reason`; the public
/// command in `ai.rs` collapses this to `Option` (unchanged IPC contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SalaryLookupReason {
    /// The transient per-call rate/concurrency cap refused the request —
    /// retrying shortly can succeed.
    RateLimited,
    /// No active/configured AI provider could be resolved.
    ProviderUnavailable,
    /// The per-provider daily request ceiling is exhausted; it only resets at
    /// UTC midnight (round-11 fix, PR #963 — previously collapsed into
    /// `RateLimited`, which reads as "retry shortly" and is misleading for a
    /// condition that cannot succeed again this run).
    DailyBudgetExhausted,
    /// Reached, but nothing reliable: no data, a parse/validation failure, a
    /// timeout, or a currency mismatch
    /// (`salary_research::reconcile_expected_currency`).
    NoData,
}

/// [`super::ai::ai_lookup_salary`]'s core, with the failure REASON its bare
/// `Option` return discards — `pub(crate)` so `agent::tools_quality::
/// lookup_salary` can surface it (zero business-logic duplication, one core,
/// 2 callers).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn ai_lookup_salary_reasoned(
    app: &AppHandle,
    role: String,
    company: Option<String>,
    location: Option<String>,
    country: Option<String>,
    currency: Option<String>,
    effort: Option<String>,
) -> Result<crate::salary_research::SalaryRange, SalaryLookupReason> {
    use crate::pipeline::cache::KvCache;
    use crate::salary_research::SalaryResearch;

    let (_guard, completer) = match admit_research(app, "lookup_salary") {
        AdmitOutcome::Admitted(g, c) => (g, c),
        AdmitOutcome::RateLimited => return Err(SalaryLookupReason::RateLimited),
        AdmitOutcome::ProviderUnavailable => return Err(SalaryLookupReason::ProviderUnavailable),
        AdmitOutcome::DailyBudgetExhausted => return Err(SalaryLookupReason::DailyBudgetExhausted),
    };

    // Resolved once here (the sole production caller) and passed through, so
    // `SalaryResearch::enrich` stays `AppHandle`-free and unit-testable.
    let cache_state = app.try_state::<KvCache>();
    SalaryResearch
        .enrich(
            &completer,
            cache_state.as_deref(),
            &role,
            company.as_deref().unwrap_or(""),
            location.as_deref().unwrap_or(""),
            country.as_deref().unwrap_or(""),
            currency.as_deref().unwrap_or(""),
            super::ai_provider::timeouts::research_deadline(effort.as_deref()),
        )
        .await
        .ok_or(SalaryLookupReason::NoData)
}
