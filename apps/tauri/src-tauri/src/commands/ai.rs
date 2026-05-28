use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::credentials::CredentialStore;
use crate::documents::{DocumentStore, EmbeddingConfig};
use crate::ipc_contracts::ai::AiEmbedRequest;
use crate::jobs::{JobStatus, JobTracker};
use crate::postings::PostingsCache;

use super::ai_provider::{
    emit_stream_error, ollama, resolve, resolve_by_name, AiGenerateRequest, ProviderId,
};

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

/// Stream an AI generation from the explicitly-selected provider.
///
/// The provider is **required and validated** — unknown/missing providers and
/// model/provider mismatches fail with a clear error. There is no silent
/// fallback to Ollama.
#[tauri::command]
pub async fn ai_generate(app: AppHandle, req: AiGenerateRequest) -> Value {
    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .start(&job_id, "ai.generate");

    let fail = |app: &AppHandle, job_id: &str, msg: String| -> Value {
        emit_stream_error(app, job_id, &msg);
        app.state::<Mutex<JobTracker>>().lock().fail(job_id, msg);
        json!({ "jobId": job_id })
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

    log::info!(
        "[ai] dispatch provider={} model={}",
        provider_id.as_str(),
        req.model
    );

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    let base_url = req.base_url.clone();
    tauri::async_runtime::spawn(async move {
        let provider = resolve(provider_id, base_url);
        if let Err(e) = provider.chat_stream(&app_clone, &job_id_clone, &req).await {
            let msg = e.to_string();
            emit_stream_error(&app_clone, &job_id_clone, &msg);
            app_clone
                .state::<Mutex<JobTracker>>()
                .lock()
                .fail(&job_id_clone, msg);
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
    let provider_client = match resolve_by_name(&provider, base_url) {
        Ok(p) => p,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    let api_key = match get_provider_key(&app, &provider) {
        Some(k) => k,
        None => return json!({ "success": false, "error": "No API key found" }),
    };
    let client = match crate::net::http::build_client(crate::net::http::ClientConfig {
        timeout: Some(std::time::Duration::from_secs(10)),
        ..Default::default()
    }) {
        Ok(c) => c,
        Err(e) => {
            return json!({ "success": false, "error": format!("Failed to create client: {}", e) })
        }
    };

    match provider_client.test_key(&client, &api_key).await {
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
    let api_key = match get_provider_key(&app, &provider) {
        Some(k) => k,
        None => return json!([]),
    };
    let client = match crate::net::http::build_client(crate::net::http::ClientConfig {
        timeout: Some(std::time::Duration::from_secs(10)),
        ..Default::default()
    }) {
        Ok(c) => c,
        Err(_) => return json!([]),
    };

    json!(provider_client.list_models(&client, &api_key).await)
}

/// Local (Ollama) model list — powers the model picker's "Ollama (Local)"
/// section. Cloud models come from `ai_list_provider_models`.
#[tauri::command]
pub async fn ai_list_models() -> Value {
    let client = match crate::net::http::build_client(crate::net::http::ClientConfig {
        timeout: Some(std::time::Duration::from_secs(5)),
        ..Default::default()
    }) {
        Ok(c) => c,
        Err(_) => return json!([]),
    };
    json!(ollama::list_tag_models(&client).await)
}

#[tauri::command]
pub async fn ai_pull_model(app: AppHandle, model: String) -> Value {
    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .start(&job_id, "ai.pull_model");

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        match ollama::pull(&app_clone, &job_id_clone, &model).await {
            Ok(()) => {
                app_clone
                    .state::<Mutex<JobTracker>>()
                    .lock()
                    .complete(&job_id_clone, json!({ "model": model, "done": true }));
            }
            Err(e) => {
                app_clone
                    .state::<Mutex<JobTracker>>()
                    .lock()
                    .fail(&job_id_clone, e.to_string());
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
    let indexed_in_active = store
        .all_vectors()
        .iter()
        .filter(|(_, ev)| cfg.matches(&ev.space))
        .count();
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
    match app.state::<DocumentStore>().set_embedding_config(&cfg) {
        Ok(()) => json!({
            "success": true,
            "config": { "provider": cfg.provider, "model": cfg.model, "baseUrl": cfg.base_url },
        }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// Re-embed every document with the active embedding config, rebuilding the
/// vector index in the active space. Emits `jobs:event` progress and returns a
/// job id. Clears the live posting embedding cache so stale-space entries go too.
#[tauri::command]
pub async fn ai_reembed_all(app: AppHandle) -> Value {
    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .start(&job_id, "ai.reembed");

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

        for doc in docs {
            let cancelled = app_clone
                .state::<Mutex<JobTracker>>()
                .lock()
                .get(&job_id_clone)
                .map(|j| j.status == JobStatus::Cancelled)
                .unwrap_or(false);
            if cancelled {
                break;
            }

            match crate::documents::embed(&app_clone, &doc.text).await {
                Some(ev) => {
                    let store = app_clone.state::<DocumentStore>();
                    let _ = store.upsert_vector(&doc.id, &ev);
                    let _ = store.set_indexed(&doc.id);
                    done += 1;
                }
                None => failed += 1,
            }

            let _ = app_clone.emit(
                "jobs:event",
                json!({ "type": "job.stream", "jobId": job_id_clone, "data": { "done": done, "failed": failed, "total": total } }),
            );
        }

        app_clone.state::<Mutex<JobTracker>>().lock().complete(
            &job_id_clone,
            json!({ "reembedded": done, "failed": failed, "total": total }),
        );
        let _ = app_clone.emit(
            "jobs:event",
            json!({ "type": "job.completed", "jobId": job_id_clone, "data": { "reembedded": done, "failed": failed, "total": total } }),
        );
    });

    json!({ "jobId": job_id })
}
