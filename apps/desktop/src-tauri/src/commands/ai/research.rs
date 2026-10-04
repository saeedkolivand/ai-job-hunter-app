//! Web-grounded research — company brief, application-answer notes and the salary
//! lookup — plus the shared admission (`admit_research`) and the `AnswerSearcher`
//! seam they run behind. Split out of `commands/ai/mod.rs` for R8 (issue #1280);
//! `mod.rs` re-exports everything, so each item keeps its `commands::ai::<name>`
//! path.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

/// The outcome of [`admit_research`]: guard+completer, or WHY refused — L-2:
/// `ai_salary::ai_lookup_salary_reasoned`, the sole caller, surfaces the
/// reason to the model rather than collapsing every refusal to one empty
/// value. `pub(in crate::commands)`: `commands::ai_salary` (split out purely for R8)
/// reuses it — zero business-logic duplication. `#[allow]`: a short-lived,
/// immediately-destructured value, never a loop/collection — boxing
/// `Completer` would only add indirection.
#[allow(clippy::large_enum_variant)]
pub(in crate::commands) enum AdmitOutcome {
    Admitted(crate::limits::ConcurrencyGuard, crate::pipeline::Completer),
    /// The transient per-call rate/concurrency cap refused the request —
    /// retrying shortly can succeed.
    RateLimited,
    /// No active/configured AI provider could be resolved.
    ProviderUnavailable,
    /// The per-provider DAILY request ceiling is exhausted (round-11 fix, PR
    /// #963). Previously collapsed into `RateLimited` below, which told
    /// `SalaryLookupReason`/`lookup_salary`'s tool envelope — and so the
    /// agent — that a condition which only resets at UTC midnight was worth
    /// retrying this run.
    DailyBudgetExhausted,
}

/// Admit one `"ai_research"` call: rate + concurrency cap, resolve the active
/// provider, then charge the per-provider daily ceiling — in that order, so a
/// rejected call costs no budget.
///
/// Extracted for [`ai_salary::ai_lookup_salary_reasoned`](super::ai_salary),
/// which — unlike `ai_research_company` below — has no cache check ahead of
/// its own real provider call, so charging the daily ceiling eagerly at
/// admission is correct for it (no cache hit could ever be mischarged).
/// `ai_research_company` deliberately does NOT call this function: it shares
/// [`crate::cover_letter::research::CompanyResearch::enrich_with`] with the
/// résumé pipeline's cover-letter research stage, and that shared enricher
/// checks its own cache before ever charging the daily ceiling — so it admits
/// through [`crate::pipeline::Completer::admit_research`] instead (rate +
/// concurrency only), the same bucket, so a cache hit there never touches
/// today's budget. `ai_research_answer` also does not call this function (it
/// hand-rolls its own rate/concurrency admission with no daily charge at
/// all — a pre-existing, separate shape this fix does not touch). The guard
/// rides in the returned tuple so the caller holds the slot for the real work.
/// `who` only labels the debug log.
pub(in crate::commands) fn admit_research(app: &AppHandle, who: &str) -> AdmitOutcome {
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    // This is a billable provider web search (Ollama fires two calls: search +
    // synthesis) with no other ceiling, so a looping/compromised renderer
    // varying its inputs must not drive unbounded paid-API spend. One shared
    // bucket across every research caller, deliberately.
    let guard = match limiter.acquire(
        crate::limits::AI_RESEARCH_BUCKET,
        crate::limits::AI_RESEARCH_RATE_MAX,
        crate::limits::AI_RESEARCH_CONCURRENCY_MAX,
    ) {
        Ok(g) => g,
        Err(e) => {
            tracing::debug!("{who}: rate limited: {e}");
            return AdmitOutcome::RateLimited;
        }
    };
    // Backend-owned routing (task #16): the active provider comes from the store.
    let completer = match crate::pipeline::Completer::from_active(app) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("{who}: provider resolution failed: {e}");
            return AdmitOutcome::ProviderUnavailable;
        }
    };
    // Per-provider daily ceiling — the same coarse runaway-cost backstop
    // `ai_generate` charges; the `(day, provider)` bucket is shared across every
    // AI command against that provider.
    if let Some(rejected) = charge_daily_or_reject(
        &limiter,
        completer.provider_id().as_str(),
        crate::limits::PROVIDER_DAILY_MAX,
        who,
    ) {
        return rejected;
    }
    AdmitOutcome::Admitted(guard, completer)
}

/// The daily-charge half of [`admit_research`], pulled out pure over
/// `&Limiter` (no `AppHandle`) so the round-11 fix it exists to make —
/// exhausting the daily ceiling must report [`AdmitOutcome::DailyBudgetExhausted`],
/// never silently collapse into the same [`AdmitOutcome::RateLimited`] the
/// transient per-call cap above uses — is unit-tested without a live
/// `AppHandle` (this crate has no `tauri::test` mock-app harness; see
/// `research_answer_tests`' doc for the same constraint). `None` means the
/// charge succeeded and admission should proceed.
fn charge_daily_or_reject(
    limiter: &crate::limits::Limiter,
    provider: &str,
    max_per_day: u32,
    who: &str,
) -> Option<AdmitOutcome> {
    if let Err(e) = limiter.charge_provider_daily(provider, max_per_day) {
        tracing::debug!("{who}: daily budget exceeded: {e}");
        return Some(AdmitOutcome::DailyBudgetExhausted);
    }
    None
}

/// Research the company named in a job ad and return a short factual brief for
/// the cover-letter "fit" paragraph. Reuses the shared [`CompanyResearch`]
/// enricher — the **active provider's own** web search + synthesis, cached for a
/// week — so cover-letter generation and application-question answers share
/// **one** research path. Degrades gracefully — an empty brief, never an error,
/// when the provider can't search (e.g. Ollama with no account key) or the
/// search/synthesis fails — so generation always proceeds.
///
/// Returns `{ company, brief }`. The brief is reference context only; the prompt
/// layer treats it as untrusted and never as a source of candidate facts.
#[tauri::command]
pub async fn ai_research_company(
    app: AppHandle,
    job_ad: String,
    company: Option<String>,
    // AI-extracted job title. Like `company`, it beats the heuristic, whose last
    // resort is the ad's first short line — an apply button on a scraped page.
    role: Option<String>,
    // Sizes the research deadline (`timeouts::research_deadline`): flat 25s meant
    // a reasoning model's research never finished — six for six in one session.
    effort: Option<String>,
) -> Value {
    use crate::cover_letter::research::CompanyResearch;

    // Deliberately NOT the `admit_research`/`AdmitOutcome` free function below
    // (which also eagerly charges the daily ceiling — still correct for
    // `ai_lookup_salary`/`ai_research_answer`, which have no cache check of
    // their own ahead of the real call). This command shares
    // `CompanyResearch::enrich_with` with the résumé pipeline's cover-letter
    // research stage, and `enrich_with` checks its cache BEFORE charging the
    // daily budget — so this admits through the SAME `Completer::admit_research`
    // (rate/concurrency only) the pipeline stage uses, rather than a second,
    // eagerly-charging implementation that would have burned a day's
    // allowance on every cache hit.
    let completer = match crate::pipeline::Completer::from_active(&app) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("research_company: provider resolution failed: {e}");
            return json!({ "company": "", "brief": "" });
        }
    };
    let Some(_guard) = completer.admit_research("research_company") else {
        return json!({ "company": "", "brief": "" });
    };

    // Prefer the accurate AI-extracted company name from the generation flow; the
    // enricher falls back to heuristic job-ad extraction only when it's absent.
    let deadline = crate::commands::ai_provider::timeouts::research_deadline(effort.as_deref());
    let result = CompanyResearch
        .enrich_with(
            &completer,
            &job_ad,
            company.as_deref(),
            role.as_deref(),
            deadline,
        )
        .await;
    json!({ "company": result.key, "brief": result.content })
}

/// Abstraction over "search the web for reference notes on this application
/// question" — mirrors
/// [`salary_research::SalarySearcher`](crate::salary_research::SalarySearcher)
/// exactly, and for the identical reason: this crate has no `tauri::test`
/// mock-app harness, so a fake `AnswerSearcher` is the only way to unit-test
/// [`research_answer_core`]'s capability-check-BEFORE-daily-charge ordering
/// without a live `AppHandle`. [`Completer`](crate::pipeline::Completer) is
/// the sole production implementation (both methods are thin forwards to its
/// own). `pub(crate)` — `extension_bridge::answer_assist::fetch_web_notes`
/// delegates to [`research_answer_core`] over this SAME trait rather than
/// re-implementing its capability-check-before-charging order, so the two
/// call sites can never drift.
pub(crate) trait AnswerSearcher {
    /// Whether a search backend is actually CONFIGURED — not whether the
    /// provider advertises one. Was `capabilities().supports_web_search`, which
    /// answered the wrong question in both directions: it skipped a keyless
    /// Ollama install that has a configured fallback backend, and it admitted
    /// (and charged for) one that has neither.
    fn research_available(&self) -> bool;
    fn research_answer(
        &self,
        question: &str,
        role: &str,
        company: &str,
    ) -> impl std::future::Future<Output = crate::error::AppResult<String>> + Send;
}

impl AnswerSearcher for crate::pipeline::Completer {
    fn research_available(&self) -> bool {
        crate::pipeline::Completer::research_available(self)
    }

    async fn research_answer(
        &self,
        question: &str,
        role: &str,
        company: &str,
    ) -> crate::error::AppResult<String> {
        crate::pipeline::Completer::research_answer(self, question, role, company).await
    }
}

/// Cap on the QUESTION forwarded to the web-search query — deliberately larger
/// than `salary_research::MAX_INPUT_CHARS` (200, still used below for
/// `role`/`company`): a full/custom application question is prose, and a
/// 200-char cut lands mid-sentence and hurts search relevance. Not folded into
/// `salary_research::truncate_input` — that would churn its many existing call
/// sites/tests for one extra caller; revisit if a third caller needs
/// char-capping.
const ANSWER_QUESTION_MAX_CHARS: usize = 700;

/// Char-boundary-safe cap, mirroring `salary_research::truncate_input`'s
/// implementation (`.chars().take(n)` never splits a multi-byte character).
/// Pure + unit-tested.
fn truncate_question(s: &str) -> String {
    s.chars().take(ANSWER_QUESTION_MAX_CHARS).collect()
}

/// Core of [`ai_research_answer`]: capability pre-check (BEFORE charging) →
/// the per-provider daily charge → truncate → search. Factored out of the
/// `#[tauri::command]` so this ordering is unit-tested against a fake
/// [`AnswerSearcher`] + a real (`AppHandle`-free)
/// [`Limiter`](crate::limits::Limiter), without a live `AppHandle`/`Completer`.
///
/// Degrades gracefully at every step — an empty string, never an error, when
/// the provider can't search (e.g. Ollama with no account key), the daily
/// budget is exhausted, or the search fails, so answer generation always
/// proceeds exactly as without web search.
///
/// `pub(crate)` — `extension_bridge::answer_assist::fetch_web_notes` is the
/// one other caller (the opt-in web-search notes for `answer.assist`), reusing
/// this exact function rather than a second hand-copy of its ordering.
pub(crate) async fn research_answer_core<S: AnswerSearcher>(
    searcher: &S,
    limiter: &crate::limits::Limiter,
    provider: &str,
    question: &str,
    role: &str,
    company: &str,
) -> String {
    // Capability pre-check BEFORE charging: unlike `ai_research_company`
    // (charged once per generation), this fires once PER SELECTED QUESTION —
    // a provider that can never search (e.g. a generic OpenAI-compatible
    // gateway) would otherwise burn one daily-budget charge per question for
    // a guaranteed-empty result. Justified divergence from the company-research
    // charge order given that N× fan-out.
    if !searcher.research_available() {
        tracing::debug!("research_answer: no search backend configured, skipping charge");
        return String::new();
    }

    // Per-provider daily request ceiling — the same coarse runaway-cost
    // backstop `ai_generate`/`ai_research_company` charge. The renderer also
    // caps how many questions per generation run request a search at all
    // (`WEB_SEARCH_MAX_PER_RUN` in `useApplicationAnswers.ts`), so this fan-out
    // can't dominate the shared `(day, provider)` budget on its own.
    if let Err(e) = limiter.charge_provider_daily(provider, crate::limits::PROVIDER_DAILY_MAX) {
        tracing::debug!("research_answer: daily budget exceeded: {e}");
        return String::new();
    }

    // Cap forwarded strings (token-cost hygiene, not a security boundary).
    let question = truncate_question(question.trim());
    let role = crate::salary_research::truncate_input(role.trim());
    let company = crate::salary_research::truncate_input(company.trim());

    searcher
        .research_answer(&question, &role, &company)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!("research_answer: web search failed: {e}");
            String::new()
        })
}

/// Web-search reference notes for a single application-question answer,
/// combining the question with the role + company for relevance. Reuses the
/// **same** web-search channel as [`ai_research_company`] — the active
/// provider's own web search, or the Ollama Web Search API for the Ollama
/// family — via [`Completer::research_answer`](crate::pipeline::Completer::research_answer).
/// Not cached (unlike company research): every question is different, so
/// there is nothing to key a cache on.
///
/// Degrades gracefully — an empty string, never an error, when the provider
/// can't search (e.g. Ollama with no account key) or the search fails, so
/// answer generation always proceeds exactly as without web search. The
/// returned notes are reference context only; the prompt layer fences them as
/// untrusted and never lets them write the answer.
#[tauri::command]
pub async fn ai_research_answer(
    app: AppHandle,
    question: String,
    role: Option<String>,
    company: Option<String>,
) -> String {
    use crate::pipeline::Completer;

    // Anti-abuse: rate + concurrency cap, sharing the same "ai_research" bucket
    // as `ai_research_company`/`ai_lookup_salary` — this is a billable provider
    // web search fanned out per selected question, so a looping/compromised
    // renderer must not drive unbounded paid-API spend.
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    let _guard = match limiter.acquire(
        "ai_research",
        crate::limits::AI_RESEARCH_RATE_MAX,
        crate::limits::AI_RESEARCH_CONCURRENCY_MAX,
    ) {
        Ok(g) => g,
        Err(e) => {
            tracing::debug!("research_answer: rate limited: {e}");
            return String::new();
        }
    };

    // Backend-owned routing (task #16): the active provider comes from the store.
    let completer = match Completer::from_active(&app) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("research_answer: provider resolution failed: {e}");
            return String::new();
        }
    };

    let provider_id = completer.provider_id().as_str();
    research_answer_core(
        &completer,
        &limiter,
        provider_id,
        &question,
        role.as_deref().unwrap_or(""),
        company.as_deref().unwrap_or(""),
    )
    .await
}

/// Web-grounded market salary-range lookup for the salary application question
/// (C2). Reuses the shared `SalaryResearch` enricher — the active provider's own
/// web search, parsed and strictly validated, cached for a week. Degrades
/// gracefully: returns `None` (never an error) whenever the provider can't
/// search, the search yields nothing reliable, or times out — so the salary
/// answer always falls back to grounding in the applicant's own stated
/// expectation alone. Only validated integers + a sanitized currency code are
/// ever returned; raw web text never crosses this boundary. `country`/
/// `currency` (resolved client-side from the job's validated ISO country)
/// ground the reported currency so a blank/weak `location` can't let the
/// model default to USD or hallucinate a currency — see
/// `crate::salary_research::SalaryResearch::enrich`. Thin wrapper over
/// `ai_salary::ai_lookup_salary_reasoned` (see its doc for the fuller reason
/// this command's bare `Option` discards).
#[tauri::command]
pub async fn ai_lookup_salary(
    app: AppHandle,
    role: String,
    company: Option<String>,
    location: Option<String>,
    // ISO-3166 alpha-2 job country, when known — grounds `currency` below.
    country: Option<String>,
    // Authoritative ISO-4217 currency for `country` (resolved client-side via
    // `countryToCurrency`); `None` when the country is unknown, which
    // preserves the unconstrained "local currency for that location"
    // behavior.
    currency: Option<String>,
    // Sizes the research deadline (`timeouts::research_deadline`).
    effort: Option<String>,
) -> Option<crate::salary_research::SalaryRange> {
    crate::commands::ai_salary::ai_lookup_salary_reasoned(
        &app, role, company, location, country, currency, effort,
    )
    .await
    .ok()
}

#[cfg(test)]
mod tests;
