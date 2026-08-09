//! Generic streaming-generation command, expressed as a [`Pipeline`].
//!
//! This is the resume (and any single-shot streaming) generator: a one-stage
//! pipeline whose stage streams a completion through the centralized provider
//! (`ai:stream` + `jobId`), exactly like chat. It exists so feature generators
//! share the pipeline lifecycle and gain an insertion point for future stages
//! (e.g. a resume validator) without a bespoke architecture.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::{emit_stream_error, AiGenerateRequest};
use crate::db::new_job_id;
use crate::error::AppResult;
use crate::pipeline::{Completer, Pipeline, Stage};

struct GenerationContext {
    job_id: String,
    /// Provider/model/base_url resolved from the backend store — the request no
    /// longer carries any of them (task #16).
    completer: Completer,
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
        // Routing is fixed by the resolved `Completer`; `stream` also overwrites
        // `req.model` with the resolved active model so nothing routes off the wire.
        ctx.completer.stream(&ctx.job_id, ctx.req.clone()).await
    }
}

/// Stream a generation through the pipeline. Provider is required + validated
/// (no silent fallback), identical to `ai_generate`; the difference is that the
/// work runs as a `Pipeline` so stages compose consistently across features.
///
/// ## Anti-abuse
///
/// This is the command the tailoring flow actually calls, so it — not just
/// `ai_generate` — has to carry the limiter. It previously carried neither the
/// rate/concurrency cap nor the per-provider daily charge, which meant the
/// documented `AI_GENERATE_CONCURRENCY_MAX` of 3 was enforced on a path the app
/// no longer used: a 2026-08-08 support bundle shows **7** concurrent streams
/// from six user-initiated runs, ending in a provider 429 that discarded nine
/// minutes of completed generation.
///
/// It uses [`Limiter::acquire_queued`] rather than `acquire`: every call here is
/// a deliberate human action (a click on Generate), so the 4th concurrent one
/// waits its turn instead of being thrown away. The wait happens inside the
/// spawned task, after the `jobId` is returned, so the IPC call still resolves
/// immediately and the renderer can show the job as queued.
#[tauri::command]
pub async fn generate_pipeline(app: AppHandle, req: AiGenerateRequest) -> Value {
    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "pipeline.generate");

    let fail = |app: &AppHandle, job_id: &str, msg: String| -> Value {
        emit_stream_error(app, job_id, &msg);
        crate::commands::jobs::job_fail(app, job_id, msg);
        json!({ "jobId": job_id })
    };

    // Resolve the active provider from the BACKEND store (not the request):
    // provider present → known → model belongs to it, all validated inside
    // `from_active`. Fail fast before spawning.
    let completer = match Completer::from_active(&app) {
        Ok(c) => c,
        Err(e) => return fail(&app, &job_id, e.to_string()),
    };

    // Per-provider daily ceiling — charged here, at admission, exactly like
    // `ai_generate`. Charging after the queue wait would let an unbounded number
    // of runs past the ceiling while they park.
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    if let Err(e) = limiter.charge_provider_daily(
        completer.provider_id().as_str(),
        crate::limits::PROVIDER_DAILY_MAX,
    ) {
        return fail(&app, &job_id, e.to_string());
    }

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        // Wait (don't reject) for a concurrency slot. `_guard` is held for the
        // whole pipeline and releases the slot on drop — including on an early
        // return or a panic inside a stage.
        let _guard = match limiter
            .acquire_queued(
                "ai_generate",
                crate::limits::AI_GENERATE_RATE_MAX,
                crate::limits::AI_GENERATE_CONCURRENCY_MAX,
                crate::limits::AI_GENERATE_QUEUE_MAX,
                // Fires only if it actually has to wait, and before the wait, so
                // the renderer can suspend its stream deadline and show the
                // position instead of an 8-minute-silent "generating".
                |ahead| crate::commands::jobs::job_queued(&app_clone, &job_id_clone, ahead),
            )
            .await
        {
            Ok((guard, parked)) => {
                if parked {
                    crate::commands::jobs::job_dequeued(&app_clone, &job_id_clone);
                }
                guard
            }
            Err(e) => {
                let msg = e.to_string();
                emit_stream_error(&app_clone, &job_id_clone, &msg);
                crate::commands::jobs::job_fail(&app_clone, &job_id_clone, msg);
                return;
            }
        };

        let mut ctx = GenerationContext {
            job_id: job_id_clone.clone(),
            completer,
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
