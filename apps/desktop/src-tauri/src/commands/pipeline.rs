//! Generic streaming-generation command, expressed as a [`Pipeline`].
//!
//! This is the resume (and any single-shot streaming) generator: a one-stage
//! pipeline whose stage streams a completion through the centralized provider
//! (`ai:stream` + `jobId`), exactly like chat. It exists so feature generators
//! share the pipeline lifecycle and gain an insertion point for future stages
//! (e.g. a resume validator) without a bespoke architecture.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai_provider::{emit_stream_error, resolve, AiGenerateRequest, ProviderId};
use crate::db::new_job_id;
use crate::error::AppResult;
use crate::pipeline::{Pipeline, Stage};

struct GenerationContext {
    app: AppHandle,
    job_id: String,
    provider_id: ProviderId,
    req: AiGenerateRequest,
}

/// Streams a chat completion through the active provider.
struct StreamGenerateStage;

#[async_trait]
impl Stage<GenerationContext> for StreamGenerateStage {
    fn name(&self) -> &'static str {
        "generate"
    }
    async fn run(&self, ctx: &mut GenerationContext) -> AppResult<()> {
        let provider = resolve(ctx.provider_id, ctx.req.base_url.clone());
        provider.chat_stream(&ctx.app, &ctx.job_id, &ctx.req).await
    }
}

/// Stream a generation through the pipeline. Provider is required + validated
/// (no silent fallback), identical to `ai_generate`; the difference is that the
/// work runs as a `Pipeline` so stages compose consistently across features.
#[tauri::command]
pub async fn generate_pipeline(app: AppHandle, req: AiGenerateRequest) -> Value {
    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "pipeline.generate");

    let fail = |app: &AppHandle, job_id: &str, msg: String| -> Value {
        emit_stream_error(app, job_id, &msg);
        crate::commands::jobs::job_fail(app, job_id, msg);
        json!({ "jobId": job_id })
    };

    let provider_str = match req.provider.as_deref() {
        Some(p) if !p.trim().is_empty() => p.to_string(),
        _ => {
            return fail(
                &app,
                &job_id,
                "No AI provider selected. Choose a provider in Settings → AI.".to_string(),
            )
        }
    };
    let provider_id = match ProviderId::parse(&provider_str) {
        Ok(id) => id,
        Err(e) => return fail(&app, &job_id, e.to_string()),
    };
    if let Err(e) = provider_id.validate_model(&req.model) {
        return fail(&app, &job_id, e.to_string());
    }

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ctx = GenerationContext {
            app: app_clone.clone(),
            job_id: job_id_clone.clone(),
            provider_id,
            req,
        };
        let pipeline = Pipeline::new("generate").add(StreamGenerateStage);
        if let Err(e) = pipeline.run(&mut ctx).await {
            let msg = e.to_string();
            emit_stream_error(&app_clone, &job_id_clone, &msg);
            crate::commands::jobs::job_fail(&app_clone, &job_id_clone, msg);
        }
    });

    json!({ "jobId": job_id })
}
