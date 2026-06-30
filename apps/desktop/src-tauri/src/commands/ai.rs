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

    let completer = match Completer::resolve(&app, provider.as_deref(), model.as_deref(), base_url)
    {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("research_company: provider resolution failed: {e}");
            return json!({ "company": "", "brief": "" });
        }
    };

    // Prefer the accurate AI-extracted company name from the generation flow; the
    // enricher falls back to heuristic job-ad extraction only when it's absent.
    let result = CompanyResearch
        .enrich_with(&completer, &job_ad, company.as_deref())
        .await;
    json!({ "company": result.key, "brief": result.content })
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
