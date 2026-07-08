use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::credentials::CredentialStore;
use crate::db::new_job_id;
use crate::documents::{embedding_space_changed, DocumentStore, EmbeddingConfig};
use crate::events::{emit_event, JobEvent, JOBS_EVENT};
use crate::ipc_contracts::ai::AiEmbedRequest;
use crate::jobs::{JobStatus, JobTracker};
use crate::postings::PostingsCache;

use super::ai_provider::{
    emit_stream_error, ollama, resolve, resolve_by_name, AiGenerateRequest, ModelCapabilities,
    ProviderId,
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

    // 1. Provider must be present.
    let provider_str = match req.provider.as_deref() {
        Some(p) if !p.trim().is_empty() => p.to_string(),
        _ => {
            return fail(
                &app,
                &job_id,
                "No AI provider selected. Choose a provider in Settings → AI.".to_string(),
            );
        }
    };
    // 2. Provider must be known.
    let provider_id = match ProviderId::parse(&provider_str) {
        Ok(id) => id,
        Err(e) => return fail(&app, &job_id, e.to_string()),
    };
    // 3. Model must belong to the active provider.
    if let Err(e) = provider_id.validate_model(&req.model) {
        return fail(&app, &job_id, e.to_string());
    }

    // 4. Per-provider daily request ceiling — a coarse runaway-cost backstop.
    if let Err(e) =
        limiter.charge_provider_daily(provider_id.as_str(), crate::limits::PROVIDER_DAILY_MAX)
    {
        return fail(&app, &job_id, e.to_string());
    }

    log::info!(
        "[ai] dispatch provider={} model={}",
        provider_id.as_str(),
        req.model
    );

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    let base_url = req.base_url.clone();
    tauri::async_runtime::spawn(async move {
        // Hold the concurrency guard for the whole stream; dropped here on completion.
        let _guard = guard;
        let provider = resolve(provider_id, base_url);
        if let Err(e) = provider.chat_stream(&app_clone, &job_id_clone, &req).await {
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
) -> Value {
    let provider_client = match resolve_by_name(&provider, base_url) {
        Ok(p) => p,
        Err(_) => return json!([]),
    };
    json!(provider_client.list_models(&app).await)
}

/// Static, network-free capability probe for a provider/model — currently only
/// whether it can attempt a web-grounded `research*` search. Reads the resolved
/// [`ModelCapabilities`] matrix (the SAME value consumed server-side by
/// `ai_research_*`), so the renderer never mirrors the per-provider booleans: a
/// NEW provider is exposed with zero TypeScript change. Drives the
/// capability-driven default of the tailoring "search company" toggle. An
/// unknown/unresolvable provider degrades to `supportsWebSearch: false`,
/// matching the caller's safe default-off fallback.
#[tauri::command]
pub fn ai_model_capabilities(
    provider: String,
    model: Option<String>,
    base_url: Option<String>,
) -> Value {
    let supports_web_search = resolve_by_name(&provider, base_url)
        .map(|client| {
            client
                .capabilities(&model.unwrap_or_default())
                .supports_web_search
        })
        .unwrap_or(false);
    json!({ "supportsWebSearch": supports_web_search })
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
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
) -> Value {
    use crate::cover_letter::research::CompanyResearch;
    use crate::pipeline::Completer;

    // Anti-abuse: rate + concurrency cap, sharing the same "ai_research" bucket
    // as `ai_lookup_salary` — this is a billable provider web search with no
    // other ceiling, so a looping/compromised renderer varying job_ad/company
    // must not drive unbounded paid-API spend.
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
            tracing::debug!("research_company: rate limited: {e}");
            return json!({ "company": "", "brief": "" });
        }
    };

    let completer = match Completer::resolve(&app, provider.as_deref(), model.as_deref(), base_url)
    {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("research_company: provider resolution failed: {e}");
            return json!({ "company": "", "brief": "" });
        }
    };

    // Per-provider daily request ceiling — the same coarse runaway-cost
    // backstop `ai_generate`/`ai_lookup_salary` charge.
    if let Err(e) = limiter.charge_provider_daily(
        completer.provider_id().as_str(),
        crate::limits::PROVIDER_DAILY_MAX,
    ) {
        tracing::debug!("research_company: daily budget exceeded: {e}");
        return json!({ "company": "", "brief": "" });
    }

    // Prefer the accurate AI-extracted company name from the generation flow; the
    // enricher falls back to heuristic job-ad extraction only when it's absent.
    let result = CompanyResearch
        .enrich_with(&completer, &job_ad, company.as_deref())
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
/// own).
trait AnswerSearcher {
    fn capabilities(&self) -> ModelCapabilities;
    fn research_answer(
        &self,
        question: &str,
        role: &str,
        company: &str,
    ) -> impl std::future::Future<Output = crate::error::AppResult<String>> + Send;
}

impl AnswerSearcher for crate::pipeline::Completer {
    fn capabilities(&self) -> ModelCapabilities {
        crate::pipeline::Completer::capabilities(self)
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
async fn research_answer_core<S: AnswerSearcher>(
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
    if !searcher.capabilities().supports_web_search {
        tracing::debug!("research_answer: provider cannot web-search, skipping charge");
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
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
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

    let completer = match Completer::resolve(&app, provider.as_deref(), model.as_deref(), base_url)
    {
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
/// `crate::salary_research::SalaryResearch::enrich`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
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
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
) -> Option<crate::salary_research::SalaryRange> {
    use crate::pipeline::cache::KvCache;
    use crate::pipeline::Completer;
    use crate::salary_research::SalaryResearch;

    // Anti-abuse: rate + concurrency cap, mirroring `ai_generate`'s guard
    // exactly. This is a billable provider web search (Ollama fires two calls:
    // search + synthesis) with no other ceiling, so a looping/compromised
    // renderer varying inputs must not drive unbounded paid-API spend. The
    // `"ai_research"` bucket is shared with `ai_research_company`, which uses
    // the same guard.
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
            tracing::debug!("lookup_salary: rate limited: {e}");
            return None;
        }
    };

    let completer = match Completer::resolve(&app, provider.as_deref(), model.as_deref(), base_url)
    {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("lookup_salary: provider resolution failed: {e}");
            return None;
        }
    };

    // Per-provider daily request ceiling — the same coarse runaway-cost
    // backstop `ai_generate` charges; the `(day, provider)` bucket is already
    // shared across every AI command against that provider.
    if let Err(e) = limiter.charge_provider_daily(
        completer.provider_id().as_str(),
        crate::limits::PROVIDER_DAILY_MAX,
    ) {
        tracing::debug!("lookup_salary: daily budget exceeded: {e}");
        return None;
    }

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
        )
        .await
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
        Some(ev) => json!({
            "vector": ev.values,
            "dim": ev.space.dim,
            "provider": ev.space.provider,
            "model": ev.space.model,
        }),
        None => json!(null),
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
    })
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
    let base_url = base_url.filter(|s| !s.trim().is_empty());
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

/// Re-embed every document with the active embedding config, rebuilding the
/// vector index in the active space. Emits `jobs:event` progress and returns a
/// job id. Clears the live posting embedding cache so stale-space entries go too.
#[tauri::command]
pub async fn ai_reembed_all(app: AppHandle) -> Value {
    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "ai.reembed");

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
        let total = docs.len();
        let mut done = 0u32;
        let mut failed = 0u32;

        // Re-embed with bounded concurrency: each document is one HTTP round-trip,
        // so a small fan-out keeps the provider busy without overwhelming it (or
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
                    Some(ev) => {
                        let store = app_clone.state::<DocumentStore>();
                        match store
                            .upsert_vector(&doc.id, &ev)
                            .and_then(|_| store.set_indexed(&doc.id))
                        {
                            Ok(()) => done += 1,
                            Err(e) => {
                                log::warn!("reembed write failed for {}: {e}", doc.id);
                                failed += 1;
                            }
                        }
                    }
                    None => failed += 1,
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

        crate::commands::jobs::job_complete(
            &app_clone,
            &job_id_clone,
            json!({ "reembedded": done, "failed": failed, "total": total }),
        );
    });

    json!({ "jobId": job_id })
}

#[cfg(test)]
mod research_answer_tests {
    //! Unit tests for `research_answer_core` — the `AppHandle`-free heart of
    //! `ai_research_answer`. A fake `AnswerSearcher` + a real (but
    //! `AppHandle`-free) `Limiter` exercise the branching/call order that
    //! matters: capability check strictly BEFORE the daily charge, and the
    //! charge happening exactly once on the successful path. The rate-limit /
    //! provider-resolution branches in the `#[tauri::command]` wrapper itself
    //! are NOT covered here — they need a live `AppHandle`, which this crate
    //! has no mock harness for (see `AnswerSearcher`'s doc comment); the
    //! `Limiter`/`ProviderId` logic they delegate to is already covered by
    //! `limits::test` and `ai_provider::mod`'s own unit tests.

    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::commands::ai_provider::TokenParam;
    use crate::error::AppResult;
    use crate::limits::Limiter;

    struct FakeAnswerSearcher {
        supports_web_search: bool,
        response: &'static str,
        calls: AtomicUsize,
    }

    fn capabilities_with(supports_web_search: bool) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: false,
            supports_tools: false,
            supports_json_mode: false,
            supports_embeddings: false,
            supports_web_search,
            token_param: TokenParam::MaxTokens,
        }
    }

    impl AnswerSearcher for FakeAnswerSearcher {
        fn capabilities(&self) -> ModelCapabilities {
            capabilities_with(self.supports_web_search)
        }

        async fn research_answer(
            &self,
            question: &str,
            _role: &str,
            _company: &str,
        ) -> AppResult<String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(format!("{}:{question}", self.response))
        }
    }

    #[tokio::test]
    async fn a_non_searchable_provider_returns_empty_without_charging_the_daily_budget() {
        let limiter = Limiter::new();
        let searcher = FakeAnswerSearcher {
            supports_web_search: false,
            response: "notes",
            calls: AtomicUsize::new(0),
        };

        let result =
            research_answer_core(&searcher, &limiter, "openai", "question?", "role", "co").await;

        assert_eq!(result, "");
        assert_eq!(
            searcher.calls.load(Ordering::SeqCst),
            0,
            "the search itself must never run for a non-searchable provider"
        );
        // The daily budget must be untouched: a fresh max=1 charge still succeeds.
        assert!(
            limiter.charge_provider_daily("openai", 1).is_ok(),
            "skipping a non-searchable provider must not consume the daily budget"
        );
    }

    #[tokio::test]
    async fn a_searchable_provider_charges_the_daily_budget_then_returns_the_search_result() {
        let limiter = Limiter::new();
        let searcher = FakeAnswerSearcher {
            supports_web_search: true,
            response: "notes",
            calls: AtomicUsize::new(0),
        };

        let result =
            research_answer_core(&searcher, &limiter, "openai", "question?", "role", "co").await;

        assert_eq!(result, "notes:question?");
        assert_eq!(searcher.calls.load(Ordering::SeqCst), 1);
        // The daily budget WAS charged: a max=1 charge for the same provider
        // now trips (this call already consumed the one slot).
        assert!(
            limiter.charge_provider_daily("openai", 1).is_err(),
            "a successful search must charge the daily budget exactly once"
        );
    }

    #[tokio::test]
    async fn a_search_failure_degrades_to_empty_after_already_charging() {
        struct ErrSearcher;
        impl AnswerSearcher for ErrSearcher {
            fn capabilities(&self) -> ModelCapabilities {
                capabilities_with(true)
            }
            async fn research_answer(
                &self,
                _question: &str,
                _role: &str,
                _company: &str,
            ) -> AppResult<String> {
                Err(crate::error::AppError::Provider(
                    "search failed".to_string(),
                ))
            }
        }

        let limiter = Limiter::new();
        let result =
            research_answer_core(&ErrSearcher, &limiter, "openai", "question?", "role", "co").await;

        assert_eq!(result, "");
    }

    // ── truncate_question ────────────────────────────────────────────────────

    #[test]
    fn truncate_question_caps_at_the_question_specific_max() {
        let long = "a".repeat(ANSWER_QUESTION_MAX_CHARS + 500);
        assert_eq!(
            truncate_question(&long).chars().count(),
            ANSWER_QUESTION_MAX_CHARS
        );
    }

    #[test]
    fn truncate_question_is_a_no_op_under_the_cap() {
        assert_eq!(
            truncate_question("Why do you want this role?"),
            "Why do you want this role?"
        );
    }

    #[test]
    fn truncate_question_preserves_a_full_question_past_the_smaller_role_company_cap() {
        // The whole point of this fix: a real custom question longer than
        // `salary_research::MAX_INPUT_CHARS` (200) — but still under this
        // question-specific cap — must survive intact, unlike the old shared
        // 200-char cap which would have cut it mid-sentence.
        let question: String = "word ".repeat(60); // 300 chars, > 200 and < 700.
        assert_eq!(truncate_question(&question), question);
        assert!(question.chars().count() > crate::salary_research::MAX_INPUT_CHARS);
    }

    #[test]
    fn truncate_question_never_splits_a_multi_byte_character() {
        let long: String = "日".repeat(ANSWER_QUESTION_MAX_CHARS + 200);
        let truncated = truncate_question(&long);
        assert_eq!(truncated.chars().count(), ANSWER_QUESTION_MAX_CHARS);
        assert!(truncated.chars().all(|c| c == '日'));
    }
}
