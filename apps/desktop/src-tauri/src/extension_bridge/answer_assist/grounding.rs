//! Billable pre-compose grounding steps (salary-range lookup + opt-in
//! web-search notes) and the cancellation guards that keep an `assist.cancel`
//! from paying for one after the fact.

use crate::applications::Application;
use crate::error::{AppError, AppResult};
use crate::salary_research::SalaryRange;

use super::context::scraped_salary_range;

/// Await `fut`, ABANDONING it the moment an `assist.cancel` for this request
/// lands. Returns `None` when it was abandoned.
///
/// The pre-flight peek ([`abort_if_cancelled_early`]) is not enough alone:
/// each grounding helper makes SEVERAL provider calls internally (the salary
/// lookup does a web search then a completion), so a cancel landing after the
/// peek but before the helper finished still paid for every remaining call
/// (#1232 — observed live: a cancel acknowledged at +260ms, then a completion
/// request *started* afterwards and billed in full). Dropping the future is
/// what actually stops the spend: an in-flight reqwest future cancels its
/// HTTP request when dropped. Polling at [`CANCEL_POLL_MS`] rather than
/// plumbing a signal through `salary_research`/`commands::ai` keeps this
/// contained — those are shared by callers unrelated to this cancel.
pub(super) async fn until_cancelled<T>(
    registry: &super::super::stream::AssistStreamRegistry,
    req_id: &str,
    r#gen: u64,
    fut: impl std::future::Future<Output = T>,
) -> Option<T> {
    tokio::pin!(fut);
    loop {
        tokio::select! {
            out = &mut fut => return Some(out),
            () = tokio::time::sleep(std::time::Duration::from_millis(CANCEL_POLL_MS)) => {
                if registry.is_cancelled_early(req_id, r#gen) {
                    return None;
                }
            }
        }
    }
}

/// How often [`until_cancelled`] re-checks the cancel marker. Small enough that
/// a cancelled grounding step is abandoned well inside one provider round trip,
/// large enough to be free next to the network wait it runs alongside.
const CANCEL_POLL_MS: u64 = 150;

/// Peek the cancel marker before a BILLABLE grounding step that runs BEFORE
/// `compose_draft_stream`'s own `start_and_register` (the registry's first
/// real `register` for this request).
///
/// Every step this guards — company-brief research, salary-market lookup,
/// web-search notes — is a paid provider round trip. Without the peek, an
/// `assist.cancel` that raced ahead of registration stopped only the COMPOSE:
/// the Prep tab's "Draft salary answer" followed immediately by Cancel still
/// paid for the grounding call in full (#1232), invisibly.
///
/// Non-consuming by design: the real `register` still reaches and consumes
/// the same marker afterwards, so this is a spend guard, never a replacement
/// for that ownership handoff.
pub(super) fn abort_if_cancelled_early(
    registry: &super::super::stream::AssistStreamRegistry,
    req_id: &str,
    r#gen: u64,
) -> AppResult<()> {
    if registry.is_cancelled_early(req_id, r#gen) {
        return Err(AppError::Message("Job cancelled".to_string()));
    }
    Ok(())
}

/// Resolve the salary reference range: the matched Application's own scraped
/// range takes precedence; failing that, a bounded web-researched market
/// lookup via [`crate::salary_research::SalaryResearch`] (charging the daily
/// ceiling first). `None` on any failure/timeout/no-role — never an error.
///
/// Generic over [`crate::salary_research::SalarySearcher`] so this stays
/// `AppHandle`-free and unit-testable against a fake searcher (same reason as
/// [`super::compose::DraftComposer`]). `cache` is injected the same way — the
/// sole production caller resolves it via `app.try_state::<KvCache>()`,
/// hitting the SAME `salary_range` namespace `ai_lookup_salary` uses, so a
/// repeat role/company/currency query hits the 7-day cache instead of
/// re-spending.
pub(super) async fn resolve_salary_range<S: crate::salary_research::SalarySearcher>(
    searcher: &S,
    limiter: &crate::limits::Limiter,
    provider_id: &str,
    cache: Option<&crate::pipeline::cache::KvCache>,
    app_ctx: Option<&Application>,
) -> Option<SalaryRange> {
    if let Some(range) = scraped_salary_range(app_ctx) {
        return Some(range);
    }
    let role = app_ctx.map(|a| a.title.as_str()).unwrap_or("");
    if role.trim().is_empty() {
        return None;
    }
    let company = app_ctx.map(|a| a.company.as_str()).unwrap_or("");

    // Cache check BEFORE charging the daily provider quota (PR #1209 review): charging
    // unconditionally meant every cache HIT still burned a unit of budget, and once the budget
    // was exhausted this returned `None` before it could ever read a value already sitting in
    // the cache. A hit must cost nothing — only a genuine miss reaches the charge below.
    if let Some(range) =
        crate::salary_research::SalaryResearch.cached_range(cache, role, company, "", "")
    {
        return Some(range);
    }

    if let Err(e) = limiter.charge_provider_daily(provider_id, crate::limits::PROVIDER_DAILY_MAX) {
        tracing::debug!("answer_assist: salary lookup skipped, daily budget exceeded: {e}");
        return None;
    }
    crate::salary_research::SalaryResearch
        .enrich(
            searcher,
            cache,
            role,
            company,
            "",
            "",
            "",
            // No per-request effort at this call depth — unscaled baseline.
            crate::commands::ai_provider::timeouts::research_deadline(None),
        )
        .await
}

/// Opt-in web-search reference notes for the question — delegates to
/// [`crate::commands::ai::research_answer_core`] rather than re-implementing
/// its capability-check-BEFORE-charging order, so the two call sites can
/// never drift. Degrades to `""` (never an error) on any failure.
///
/// Generic over [`crate::commands::ai::AnswerSearcher`] so this wrapper is
/// unit-testable against a fake searcher, without a live `AppHandle`.
pub(super) async fn fetch_web_notes<S: crate::commands::ai::AnswerSearcher>(
    searcher: &S,
    limiter: &crate::limits::Limiter,
    provider_id: &str,
    question: &str,
    app_ctx: Option<&Application>,
) -> String {
    let role = app_ctx.map(|a| a.title.as_str()).unwrap_or("");
    let company = app_ctx.map(|a| a.company.as_str()).unwrap_or("");
    crate::commands::ai::research_answer_core(
        searcher,
        limiter,
        provider_id,
        question,
        role,
        company,
    )
    .await
}
