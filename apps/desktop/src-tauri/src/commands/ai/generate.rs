//! The two admission-controlled provider calls — `ai_generate` (streamed
//! generation) and `ai_embed` (one embedding). Split out of `commands/ai/mod.rs`
//! for R8 (issue #1280); `mod.rs` re-exports the commands, so each keeps its
//! `commands::ai::<name>` path.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::{emit_stream_error, AiGenerateRequest};
use crate::db::new_job_id;
use crate::documents::DocumentStore;
use crate::error::AppResult;
use crate::ipc_contracts::ai::AiEmbedRequest;

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

/// Admit one `ai_embed` call against the rate + concurrency cap. The
/// per-provider DAILY charge no longer lives here (see `ai_embed`'s doc
/// comment for why): it now fires once per ACTUAL provider round-trip the
/// call ends up making (`ai_provider::embed::MeteredAttempt`), not once per
/// admitted call — a single call can fan out into several chunk sends.
///
/// Cap is a parameter rather than the constant directly, purely so this is
/// testable with a tiny cap — the same reason `charge_daily_or_reject` above
/// takes `max_per_day` instead of hardcoding [`crate::limits::PROVIDER_DAILY_MAX`].
/// Production (`ai_embed`) always passes the real constants.
fn admit_embed(
    limiter: &std::sync::Arc<crate::limits::Limiter>,
    max_requests: usize,
    max_concurrent: usize,
) -> AppResult<crate::limits::ConcurrencyGuard> {
    limiter.acquire("ai_embed", max_requests, max_concurrent)
}

/// Embed text using the active embedding provider/model (persisted in the
/// document store). Routes through the centralized provider layer, so the
/// returned vector is tagged with its embedding space.
///
/// Anti-abuse, mirroring `ai_generate`: [`admit_embed`] rejects before any
/// provider work runs; the per-provider daily charge then fires once per
/// ACTUAL provider round-trip `documents::embed_with_config` makes (via
/// `MeteredAttempt`), not once per admitted call — `req.text` is clamped to
/// `MAX_JOB_DESCRIPTION_BYTES` but can still fan out into
/// `embed_adaptive`'s ~32-chunk ceiling (or more, on a context-length
/// retry), each a real billed round-trip. Charging a flat 1 unit here used
/// to undercount the daily ceiling by up to that factor (#1087 finding 1).
///
/// `embedding_config()` is read exactly ONCE into `cfg` below, and that
/// SAME snapshot feeds both the charge (which provider's daily budget) and
/// the dispatch inside `embed_with_config` (which provider actually
/// receives the request). Reading it a second time — as the old code did,
/// once here and again inside `documents::embed` — let
/// `ai_set_embedding_config` land in between and charge one provider while
/// billing another (#1087 finding 2).
#[tauri::command]
pub async fn ai_embed(app: AppHandle, req: AiEmbedRequest) -> Value {
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();

    let _guard = match admit_embed(
        &limiter,
        crate::limits::AI_EMBED_RATE_MAX,
        crate::limits::AI_EMBED_CONCURRENCY_MAX,
    ) {
        Ok(g) => g,
        Err(e) => return json!({ "error": e.to_string() }),
    };

    // ONE read, shared by the charge closure below and the dispatch inside
    // `embed_with_config` — see this function's doc comment.
    let cfg = app.state::<DocumentStore>().embedding_config();
    let charge_provider = cfg.provider.clone();
    let charge_fn =
        move || limiter.charge_provider_daily(&charge_provider, crate::limits::PROVIDER_DAILY_MAX);
    let charge: &(dyn Fn() -> AppResult<()> + Send + Sync) = &charge_fn;

    let text = crate::applications::clamp_to_bytes(
        req.text,
        crate::applications::MAX_JOB_DESCRIPTION_BYTES,
    );
    match crate::documents::embed_with_config(&app, &cfg, &text, Some(charge)).await {
        Ok(ev) => json!({
            "vector": ev.values,
            "dim": ev.space.dim,
            "provider": ev.space.provider,
            "model": ev.space.model,
        }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[cfg(test)]
mod tests;
