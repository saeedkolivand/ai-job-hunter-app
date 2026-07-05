//! The "prep this application" agentic flow command.
//!
//! Wires the agent controller (`crate::agent`) to real Tauri commands. For one job
//! and résumé the agent plans, researches the company, scores the résumé match,
//! drafts a cover letter, suggests interview questions, and offers to SAVE the
//! drafted cover letter — a Write tool that SUSPENDS the run for explicit user
//! confirmation (`agent_confirm`) before it persists anything. Steps stream to the
//! renderer as `agent:step` events (including `confirm_request` steps); the run
//! completes as a `jobs:event`.
//!
//! Requires a tool-capable model ([`require_tool_capable`]) and is user-cancellable
//! via `jobs_cancel` (the run's token is registered with the shared
//! [`crate::scraping::ScraperEngine`] token registry, mirroring `scrape_boards`).

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::agent::controller::{run_agent_live, AgentStep, AgentStepKind, StoppedReason};
use crate::agent::flows::PREP_APPLICATION_SYSTEM;
use crate::agent::gate::{AgentGate, Decision};
use crate::agent::tools::{fenced, prep_application_tools, ToolContext, JOB_CAP, RESUME_CAP};
use crate::commands::ai_provider::{ModelCapabilities, ProviderId};
use crate::db::new_job_id;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AGENT_STEP};
use crate::ipc_contracts::agent::{AgentConfirmRequest, AgentRunRequest};
use crate::pipeline::Completer;
use crate::scraping::ScraperEngine;

/// Prepare one job application via the agentic loop. Returns `{ jobId }`
/// immediately; the run streams `agent:step` events and finishes the job async.
///
/// Modeled on [`crate::commands::ai::ai_generate`]: acquire the anti-abuse limiter
/// (held for the whole run), validate the provider/model, start a `JobTracker`
/// entry, then spawn the loop. All validation failures fail the job with a clear
/// message and still return `{ jobId }` so the renderer can surface it.
#[tauri::command]
pub async fn agent_run(app: AppHandle, req: AgentRunRequest) -> Value {
    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "agent.run");

    let fail = |app: &AppHandle, job_id: &str, msg: String| -> Value {
        crate::commands::jobs::job_fail(app, job_id, msg);
        json!({ "jobId": job_id })
    };

    // 0. Anti-abuse: rate + concurrency cap. Held for the run's lifetime (moved into
    // the spawned task), so the in-flight slot frees exactly when the run ends. One
    // run fans out into several provider calls (each turn/tool is separately charged
    // against the per-provider daily ceiling), so admit fewer than an `ai_generate`.
    let limiter = app.state::<Arc<crate::limits::Limiter>>().inner().clone();
    let guard = match limiter.acquire(
        "agent_run",
        crate::limits::AGENT_RUN_RATE_MAX,
        crate::limits::AGENT_RUN_CONCURRENCY_MAX,
    ) {
        Ok(g) => g,
        Err(e) => return fail(&app, &job_id, e.to_string()),
    };

    // 1-2. Provider must be present, known, and own the model — the same
    // required-and-validated rule as `ai_generate` (no silent fallback).
    let provider_id = match ProviderId::parse(req.provider.trim()) {
        Ok(id) => id,
        Err(e) => return fail(&app, &job_id, e.to_string()),
    };
    if let Err(e) = provider_id.validate_model(&req.model) {
        return fail(&app, &job_id, e.to_string());
    }

    // Resolve the active provider into a Completer for the agent's own turns.
    let completer = match Completer::resolve(
        &app,
        Some(&req.provider),
        Some(&req.model),
        req.base_url.clone(),
    ) {
        Ok(c) => c,
        Err(e) => return fail(&app, &job_id, e.to_string()),
    };

    // HIGH-2 defense-in-depth: a non-tool model degrades `chat_with_tools` to a
    // single-shot answer (see the trait default), which could present a fabricated
    // match score or invented company research as if the tools actually ran. Reject
    // early with a clear message — the renderer separately disables the entry point
    // for non-tool models; this is the server-side guard.
    if let Err(e) = require_tool_capable(completer.capabilities(), &req.model) {
        return fail(&app, &job_id, e.to_string());
    }

    // Load the résumé + cached job posting to build the (untrusted, fenced) user
    // message. Both must exist — fail early with a clear message otherwise.
    let Some(resume) = app.state::<DocumentStore>().get(&req.resume_id) else {
        return fail(
            &app,
            &job_id,
            format!("resume not found: {}", req.resume_id),
        );
    };
    let Some(job_text) = crate::commands::match_resume::job_text_for(&app, &req.job_id) else {
        return fail(
            &app,
            &job_id,
            format!("job not found in cache: {}", req.job_id),
        );
    };

    // Trusted routing context threaded into the tools — provider/model/base_url/
    // job_id all come from the VALIDATED request, NEVER from model-supplied tool
    // args (job_id lets `research_company` load THIS run's own posting server-side
    // — see the LOW-1 fix in `agent::tools`).
    let ctx = ToolContext {
        provider: req.provider.clone(),
        model: req.model.clone(),
        base_url: req.base_url.clone(),
        job_id: req.job_id.clone(),
    };
    let user = build_user_message(&req.resume_id, &req.job_id, &resume.text, &job_text);

    // HIGH-1(a): register the cancel token BEFORE spawning (mirrors
    // `commands::scrape::scrape_boards`) so a fast `jobs_cancel` call arriving
    // between this return and the spawned task waking is never a no-op.
    // TODO(arch): borrowing `ScraperEngine`'s job-token registry works today
    // (`jobs_cancel` already dispatches through it for every job kind) but a
    // dedicated, domain-neutral job-cancellation registry would be the cleaner
    // long-term shape than tying agent runs to the scraper engine.
    let cancel = CancellationToken::new();
    let engine = app.state::<Arc<ScraperEngine>>().inner().clone();
    engine.register_token(&job_id, cancel.clone()).await;

    let app_task = app.clone();
    let job_id_task = job_id.clone();
    let engine_task = engine.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = guard; // release the concurrency slot when the run ends
        let tools = prep_application_tools();
        let outcome = run_agent_live(
            &app_task,
            &completer,
            &tools,
            ctx,
            PREP_APPLICATION_SYSTEM,
            &job_id_task,
            user,
            &cancel,
        )
        .await;
        engine_task.unregister_token(&job_id_task).await;

        match outcome {
            // HIGH-1(b): a cancelled run must not resurrect the job to Completed
            // nor emit the terminal Proposal step — a proposal built on a
            // deliberately-aborted run is misleading, not a finished suggestion.
            Ok(o) if o.stopped_reason == StoppedReason::Cancelled => {
                crate::commands::jobs::job_cancel(&app_task, &job_id_task);
            }
            Ok(o) => {
                // Terminal PROPOSAL step: the agent's final summary of what it
                // prepared. Any actual write already happened INSIDE the loop, gated
                // behind an explicit user confirmation (a `ConfirmRequest` step);
                // this terminal step narrates only.
                emit_event(
                    &app_task,
                    AGENT_STEP,
                    AgentStep {
                        job_id: job_id_task.clone(),
                        step: o.steps + 1,
                        text: o.final_text.clone(),
                        tools: Vec::new(),
                        denied: Vec::new(),
                        kind: AgentStepKind::Proposal,
                        confirm: None,
                    },
                );
                crate::commands::jobs::job_complete(
                    &app_task,
                    &job_id_task,
                    json!({
                        "finalText": o.final_text,
                        "steps": o.steps,
                        "stoppedReason": o.stopped_reason,
                    }),
                );
            }
            Err(e) => {
                crate::commands::jobs::job_fail(&app_task, &job_id_task, e.to_string());
            }
        }
    });

    json!({ "jobId": job_id })
}

/// Resolve a suspended Write confirmation for a running agent (the human-in-the-loop
/// confirm gate). Maps the wire request to a [`Decision`] and delivers it to the
/// blocked run via the shared [`AgentGate`]; the controller is the trust boundary
/// that re-validates any edited args before executing (content only — never
/// routing/egress; see [`crate::agent::gate`]).
///
/// Returns `{ ok: false }` — never an error, never a panic — when there is no such
/// pending call: it was already resolved, timed out, cancelled, or the id is
/// unknown. `approveEdited` with no `editedArgs` is likewise a benign `{ ok: false }`.
#[tauri::command]
pub async fn agent_confirm(app: AppHandle, req: AgentConfirmRequest) -> Value {
    let Some(decision) = map_decision(&req.decision, req.edited_args) else {
        // Malformed request (unknown token, or `approveEdited` with no args) — a
        // benign no-op, never a panic.
        return json!({ "ok": false });
    };
    let gate = app.state::<AgentGate>();
    let ok = gate.resolve(&req.job_id, &req.call_id, decision);
    json!({ "ok": ok })
}

/// Map the wire `decision` token (+ optional edited args) to a [`Decision`], or
/// `None` for a malformed request. Pure (no `AppHandle`) so the mapping rules are
/// unit-testable without the Tauri harness this crate lacks:
/// - `approveEdited` REQUIRES `editedArgs` — a missing payload is `None`, never a
///   silent plain-approve (which would execute the model's ORIGINAL args the user
///   was trying to change).
/// - an unknown token is `None` — reject without acting.
fn map_decision(decision: &str, edited_args: Option<Value>) -> Option<Decision> {
    match decision {
        "approve" => Some(Decision::Approve),
        "approveEdited" => edited_args.map(Decision::ApproveEdited),
        "deny" => Some(Decision::Deny),
        _ => None,
    }
}

/// Build the untrusted user message seeding the transcript: the résumé + job ids
/// the agent passes to the tools, plus the fenced résumé and job posting as DATA
/// (never instructions). Reuses `agent::tools`'s [`fenced`] helper + caps
/// ([`RESUME_CAP`]/[`JOB_CAP`]) — the SAME bound and fence format the tools use, so
/// the cap and the tag shape are declared in exactly one place.
fn build_user_message(resume_id: &str, job_id: &str, resume: &str, job: &str) -> String {
    format!(
        "Prepare this application. Use these exact ids when calling tools:\n\
         résumé id: {resume_id}\n\
         job id: {job_id}\n\n\
         {}\n\n\
         {}",
        fenced("candidate_resume", resume, RESUME_CAP),
        fenced("job_posting", job, JOB_CAP)
    )
}

/// Pure gate for HIGH-2: the prep flow needs native tool-calling — a non-tool
/// model would silently fall back to `chat_with_tools`'s single-shot default,
/// which could present a fabricated match score or invented company research as
/// if the tools actually ran. Extracted as a pure function (no `AppHandle`) so it
/// is unit-testable without the Tauri test harness this crate doesn't have.
fn require_tool_capable(caps: ModelCapabilities, model: &str) -> AppResult<()> {
    if caps.supports_tools {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "The prep-application flow needs a tool-capable model — {model} does not support \
             tool calling. Choose a different model in Settings → AI."
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::ai_provider::TokenParam;

    /// The seed message carries both ids (so the model can pass them to the tools)
    /// and fences the résumé + job posting as data.
    #[test]
    fn build_user_message_carries_ids_and_fences_data() {
        let msg = build_user_message("res-1", "job-9", "my résumé", "the job ad");
        assert!(msg.contains("résumé id: res-1"));
        assert!(msg.contains("job id: job-9"));
        assert!(msg.contains("<candidate_resume>\nmy résumé\n</candidate_resume>"));
        assert!(msg.contains("<job_posting>\nthe job ad\n</job_posting>"));
    }

    /// Oversized blobs are truncated to the cap so cost/context stays bounded.
    #[test]
    fn build_user_message_caps_oversized_blobs() {
        let huge = "y".repeat(20_000);
        let msg = build_user_message("r", "j", &huge, "short");
        assert!(msg.contains(&"y".repeat(8_000)));
        assert!(!msg.contains(&"y".repeat(8_001)));
    }

    /// `agent_confirm`'s decision mapping: the three valid tokens map to the right
    /// `Decision`, `approveEdited` carries the edited args through.
    #[test]
    fn map_decision_maps_the_valid_tokens() {
        assert!(matches!(
            map_decision("approve", None),
            Some(Decision::Approve)
        ));
        assert!(matches!(map_decision("deny", None), Some(Decision::Deny)));
        let edited = serde_json::json!({ "coverLetterText": "edited" });
        match map_decision("approveEdited", Some(edited.clone())) {
            Some(Decision::ApproveEdited(v)) => assert_eq!(v, edited),
            other => panic!("expected ApproveEdited, got {other:?}"),
        }
    }

    /// A malformed request maps to `None` (the command surfaces `{ ok: false }`):
    /// an unknown token, or `approveEdited` with NO edited args — the latter must
    /// NOT silently fall back to a plain approve of the original args.
    #[test]
    fn map_decision_rejects_malformed_requests() {
        assert!(map_decision("nuke", None).is_none());
        assert!(map_decision("approveEdited", None).is_none());
        assert!(map_decision("", None).is_none());
    }

    /// Minimal `ModelCapabilities` literal — every field but `supports_tools` is
    /// irrelevant to the gate under test.
    fn caps(supports_tools: bool) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: false,
            supports_tools,
            supports_json_mode: false,
            supports_embeddings: false,
            token_param: TokenParam::MaxTokens,
        }
    }

    /// HIGH-2: a tool-capable model passes the gate.
    #[test]
    fn require_tool_capable_allows_a_tool_capable_model() {
        assert!(require_tool_capable(caps(true), "gpt-4o").is_ok());
    }

    /// HIGH-2: a non-tool model is rejected with a typed `AppError::Validation` —
    /// never a bare stringly-typed error (rust-standards R6) — carrying a clear,
    /// model-naming message. The server-side guard against a silent single-shot
    /// fallback that could present fabricated tool results as if they actually ran.
    #[test]
    fn require_tool_capable_rejects_a_non_tool_model() {
        let err = require_tool_capable(caps(false), "llama3").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        assert!(err.to_string().contains("llama3"));
        assert!(err.to_string().contains("tool-capable model"));
    }
}
