use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::credentials::CredentialStore;
use crate::db::new_job_id;
use crate::documents::{embedding_space_changed, DocumentStore, EmbeddingConfig};
use crate::error::AppResult;
use crate::events::{emit_event, JobEvent, JOBS_EVENT};
use crate::ipc_contracts::ai::AiEmbedRequest;
use crate::jobs::{JobStatus, JobTracker};
use crate::postings::PostingsCache;

use super::ai_provider::{
    emit_stream_error, ollama, resolve, resolve_by_name, AiGenerateRequest, ProviderId,
};

/// Stream an AI generation from the explicitly-selected provider.
///
/// The provider is **required and validated** — unknown/missing providers and
/// model/provider mismatches fail with a clear error. There is no silent
/// fallback to Ollama.
#[tauri::command]
pub async fn ai_generate(app: AppHandle, req: AiGenerateRequest) -> Value {
    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "ai.generate");

    let fail = |app: &AppHandle, job_id: &str, msg: String| -> Value {
        emit_stream_error(app, job_id, &msg);
        crate::commands::jobs::job_fail(app, job_id, msg);
        json!({ "jobId": job_id })
    };

    // 0. Anti-abuse: rate + concurrency cap. Rejected before any provider work so
    // a looping/XSS'd renderer can't drive unbounded paid-API spend. The guard is
    // held for the lifetime of the streamed generation (moved into the task), so
    // the in-flight slot is released exactly when generation finishes.
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    let guard = match limiter.acquire(
        "ai_generate",
        crate::limits::AI_GENERATE_RATE_MAX,
        crate::limits::AI_GENERATE_CONCURRENCY_MAX,
    ) {
        Ok(g) => g,
        Err(e) => return fail(&app, &job_id, e.to_string()),
    };

    // 1–3. Resolve the active provider from the BACKEND store (not the request):
    // provider present → known → model belongs to it, all validated inside
    // `from_active`. `base_url` can no longer be supplied by the renderer — routing
    // comes from the persisted store, closing the key-exfiltration SSRF (#16).
    let completer = match crate::pipeline::Completer::from_active(&app) {
        Ok(c) => c,
        Err(e) => return fail(&app, &job_id, e.to_string()),
    };

    // 4. Per-provider daily request ceiling — a coarse runaway-cost backstop.
    if let Err(e) = limiter.charge_provider_daily(
        completer.provider_id().as_str(),
        crate::limits::PROVIDER_DAILY_MAX,
    ) {
        return fail(&app, &job_id, e.to_string());
    }

    log::info!(
        "[ai] dispatch provider={}",
        completer.provider_id().as_str()
    );

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        // Hold the concurrency guard for the whole stream; dropped here on completion.
        let _guard = guard;
        // `stream` overwrites `req.model` with the resolved active model, so the
        // provider/model/base_url all come from the store, never the request.
        if let Err(e) = completer.stream(&job_id_clone, req).await {
            let msg = e.to_string();
            emit_stream_error(&app_clone, &job_id_clone, &msg);
            crate::commands::jobs::job_fail(&app_clone, &job_id_clone, msg);
        }
    });

    json!({ "jobId": job_id })
}

pub(crate) fn get_provider_key(app: &AppHandle, provider: &str) -> Option<String> {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    guard
        .get_decrypted(&format!("ai:{provider}"))
        .map(|(_, password)| password)
}

#[tauri::command]
pub fn ai_set_provider_key(app: AppHandle, provider: String, api_key: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.set(&format!("ai:{provider}"), "apikey", &api_key) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn ai_remove_provider_key(app: AppHandle, provider: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.remove(&format!("ai:{provider}")) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn ai_has_provider_key(app: AppHandle, provider: String) -> Value {
    json!({ "has": get_provider_key(&app, &provider).is_some() })
}

#[tauri::command]
pub async fn ai_test_provider_key(
    app: AppHandle,
    provider: String,
    base_url: Option<String>,
) -> Value {
    // The provider resolves its own credentials/transport (keychain key + client,
    // or a CLI binary check) — this command just dispatches.
    let provider_client = match resolve_by_name(&provider, base_url) {
        Ok(p) => p,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    match provider_client.test_key(&app).await {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub async fn ai_list_provider_models(
    app: AppHandle,
    provider: String,
    base_url: Option<String>,
) -> AppResult<Value> {
    let provider_client = resolve_by_name(&provider, base_url)?;
    Ok(json!(provider_client.list_models(&app).await?))
}

/// Capability probe for a provider/model. Network-free, but NOT side-effect
/// free: `supportsWebSearch` reads the OS keychain to see whether a search
/// backend is actually configured — whether it can
/// attempt a web-grounded `research*` search, whether it accepts a
/// reasoning-effort value, and (when it does) exactly which levels this
/// model accepts (drives the Settings → AI effort picker). Reads the
/// resolved [`ModelCapabilities`] matrix + [`AiProvider::effort_levels`] (the
/// SAME values consumed server-side by `ai_research_*` and by each adapter's
/// own effort-field gate), so the renderer never mirrors the per-provider
/// vocabulary or booleans: a NEW provider/model is exposed with zero
/// TypeScript change — this is a per-MODEL lookup (Gemini's accepted level
/// subset genuinely varies by model tier, not just by provider). An
/// unknown/unresolvable provider degrades to `supportsWebSearch: false`,
/// `supportsReasoning: false`, `effortLevels: []`, matching the caller's safe
/// default-off fallback.
#[tauri::command]
pub fn ai_model_capabilities(
    app: AppHandle,
    provider: String,
    model: Option<String>,
    base_url: Option<String>,
) -> Value {
    let model = model.unwrap_or_default();
    match resolve_by_name(&provider, base_url) {
        Ok(client) => {
            let caps = client.capabilities(&model);
            json!({
                // Whether research can actually RUN, not what the provider
                // advertises — see `search::research_available`. This is why the
                // command takes `app`, and why it reads stored credentials.
                "supportsWebSearch": super::ai_provider::search::research_available(
                    &app,
                    client.as_ref(),
                    &model,
                ),
                "supportsReasoning": caps.supports_reasoning,
                "effortLevels": client.effort_levels(&model),
            })
        }
        Err(_) => json!({
            "supportsWebSearch": false,
            "supportsReasoning": false,
            "effortLevels": Vec::<&str>::new(),
        }),
    }
}

/// Local (Ollama) model list — powers the model picker's "Ollama (Local)"
/// section. Cloud models come from `ai_list_provider_models`.
#[tauri::command]
pub async fn ai_list_models() -> Value {
    json!(ollama::list_tag_models().await)
}

/// Inspect a local (Ollama) model's real context window + size via `/api/show`,
/// to suggest safe generation limits. Returns `Null` when Ollama is unreachable
/// or the model has no usable info — the UI only calls this for the local provider.
#[tauri::command]
pub async fn ai_inspect_model(model: String) -> Value {
    ollama::show_model(&model).await
}

/// The outcome of [`admit_research`]: guard+completer, or WHY refused — L-2:
/// `ai_salary::ai_lookup_salary_reasoned` surfaces the reason to the model;
/// every other caller still just degrades to its own empty value on any
/// non-`Admitted`. `pub(super)`: `commands::ai_salary` (split out purely for
/// R8) reuses it — zero business-logic duplication. `#[allow]`: a
/// short-lived, immediately-destructured value, never a loop/collection —
/// boxing `Completer` would only add indirection.
#[allow(clippy::large_enum_variant)]
pub(super) enum AdmitOutcome {
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
/// Extracted because `ai_research_company`, `ai_lookup_salary` and
/// `ai_research_answer` each open with the identical ~30-line preamble and each
/// degrades to its OWN "nothing found" value rather than an error. The guard
/// rides in the returned tuple so the caller holds the slot for the real work.
/// `who` only labels the debug log.
pub(super) fn admit_research(app: &AppHandle, who: &str) -> AdmitOutcome {
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    // This is a billable provider web search (Ollama fires two calls: search +
    // synthesis) with no other ceiling, so a looping/compromised renderer
    // varying its inputs must not drive unbounded paid-API spend. One shared
    // bucket across all three research commands, deliberately.
    let guard = match limiter.acquire(
        "ai_research",
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

    let (_guard, completer) = match admit_research(&app, "research_company") {
        AdmitOutcome::Admitted(g, c) => (g, c),
        _ => return json!({ "company": "", "brief": "" }),
    };

    // Prefer the accurate AI-extracted company name from the generation flow; the
    // enricher falls back to heuristic job-ad extraction only when it's absent.
    let deadline = super::ai_provider::timeouts::research_deadline(effort.as_deref());
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
    super::ai_salary::ai_lookup_salary_reasoned(
        &app, role, company, location, country, currency, effort,
    )
    .await
    .ok()
}

#[tauri::command]
pub async fn ai_pull_model(app: AppHandle, model: String) -> Value {
    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "ai.pull_model");

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        match ollama::pull(&app_clone, &job_id_clone, &model).await {
            Ok(()) => {
                crate::commands::jobs::job_complete(
                    &app_clone,
                    &job_id_clone,
                    json!({ "model": model, "done": true }),
                );
            }
            Err(e) => {
                crate::commands::jobs::job_fail(&app_clone, &job_id_clone, e.to_string());
            }
        }
    });

    json!({ "jobId": job_id })
}

#[tauri::command]
pub fn ai_unload_model(_model: String) -> Value {
    json!({ "success": true })
}

/// Embed text using the active embedding provider/model (persisted in the
/// document store). Routes through the centralized provider layer, so the
/// returned vector is tagged with its embedding space.
#[tauri::command]
pub async fn ai_embed(app: AppHandle, req: AiEmbedRequest) -> Value {
    match crate::documents::embed(&app, &req.text).await {
        Ok(ev) => json!({
            "vector": ev.values,
            "dim": ev.space.dim,
            "provider": ev.space.provider,
            "model": ev.space.model,
        }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ── Active generation provider (backend-owned; task #16) ─────────────────────────

/// Read the active generation provider config: the active provider's resolved
/// `model`/`baseUrl` (what `useGenerateConfig` reads) plus the `providers` map
/// for the Settings AI tab. Unseeded → `activeProvider` absent (generation errors
/// "No AI provider selected", never a silent fallback). Values are returned as the
/// writer validated them; the generation egress (`Completer::from_active`)
/// defensively re-validates the base_url before use.
#[tauri::command]
pub fn ai_active_config(app: AppHandle) -> Value {
    serde_json::to_value(
        app.state::<crate::ai_config::AiConfigStore>()
            .active_config(),
    )
    .unwrap_or_else(|_| json!({ "providers": {} }))
}

/// Switch the active provider (the "switch" half of the switch-vs-edit split —
/// deliberately separate from `ai_set_provider_settings` so editing a provider's
/// settings can never silently flip which provider is active). Validates the id
/// server-side. Returns the fresh active config, or `{ error }`.
#[tauri::command]
pub fn ai_set_active_provider(app: AppHandle, provider: String) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.set_active_provider(&provider) {
        Ok(()) => serde_json::to_value(store.active_config()).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Edit a provider's model/base_url/context_window (the "edit" half — never
/// flips the active provider). Server-side validation: known id, cross-family
/// model check, base_url provenance (scheme + cloud-metadata block;
/// loopback/LAN gateways stay allowed), and the context-window range. Returns
/// the fresh active config, or `{ error }`.
///
/// PATCH semantics per field — absent keeps the stored value, explicit `null`
/// clears it, a value sets it. So a caller may send ONLY what changed, and
/// omitting a field can never erase it. See
/// [`ProviderSettingsPatch`](crate::ai_config::ProviderSettingsPatch) for why
/// the request is a struct rather than loose arguments.
#[tauri::command]
pub fn ai_set_provider_settings(
    app: AppHandle,
    req: crate::ai_config::ProviderSettingsPatch,
) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.set_provider_settings(req) {
        Ok(()) => serde_json::to_value(store.active_config()).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// One-time first-run seed from the renderer's migrated Zustand `aiProviderConfig`
/// (`{ activeProvider, providers: { [id]: { model, baseUrl } } }`). Row-presence
/// gated SERVER-side: a no-op once anything has been set, so it can never clobber a
/// later explicit change (the renderer also gates on `persist.hasHydrated()`). Bad
/// values are scrubbed, never rejected. Returns `{ seeded: bool }` or `{ error }`.
#[tauri::command]
pub fn ai_seed_active_config(app: AppHandle, config: crate::ai_config::AiConfigSnapshot) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.seed_if_empty(&config) {
        Ok(seeded) => json!({ "seeded": seeded }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ── Per-stage model overrides ────────────────────────────────────────────────

/// Read every explicitly-set per-stage model override, keyed by stage name.
///
/// ABSENT means "this stage runs on the active provider" — the read model has
/// no entry for an unconfigured stage, and the UI must render that as the
/// default rather than as an override equal to the default. Rows naming a stage
/// the current build no longer runs are filtered out server-side, so every key
/// returned is a live stage.
#[tauri::command]
pub fn ai_stage_overrides(app: AppHandle) -> Value {
    serde_json::to_value(
        app.state::<crate::ai_config::AiConfigStore>()
            .stage_overrides(),
    )
    .unwrap_or_else(|_| json!({}))
}

/// Point ONE pipeline stage at a specific provider + model.
///
/// Strict server-side validation — unknown stage, unknown provider,
/// cross-family model, bad `base_url` provenance, out-of-range context window
/// are all `{ error }`, never a silently scrubbed row: an override the user
/// cannot see the effect of is worse than a refused one. Returns the fresh
/// override map so the caller re-renders from the server's answer.
#[tauri::command]
pub fn ai_set_stage_override(
    app: AppHandle,
    stage: String,
    provider: String,
    model: Option<String>,
    base_url: Option<String>,
    context_window: Option<u32>,
) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    let over = crate::ai_config::StageOverride {
        provider,
        model: model.unwrap_or_default(),
        base_url,
        context_window,
    };
    match store.set_stage_override(&stage, over) {
        Ok(()) => serde_json::to_value(store.stage_overrides()).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Return ONE stage to the active provider. A no-op (not an error) for a stage
/// that has no override, so the UI can clear without reading first.
#[tauri::command]
pub fn ai_clear_stage_override(app: AppHandle, stage: String) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.clear_stage_override(&stage) {
        Ok(()) => serde_json::to_value(store.stage_overrides()).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ── Embeddings configuration & re-indexing ──────────────────────────────────────

/// The active embedding space, the vector counts per space, and how many
/// documents are indexed in the active space (vs. stale / unindexed).
#[tauri::command]
pub async fn ai_embedding_status(app: AppHandle) -> Value {
    let store = app.state::<DocumentStore>();
    let cfg = store.embedding_config();
    let total_docs = store.list().len();
    // SQL COUNT in the active space — never deserializes the vector blobs (the old
    // path loaded every vector via a full vector scan just to count the matching ones).
    let indexed_in_active = store.count_vectors_in_space(&cfg.provider, &cfg.model);
    let spaces: Vec<Value> = store
        .vector_space_counts()
        .into_iter()
        .map(|(s, n)| {
            json!({
                "provider": s.provider,
                "model": s.model,
                "dim": s.dim,
                "count": n,
                "active": cfg.provider == s.provider && cfg.model == s.model,
            })
        })
        .collect();
    json!({
        "active": { "provider": cfg.provider, "model": cfg.model, "baseUrl": cfg.base_url },
        "spaces": spaces,
        "documents": {
            "total": total_docs,
            "indexedInActiveSpace": indexed_in_active,
            "stale": total_docs.saturating_sub(indexed_in_active),
        },
        // Whether an embedding job is running right now (auto or manual). The
        // settings strip needs the real thing: inferring "indexing now" from the
        // auto-index PREFERENCE alone claims work is happening even when the run
        // already failed, or was never started because nothing changed.
        "indexing": running_embed_job(&app).is_some(),
    })
}

// ── AI-spend visibility ──────────────────────────────────────────────────────

/// Read-only AI-spend summary: today's REAL per-provider token totals — as
/// reported by each provider's own response, never estimated (see
/// `commands::ai_provider::stream` / `pipeline::Completer::complete`, the two
/// chokepoints that record them) — plus an ESTIMATED USD cost from a static
/// list-price rate table (`crate::spend::estimate_cost`). The dollar figure is
/// a best-effort ballpark, not a billing-accurate source: a BYO-key user has
/// no billing API to query. Local (Ollama) and CLI-agent calls always cost
/// $0. A missing store (failed to open at startup) degrades to all-zero
/// rather than erroring.
#[tauri::command]
pub fn ai_spend_summary(app: AppHandle) -> Value {
    let Some(store) = app.try_state::<crate::spend::SpendStore>() else {
        return json!({
            "today": { "inputTokens": 0, "outputTokens": 0, "estCostUsd": 0.0 },
            "perProvider": [],
            "thinkingByModel": [],
        });
    };
    let today = store.today_totals();
    let per_provider: Vec<Value> = store
        .by_provider_today()
        .into_iter()
        .map(|p| {
            json!({
                "provider": p.provider,
                "inputTokens": p.input_tokens,
                "outputTokens": p.output_tokens,
                "estCostUsd": p.est_cost_usd,
            })
        })
        .collect();
    // Observed reasoning overhead per model, over all history — the honest
    // input to "which model should run which stage". EMPTY until a provider
    // that reports a distinct thinking count has actually been used (OpenAI's
    // reasoning models, Gemini's thinking models); Anthropic and Ollama fold
    // thinking into their output count and so contribute nothing here rather
    // than a zero that would read as "this model does not reason".
    let thinking_by_model: Vec<Value> = store
        .thinking_by_model()
        .into_iter()
        .map(|m| {
            json!({
                "provider": m.provider,
                "model": m.model,
                "calls": m.calls,
                "thinkingTokens": m.thinking_tokens,
                "outputTokens": m.output_tokens,
            })
        })
        .collect();
    json!({
        "today": {
            "inputTokens": today.input_tokens,
            "outputTokens": today.output_tokens,
            "estCostUsd": today.est_cost_usd,
        },
        "perProvider": per_provider,
        "thinkingByModel": thinking_by_model,
    })
}

/// Scrub-then-validate `base_url` before it can reach persistence — the exact
/// pair `AiConfigStore::validate_settings` (`ai_config/mod.rs`) applies for
/// `ai_set_provider_settings`, extracted here as a pure, AppHandle-free
/// function so `ai_set_embedding_config` below stops being the one setter
/// that persists a renderer-supplied embedding endpoint (carrying the
/// provider API key plus résumé/job text on every embed call) unvalidated.
/// `base_url` only means anything for `OpenAiCompatible` (see `resolve`'s
/// doc comment), so it is dropped for every other provider before
/// validation; whatever survives is checked against
/// `net::ssrf::validate_provider_base_url`, which deliberately keeps
/// loopback/LAN addresses — a local LM Studio/vLLM/Ollama endpoint must keep
/// working.
fn scrub_and_validate_embedding_base_url(
    provider_id: ProviderId,
    base_url: Option<String>,
) -> AppResult<Option<String>> {
    let base_url = if matches!(provider_id, ProviderId::OpenAiCompatible) {
        base_url
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
    } else {
        None
    };
    if let Some(ref u) = base_url {
        crate::net::ssrf::validate_provider_base_url(u)?;
    }
    Ok(base_url)
}

/// Set the active embedding provider/model. The provider must support embeddings
/// (validated server-side); an empty model resolves to the provider's default.
/// Changing this changes the embedding space — call `ai_reembed_all` afterwards
/// to rebuild the index so comparisons stay valid.
#[tauri::command]
pub async fn ai_set_embedding_config(
    app: AppHandle,
    provider: String,
    model: Option<String>,
    base_url: Option<String>,
) -> Value {
    let provider_id = match ProviderId::parse(&provider) {
        Ok(p) => p,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    let base_url = match scrub_and_validate_embedding_base_url(provider_id, base_url) {
        Ok(u) => u,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    let client = resolve(provider_id, base_url.clone());
    let model = model
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .or_else(|| client.default_embedding_model().map(String::from));
    let model = match model {
        Some(m) => m,
        None => {
            return json!({
                "success": false,
                "error": format!("{} does not support embeddings.", provider_id.as_str()),
            })
        }
    };
    if !client.capabilities(&model).supports_embeddings {
        return json!({
            "success": false,
            "error": format!("{} does not support embeddings.", provider_id.as_str()),
        });
    }
    let cfg = EmbeddingConfig {
        provider: provider_id.as_str().to_string(),
        model,
        base_url,
    };
    let store = app.state::<DocumentStore>();
    // Whether this is a real space change — the posting_vectors / match_scores
    // caches key on provider+model, so their old-space rows become unreachable
    // and must be reclaimed only when the space actually changes. Decision lives
    // in `embedding_space_changed` (shared with its unit test).
    let space_changed = embedding_space_changed(&store.embedding_config(), &cfg);
    match store.set_embedding_config(&cfg) {
        Ok(()) => {
            if space_changed {
                // Evict stale-space cache rows (mirrors how `ai_reembed_all`
                // clears the live `PostingsCache` embeddings).
                store.clear_posting_vectors().ok();
                store.clear_match_scores().ok();
            }
            json!({
                "success": true,
                "config": { "provider": cfg.provider, "model": cfg.model, "baseUrl": cfg.base_url },
            })
        }
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// Whether a re-embed run should report failure (`job.failed`) rather than a
/// `job.completed` with a 0/N payload — true only when EVERY document failed
/// and there was at least one to embed. Pure + unit-tested so the bug this
/// fixes (a total failure used to still emit `job.completed`, leaving the
/// settings strip showing a stale success toast over an unchanged index)
/// can't silently regress. A run with zero documents (`failed == 0`) is not a
/// failure — there was nothing to fail at.
fn reembed_run_failed(done: u32, failed: u32) -> bool {
    done == 0 && failed > 0
}

/// Job kinds that embed documents. Both write the same vectors for the same
/// documents, so only ONE may run at a time.
const EMBED_JOB_KINDS: [&str; 2] = ["ai.reembed", "ai.indexStale"];

/// Claim the right to run an embedding job, or report the one already running.
///
/// Auto-indexing and the manual "Re-index now" button are independent triggers
/// with no knowledge of each other, so without this a background auto-index and
/// a user's button press embed the same documents concurrently — billing a cloud
/// provider twice for identical work. Enforced in the BACKEND rather than by
/// disabling the button, because that is the only place every trigger has to
/// pass through; a UI-only guard narrows the race instead of closing it.
///
/// The scan and the registration happen under ONE lock
/// ([`JobTracker::start_exclusive`]): checking first and starting after is
/// check-then-act, and two commands can both see "nothing running" before either
/// registers.
///
/// `None` means this caller owns the run; `Some(existing)` is the job to watch
/// instead — a normal outcome, not a failure, which is why this is not a
/// `Result` (see R6).
fn claim_embed_job(app: &AppHandle, job_id: &str, kind: &str) -> Option<String> {
    crate::commands::jobs::job_start_exclusive(app, job_id, kind, &EMBED_JOB_KINDS)
}

/// Whether an embedding job is running right now (for the status surface).
fn running_embed_job(app: &AppHandle) -> Option<String> {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .list()
        .iter()
        .find(|j| is_active_embed_job(&j.kind, &j.status))
        .map(|j| j.id.clone())
}

/// Whether a job record is an embedding job that has NOT finished.
///
/// Split out purely so it is testable: the callers need an `AppHandle` this
/// crate has no harness for, this needs nothing. The terminal-status half is the
/// load-bearing part — counting a COMPLETED job as active would block every
/// future index permanently after the first run. Mirrors the predicate inside
/// [`JobTracker::start_exclusive`]; a test pins the two agreeing.
fn is_active_embed_job(kind: &str, status: &JobStatus) -> bool {
    EMBED_JOB_KINDS.contains(&kind)
        && matches!(
            status,
            JobStatus::Running | JobStatus::Queued | JobStatus::Streaming
        )
}

/// Documents with no usable vector in the ACTIVE embedding space — i.e. never
/// indexed, or indexed under a different provider/model/format.
///
/// The same `EmbeddingConfig::matches` predicate `match_resume` uses to decide
/// whether it can reuse a stored vector, so "stale" means exactly the same thing
/// to the indexer and to the consumer.
fn stale_documents(app: &AppHandle) -> Vec<crate::documents::DocumentRecord> {
    let store = app.state::<DocumentStore>();
    let cfg = store.embedding_config();
    store
        .list()
        .into_iter()
        .filter(|d| {
            !store
                .get_vector(&d.id)
                .is_some_and(|v| cfg.matches(&v.space))
        })
        .collect()
}

/// Embed `docs` with the active config and write the vectors, emitting
/// `jobs:event` progress. The shared body of [`ai_reembed_all`] (every document,
/// a full rebuild) and [`ai_index_stale_documents`] (only what is missing) so the
/// two can never drift in error handling, cancellation or progress reporting.
async fn run_embed_job(
    app: AppHandle,
    job_id: String,
    docs: Vec<crate::documents::DocumentRecord>,
) {
    let app_clone = app;
    let job_id_clone = job_id;
    {
        let total = docs.len();
        let mut done = 0u32;
        let mut failed = 0u32;
        // The FIRST embedding/write error, carried through to `job_fail` so a
        // total failure surfaces its real cause instead of dying in the log
        // (e.g. an Ollama context-length overflow or a retired Gemini model).
        let mut first_error: Option<String> = None;

        // Re-embed with bounded concurrency: each document is normally one HTTP
        // round-trip, though a document longer than the provider's per-chunk cap
        // now costs several (see `ai_provider::embed_adaptive` — chunk-and-mean-
        // pool, bounded to at most `MAX_CHUNKS_PER_DOCUMENT` chunks). A small
        // fan-out here keeps the provider busy without overwhelming it (or
        // hammering a rate limit). Cancellation is honored between chunks; store
        // writes (sync) stay serialized to avoid lock contention.
        const REEMBED_CONCURRENCY: usize = 4;
        let mut was_cancelled = false;
        for chunk in docs.chunks(REEMBED_CONCURRENCY) {
            let cancelled = app_clone
                .state::<Mutex<JobTracker>>()
                .lock()
                .get(&job_id_clone)
                .map(|j| j.status == JobStatus::Cancelled)
                .unwrap_or(false);
            if cancelled {
                was_cancelled = true;
                break;
            }

            // Embed this chunk's documents concurrently, preserving order so each
            // result pairs with its document id.
            let embeds = futures::future::join_all(
                chunk
                    .iter()
                    .map(|doc| crate::documents::embed(&app_clone, &doc.text)),
            )
            .await;

            for (doc, ev) in chunk.iter().zip(embeds) {
                match ev {
                    Ok(ev) => {
                        let store = app_clone.state::<DocumentStore>();
                        match store
                            .upsert_vector(&doc.id, &ev)
                            .and_then(|_| store.set_indexed(&doc.id))
                        {
                            Ok(()) => done += 1,
                            Err(e) => {
                                log::warn!("reembed write failed for {}: {e}", doc.id);
                                first_error.get_or_insert_with(|| e.to_string());
                                failed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        first_error.get_or_insert_with(|| e.to_string());
                        failed += 1;
                    }
                }
            }

            emit_event(
                &app_clone,
                JOBS_EVENT,
                JobEvent {
                    r#type: "job.stream".to_string(),
                    job_id: job_id_clone.clone(),
                    data: Some(json!({ "done": done, "failed": failed, "total": total })),
                    ts: crate::db::now_ms() as i64,
                },
            );
        }

        // A user-cancelled job is already in Cancelled status; calling
        // job_complete would overwrite it with Completed. Bail with partial counts.
        if was_cancelled {
            return;
        }

        // Every document failed — this is a failure, not a "completed" run with
        // a 0/N count (the bug this branch fixes: the embed provider erroring
        // for every document used to still emit `job.completed`, so the
        // settings strip showed a stale "success" toast over an unchanged
        // 0/N index). Partial success still completes with the existing
        // `{reembedded, failed, total}` payload.
        if reembed_run_failed(done, failed) {
            crate::commands::jobs::job_fail(
                &app_clone,
                &job_id_clone,
                first_error.unwrap_or_else(|| "embedding failed for every document".to_string()),
            );
            return;
        }

        crate::commands::jobs::job_complete(
            &app_clone,
            &job_id_clone,
            json!({ "reembedded": done, "failed": failed, "total": total }),
        );
    }
}

/// Re-embed every document with the active embedding config, rebuilding the
/// vector index in the active space. Emits `jobs:event` progress and returns a
/// job id. Clears the live posting embedding cache so stale-space entries go too.
#[tauri::command]
pub async fn ai_reembed_all(app: AppHandle) -> Value {
    let job_id = new_job_id();
    // Already embedding (an auto-index run, or a double-click): hand back the
    // running job so the caller watches THAT instead of starting a paid duplicate.
    if let Some(existing) = claim_embed_job(&app, &job_id, "ai.reembed") {
        return json!({ "jobId": existing });
    }

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        // Drop stale live-posting embeddings so search re-embeds them.
        app_clone
            .state::<Mutex<PostingsCache>>()
            .lock()
            .clear_embeddings();

        // Snapshot documents up front so no store guard is held across awaits.
        let docs = app_clone.state::<DocumentStore>().list();
        run_embed_job(app_clone, job_id_clone, docs).await;
    });

    json!({ "jobId": job_id })
}

/// Index only the documents that have no usable vector in the active space.
///
/// The auto-index path (renderer preference `autoIndexOnUpload`): a newly
/// imported résumé, or every document after the embedding provider/model
/// changed. Deliberately NOT [`ai_reembed_all`] — that re-embeds every document
/// unconditionally, so using it here would re-bill a cloud embedding provider
/// for documents that are already correctly indexed every time one new file is
/// added.
///
/// Returns `{ "jobId": null }` when nothing is stale, so the caller can stay
/// silent instead of showing progress for a no-op run.
#[tauri::command]
pub async fn ai_index_stale_documents(app: AppHandle) -> Value {
    let docs = stale_documents(&app);
    if docs.is_empty() {
        return json!({ "jobId": Value::Null });
    }
    let job_id = new_job_id();
    // Same guard as `ai_reembed_all` — a manual re-index already covers every
    // stale document, so joining it is strictly better than racing it.
    if let Some(existing) = claim_embed_job(&app, &job_id, "ai.indexStale") {
        return json!({ "jobId": existing });
    }

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        run_embed_job(app_clone, job_id_clone, docs).await;
    });

    json!({ "jobId": job_id })
}

#[cfg(test)]
mod test;
