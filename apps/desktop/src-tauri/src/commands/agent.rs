//! The agentic-flow commands.
//!
//! Wires the agent controller (`crate::agent`) to real Tauri commands. ONE
//! command starts every flow: the request's `kind` selects one entry of the
//! backend-owned registry (`crate::agent::flows::FLOWS`), which is what supplies
//! the run's prompt, tool whitelist and budget — an unregistered kind fails the
//! run rather than falling back to the default. Two flows ship today:
//!
//! * `prep_application` — for one job and résumé the agent plans, researches the
//!   company, scores the match, drafts a cover letter and a résumé, suggests
//!   interview questions, and offers to SAVE both.
//! * `improve_resume` — reviews the résumé already generated for the job against
//!   its quality report and the candidate's evidence, and offers a corrected
//!   version.
//!
//! Every save is a Write tool that SUSPENDS the run for explicit user
//! confirmation (`agent_confirm`) before it persists anything. Steps stream to the
//! renderer as `agent:step` events (including `confirm_request` steps); the run
//! completes as a `jobs:event`.
//!
//! Requires a tool-capable model ([`require_tool_capable`]) and is user-cancellable
//! via `jobs_cancel` (the run's token is registered with the shared, domain-neutral
//! [`crate::jobs::cancel::CancelRegistry`] the scraper engine also dispatches
//! through, mirroring `scrape_boards`).

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::agent::controller::{run_agent_live, AgentStep, AgentStepKind, StoppedReason};
use crate::agent::flows;
use crate::agent::gate::{AgentGate, Decision};
use crate::agent::tools::{fenced, ToolContext, JOB_CAP, RESUME_CAP};
use crate::agent::tools_pipeline::{generation_for_job, GenerationLookup};
use crate::commands::ai_provider::ModelCapabilities;
use crate::db::new_job_id;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AGENT_STEP};
use crate::ipc_contracts::agent::{AgentConfirmRequest, AgentRunRequest};
use crate::jobs::cancel::CancelRegistry;
use crate::pipeline::Completer;

/// Fail the run: mark the job Failed and release its cancel-token registration.
/// Shared by every validation step that now runs INSIDE the spawned task (see
/// `agent_run`'s fix note) so each one doesn't hand-roll the same two calls.
async fn fail_run(app: &AppHandle, cancels: &CancelRegistry, job_id: &str, msg: String) {
    crate::commands::jobs::job_fail(app, job_id, msg);
    cancels.unregister(job_id).await;
}

/// Start ONE agentic run over a job + résumé — which one is `req.kind`
/// ([`flows::flow_for`]). Returns `{ jobId }` immediately; the run streams
/// `agent:step` events and finishes the job async.
///
/// Modeled on [`crate::commands::ai::ai_generate`]: acquire the anti-abuse limiter,
/// register the cancel token, then spawn the loop. EVERY fail-able step — the flow
/// lookup, `Completer::from_active` (backend-owned provider resolution +
/// validation), the tool-capability check, and loading the résumé + cached job
/// posting (+ the generation under review, for a flow that reviews one) — runs
/// INSIDE the spawned task, alongside the loop itself, so no terminal `jobs:event`
/// can ever fire before this function returns `{ jobId }`. That return is the renderer's ONLY source of the job id; it
/// starts filtering `jobs:event`/`agent:step` by that id only afterwards, so a
/// terminal event emitted synchronously (as validation failures used to, via a
/// `fail` closure called before the `json!` return) was silently dropped and the
/// run looked stuck at pending forever. The one exception is the limiter's
/// rate/concurrency rejection: it still fails synchronously (before the cancel
/// token even exists to register), which the renderer separately reconciles.
#[tauri::command]
pub async fn agent_run(app: AppHandle, req: AgentRunRequest) -> Value {
    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "agent.run");

    // 0. Anti-abuse: rate + concurrency cap. Held for the run's lifetime (moved into
    // the spawned task), so the in-flight slot frees exactly when the run ends. One
    // run fans out into several provider calls (each turn/tool is separately charged
    // against the per-provider daily ceiling), so admit fewer than an `ai_generate`.
    // This is the one remaining SYNCHRONOUS fail path (see the fn doc above): it
    // runs before the cancel token exists, so there is nothing to unregister.
    let limiter = app.state::<Arc<crate::limits::Limiter>>().inner().clone();
    let guard = match limiter.acquire(
        "agent_run",
        crate::limits::AGENT_RUN_RATE_MAX,
        crate::limits::AGENT_RUN_CONCURRENCY_MAX,
    ) {
        Ok(g) => g,
        Err(e) => {
            crate::commands::jobs::job_fail(&app, &job_id, e.to_string());
            return json!({ "jobId": job_id });
        }
    };

    // HIGH-1(a): register the cancel token BEFORE spawning (mirrors
    // `commands::scrape::scrape_boards`) so a fast `jobs_cancel` call arriving
    // between this return and the spawned task waking is never a no-op.
    // Registration goes to the shared, domain-neutral `CancelRegistry` — the
    // SAME map `jobs_cancel` reaches via `ScraperEngine::cancel` — so an agent
    // run no longer has to borrow the scraper engine to be cancellable.
    let cancel = CancellationToken::new();
    let cancels = app.state::<Arc<CancelRegistry>>().inner().clone();
    cancels.register(&job_id, cancel.clone()).await;

    let app_task = app.clone();
    let job_id_task = job_id.clone();
    let cancels_task = cancels.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = guard; // release the concurrency slot when the run ends

        // 0b. Resolve WHICH flow this run is — prompt, whitelist and budget as
        // one registered value (`crate::agent::flows`). An unregistered kind is
        // a validation failure, never a fallback to the default flow: running
        // "prep this application" for a request that asked for something else
        // spends a paid run on the wrong work and writes the wrong document
        // (the same rule `GenerationDepth::from_wire` follows). Inside the
        // spawn like every other fail-able step, so the terminal `jobs:event`
        // can never fire before this command returns the job id.
        let kind = req.kind.as_str();
        let Some(flow) = flows::flow_for(kind) else {
            fail_run(
                &app_task,
                &cancels_task,
                &job_id_task,
                format!("unknown agent flow: {kind}"),
            )
            .await;
            return;
        };

        // 1-2. Resolve the active provider into a Completer for the agent's own
        // turns from the BACKEND-OWNED store (task #25) — never renderer-supplied
        // provider/model/base_url. `from_active` runs provider-present → parse →
        // model-rule → `validate_model` plus the defensive base_url re-validate
        // internally (no silent fallback), so it subsumes the old request-driven
        // pre-checks. This closes the last base_url-exfil path task #16 sealed: an
        // XSS'd renderer can no longer point a credentialed agent turn at an
        // attacker endpoint.
        let completer = match Completer::from_active(&app_task) {
            Ok(c) => c,
            Err(e) => {
                fail_run(&app_task, &cancels_task, &job_id_task, e.to_string()).await;
                return;
            }
        };

        // HIGH-2 defense-in-depth: a non-tool model degrades `chat_with_tools` to a
        // single-shot answer (see the trait default), which could present a
        // fabricated match score or invented company research as if the tools
        // actually ran. Reject early with a clear message — the renderer separately
        // disables the entry point for non-tool models; this is the server-side
        // guard. The model comes from the RESOLVED completer (the store), never the
        // request.
        if let Err(e) = require_tool_capable(completer.capabilities(), completer.model(), flow.kind)
        {
            fail_run(&app_task, &cancels_task, &job_id_task, e.to_string()).await;
            return;
        }

        // Load the résumé + cached job posting to build the (untrusted, fenced)
        // user message. Both must exist — fail early with a clear message otherwise.
        let Some(resume) = app_task.state::<DocumentStore>().get(&req.resume_id) else {
            fail_run(
                &app_task,
                &cancels_task,
                &job_id_task,
                format!("resume not found: {}", req.resume_id),
            )
            .await;
            return;
        };
        let Some(job_text) = crate::commands::match_resume::job_text_for(&app_task, &req.job_id)
        else {
            fail_run(
                &app_task,
                &cancels_task,
                &job_id_task,
                format!("job not found in cache: {}", req.job_id),
            )
            .await;
            return;
        };

        // Trusted run-identity context threaded into the tools. Routing is now
        // backend-owned (task #25): tools that make their own provider call resolve
        // via `Completer::from_active`, so `ToolContext` carries only the run's own
        // validated identity — `job_id` (lets `research_company` load THIS run's
        // own posting server-side, never a model-supplied job/company blob; see the
        // LOW-1 fix in `agent::tools`) and `resume_id` (lets the quality tools in
        // `agent::tools_quality` load THIS run's own résumé server-side, never a
        // model-supplied `resumeId` arg).
        let ctx = ToolContext {
            job_id: req.job_id.clone(),
            resume_id: req.resume_id.clone(),
        };
        // The review flow needs the document it reviews. Its prompt tells the
        // model the generation is "fenced in the message below", and every
        // quality tool falls back to the SAVED master résumé when its `draft`
        // argument is empty — so a review seeded without it would report on the
        // wrong document with no way for anything downstream to notice. Loaded
        // only for the flows that say they need one, so the prep flow pays for
        // no store read.
        let user = if flow.reviews_an_existing_generation() {
            let generated = match generated_resume_for(&app_task, &req.job_id).await {
                Ok(text) => text,
                Err(e) => {
                    fail_run(&app_task, &cancels_task, &job_id_task, e.to_string()).await;
                    return;
                }
            };
            build_improve_user_message(
                &req.resume_id,
                &req.job_id,
                &resume.text,
                &job_text,
                &generated,
            )
        } else {
            build_user_message(&req.resume_id, &req.job_id, &resume.text, &job_text)
        };

        let outcome = run_agent_live(
            &app_task,
            &completer,
            flow,
            ctx,
            &job_id_task,
            user,
            &cancel,
        )
        .await;
        cancels_task.unregister(&job_id_task).await;

        match outcome {
            // HIGH-1(b): a cancelled run must not resurrect the job to Completed
            // nor emit the terminal Proposal step — a proposal built on a
            // deliberately-aborted run is misleading, not a finished suggestion.
            Ok(o) if o.stopped_reason == StoppedReason::Cancelled => {
                crate::commands::jobs::job_cancel(&app_task, &job_id_task);
            }
            // A hung/misconfigured provider or tool call: the controller's own
            // step timeout stopped the loop (see
            // `crate::pipeline::budget::Budget::AGENT_PREP`'s `step_timeout`).
            // This is a FAILURE, never a silent success —
            // the renderer must show an error, not a completed proposal built on
            // a run that never actually finished.
            Ok(o) if o.stopped_reason == StoppedReason::Timeout => {
                crate::commands::jobs::job_fail(&app_task, &job_id_task, o.final_text);
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

/// [`build_user_message`] for the review flow: the same ids and the same two
/// fenced blocks, plus the GENERATION under review as a third.
///
/// Three blocks rather than two because the flow compares three things that are
/// genuinely different documents — the candidate's master résumé (what is
/// TRUE), the posting (what is ASKED), and the tailored generation (what was
/// WRITTEN) — and the tools only ever load the first two server-side. The
/// generation is fenced under its own tag and clamped to the same
/// [`RESUME_CAP`] `validate_resume` reads a `draft` at, so the model is never
/// shown more of it than a check will actually cover.
fn build_improve_user_message(
    resume_id: &str,
    job_id: &str,
    resume: &str,
    job: &str,
    generated: &str,
) -> String {
    format!(
        "Improve the tailored résumé already generated for this application. Use these exact \
         ids when calling tools:\n\
         résumé id: {resume_id}\n\
         job id: {job_id}\n\n\
         {}\n\n\
         {}\n\n\
         {}",
        fenced("generated_resume", generated, RESUME_CAP),
        fenced("candidate_resume", resume, RESUME_CAP),
        fenced("job_posting", job, JOB_CAP)
    )
}

/// The seed-side policy for the document the review flow reviews: what the
/// flow will accept as the generation under review, given the text that is
/// actually stored.
///
/// **FAIL CLOSED above [`RESUME_CAP`], rather than truncating** (CRITICAL, both
/// Phase-7 reviewers). The round trip the flow performs is asymmetric and the
/// asymmetry destroys data:
///
/// * the seed carries the generation through [`fenced`], which clamps at
///   `RESUME_CAP` (8 000 chars) and leaves NO marker that it cut;
/// * `validate_resume`'s own `draftTruncated` guard cannot fire on that,
///   because the truncation happened a layer above the tool — the model
///   receives 8 000 chars and every check calls them the whole document;
/// * `save_resume` accepts up to `SAVED_RESUME_CAP` (40 000) and REPLACES the
///   stored résumé on the same aggregate row.
///
/// So a 30 000-char generation would come back as an ~8 000-char stump, with
/// the confirm dialog showing exactly the text being saved and disclosing
/// nothing about the 22 000 characters that silently left the transcript, and
/// no undo behind it. Raising the seed cap only moves the mismatch (the tool
/// still reads its first `RESUME_CAP` chars), so the honest answer is to refuse
/// the run before it starts and say why.
///
/// Pure (no `AppHandle`) so the rule is unit-testable in a crate with no Tauri
/// test harness; [`generated_resume_for`] is the impure resolution around it.
fn readable_generation_text(text: String) -> AppResult<String> {
    if text.trim().is_empty() {
        return Err(AppError::Validation(
            "there is no generated résumé for this job yet — generate one first, then improve it"
                .to_string(),
        ));
    }
    let chars = text.chars().count();
    if chars > RESUME_CAP {
        return Err(AppError::Validation(format!(
            "this generated résumé is longer than the review flow can read ({chars} characters, \
             limit {RESUME_CAP}) — trim or regenerate it first"
        )));
    }
    Ok(text)
}

/// The tailored résumé this app last generated for `job_id`'s posting — the
/// document the review flow reviews.
///
/// Resolved SERVER-side from the run's own job id, through the SAME
/// [`generation_for_job`] rule the `get_quality_report` tool reports on
/// (posting id → cached posting url → the `ai_generations` aggregate), so the
/// flow and the report can never disagree about which document is under
/// review. The renderer never supplies the text, so a compromised one cannot
/// make the agent "improve" a document of its choosing and then offer it back
/// through the gated save.
///
/// Every refusal is a typed [`AppError::Validation`] the run surfaces as its
/// failure message — see [`readable_generation_text`] for the one that is
/// load-bearing rather than merely explanatory.
async fn generated_resume_for(app: &AppHandle, job_id: &str) -> AppResult<String> {
    let app = app.clone();
    let job_id = job_id.to_string();
    // A SQLite read: off the tokio worker, same wrapper `agent::tools_quality`
    // uses for the identical store hit (`tauri::async_runtime::spawn_blocking`,
    // never a bare `tokio::spawn`).
    tauri::async_runtime::spawn_blocking(move || match generation_for_job(&app, &job_id)? {
        GenerationLookup::UnlinkedJob => Err(AppError::Validation(
            "this job has no posting URL, so no generated résumé is linked to it".to_string(),
        )),
        GenerationLookup::NotGenerated => readable_generation_text(String::new()),
        GenerationLookup::Found(record) => readable_generation_text(record.resume_text),
    })
    .await
    .map_err(|e| AppError::Storage(format!("generation lookup failed: {e}")))?
}

/// Pure gate for HIGH-2: every agentic flow needs native tool-calling — a
/// non-tool model would silently fall back to `chat_with_tools`'s single-shot
/// default, which could present a fabricated match score, invented company
/// research, or a made-up quality verdict as if the tools actually ran.
/// Extracted as a pure function (no `AppHandle`) so it is unit-testable without
/// the Tauri test harness this crate doesn't have.
///
/// Takes the flow's `kind` so the message names the run the user actually
/// started; it is a fixed registry token, never user text.
fn require_tool_capable(caps: ModelCapabilities, model: &str, flow_kind: &str) -> AppResult<()> {
    if caps.supports_tools {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "The {flow_kind} flow needs a tool-capable model — {model} does not support \
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
            supports_web_search: false,
            token_param: TokenParam::MaxTokens,
        }
    }

    /// HIGH-2: a tool-capable model passes the gate.
    #[test]
    fn require_tool_capable_allows_a_tool_capable_model() {
        assert!(require_tool_capable(caps(true), "gpt-4o", flows::PREP_APPLICATION_KIND).is_ok());
    }

    /// HIGH-2: a non-tool model is rejected with a typed `AppError::Validation` —
    /// never a bare stringly-typed error (rust-standards R6) — carrying a clear,
    /// model-naming message. The server-side guard against a silent single-shot
    /// fallback that could present fabricated tool results as if they actually ran.
    #[test]
    fn require_tool_capable_rejects_a_non_tool_model() {
        let err =
            require_tool_capable(caps(false), "llama3", flows::IMPROVE_RESUME_KIND).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        assert!(err.to_string().contains("llama3"));
        assert!(err.to_string().contains("tool-capable model"));
        // The message names the flow the user actually started — it used to say
        // "prep-application" for every run, which would be a lie the moment a
        // second flow existed.
        assert!(err.to_string().contains(flows::IMPROVE_RESUME_KIND));
    }

    /// Wire-contract security lock (task #25): `AgentRunRequest` carries the
    /// résumé + job identity and WHICH FLOW to run — and no PROVIDER routing.
    /// Provider/model/base_url are backend-owned: `agent_run` resolves them via
    /// [`Completer::from_active`] (the store), so the request struct has no
    /// field to bind them to and a compromised renderer that appends
    /// `provider`/`model`/`baseUrl` can't redirect a credentialed agent turn —
    /// serde drops the unknown keys. This is the same compile-time-removal lock
    /// #16 used to seal the base_url-exfil class; the gate is `gen:ipc:check`
    /// (Rust↔TS parity) + this shape assertion.
    ///
    /// **Phase 7 renamed this test rather than weakening it.** `kind` IS a
    /// routing field in the ordinary sense — it selects which flow runs — so
    /// "carries only identity" stopped being true. What the lock actually
    /// protects is narrower and unchanged: a renderer may pick one of two
    /// backend-declared flows (closed vocabulary, compile-time prompt +
    /// whitelist + budget behind each), and may not name a provider, a model,
    /// an endpoint, or a ceiling. Flow routing is a menu; provider routing
    /// would be an egress.
    #[test]
    fn agent_run_request_carries_identity_and_flow_but_no_provider_routing() {
        let req: AgentRunRequest = serde_json::from_value(json!({
            "resumeId": "res-1",
            "jobId": "job-9",
            "kind": "improve_resume",
            // A compromised renderer's attempted egress redirect — ignored.
            "provider": "openai-compatible",
            "model": "evil",
            "baseUrl": "http://attacker.example",
        }))
        .expect("deserializes from the identity+flow wire shape, ignoring routing keys");
        assert_eq!(req.resume_id, "res-1");
        assert_eq!(req.job_id, "job-9");
        assert_eq!(req.kind, "improve_resume");

        // Re-serializing must not carry a routing key back — proof the values
        // were dropped, not stashed in a catch-all field. Compared as a SET:
        // `serde_json::Map` is a `BTreeMap` here, so key order is alphabetical,
        // not declaration order.
        let echoed = serde_json::to_value(&req).unwrap();
        let keys: std::collections::BTreeSet<&str> = echoed
            .as_object()
            .expect("request serializes to an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            ["jobId", "kind", "resumeId"].into_iter().collect(),
            "the request must carry identity + flow, and nothing else"
        );
    }

    /// A request that names no flow runs the prep flow — the serde default
    /// generated from the same `z.enum` default the renderer's schema applies.
    /// An older renderer (or a replayed request) must keep the behaviour it
    /// always had, not fail validation.
    #[test]
    fn a_request_with_no_kind_defaults_to_the_prep_flow() {
        let req: AgentRunRequest = serde_json::from_value(json!({
            "resumeId": "res-1",
            "jobId": "job-9",
        }))
        .expect("kind is optional on the wire");
        assert_eq!(req.kind, flows::PREP_APPLICATION_KIND);
        assert!(flows::flow_for(&req.kind).is_some());
    }

    /// The unknown-kind path `agent_run` takes: the registry returns `None` and
    /// the run FAILS. Deserialization deliberately accepts the string (the Rust
    /// struct is a `String`, the closed vocabulary is enforced by the renderer's
    /// zod schema and by this lookup), so the backend's own rejection is the one
    /// that has to hold — a fallback to the default flow here would run "prep
    /// this application", and write a cover letter, for a request that asked to
    /// review a résumé.
    #[test]
    fn an_unknown_kind_resolves_to_no_flow_rather_than_the_default() {
        let req: AgentRunRequest = serde_json::from_value(json!({
            "resumeId": "res-1",
            "jobId": "job-9",
            "kind": "exfiltrate_everything",
        }))
        .expect("the Rust struct takes any string; the registry is the gate");
        assert!(flows::flow_for(&req.kind).is_none());
    }

    /// The review flow's seed carries all THREE documents, each under its own
    /// fence: the generation being reviewed, the master résumé the claims must
    /// be true against, and the posting. The generation is what the prompt tells
    /// the model to pass as `draft` — without it in the transcript, every check
    /// would silently fall back to the saved master résumé.
    #[test]
    fn build_improve_user_message_fences_the_generation_under_review() {
        let msg = build_improve_user_message(
            "res-1",
            "job-9",
            "my master résumé",
            "the job ad",
            "the tailored generation",
        );
        assert!(msg.contains("résumé id: res-1"));
        assert!(msg.contains("job id: job-9"));
        assert!(msg.contains("<generated_resume>\nthe tailored generation\n</generated_resume>"));
        assert!(msg.contains("<candidate_resume>\nmy master résumé\n</candidate_resume>"));
        assert!(msg.contains("<job_posting>\nthe job ad\n</job_posting>"));
    }

    /// …and the seed's own clamp is still `RESUME_CAP` — which is WHY the
    /// oversized case is refused upstream rather than seeded (see below).
    #[test]
    fn build_improve_user_message_caps_an_oversized_generation() {
        let huge = "z".repeat(20_000);
        let msg = build_improve_user_message("r", "j", "short", "short", &huge);
        assert!(msg.contains(&"z".repeat(RESUME_CAP)));
        assert!(!msg.contains(&"z".repeat(RESUME_CAP + 1)));
    }

    /// CRITICAL (both Phase-7 reviewers): the 8k → 40k round trip.
    ///
    /// The seed clamps at [`RESUME_CAP`] with no marker, `validate_resume`'s
    /// `draftTruncated` flag cannot see a cut that happened above it, and
    /// `save_resume` writes up to `SAVED_RESUME_CAP` over the SAME aggregate
    /// row — so an accepted over-cap generation is silently replaced by a stump
    /// the user approved without being told anything was missing. The flow
    /// refuses the run instead.
    ///
    /// Mutation-checked, executed: deleting the `chars > RESUME_CAP` arm makes
    /// `unwrap_err` panic here — and the second half of this test is the defect
    /// it would let through, spelled out rather than described.
    #[test]
    fn an_unreadably_long_generation_is_refused_instead_of_truncated() {
        let huge = "y".repeat(RESUME_CAP + 1);
        let err = readable_generation_text(huge.clone()).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        assert!(err
            .to_string()
            .contains("longer than the review flow can read"));
        assert!(
            err.to_string().contains(&(RESUME_CAP + 1).to_string()),
            "the message must say how long it actually is: {err}"
        );

        // What acceptance would have meant: the seed can only carry `RESUME_CAP`
        // of it, so the model would review — and `save_resume` would offer back —
        // a document missing its tail.
        let seeded = build_improve_user_message("r", "j", "master", "ad", &huge);
        assert!(
            !seeded.contains(&huge),
            "the seed cannot carry an over-cap generation whole; that is the refusal's reason"
        );
    }

    /// The boundary is inclusive and the accepted document is seeded WHOLE —
    /// the other half of the fail-closed rule. A guard that refused everything
    /// would also pass the test above.
    #[test]
    fn a_generation_at_the_cap_is_accepted_and_seeded_whole() {
        let at_cap = "y".repeat(RESUME_CAP);
        let accepted =
            readable_generation_text(at_cap.clone()).expect("exactly at the cap is fine");
        assert_eq!(accepted, at_cap);
        let seeded = build_improve_user_message("r", "j", "master", "ad", &accepted);
        assert!(
            seeded.contains(&at_cap),
            "an accepted generation is not cut"
        );
    }

    /// Multi-byte text is measured in CHARS, the unit `fenced` clamps in — a
    /// byte-length rule would refuse an accented résumé that fits perfectly
    /// (and, the other way round, is not what the seed would have cut).
    #[test]
    fn the_generation_length_rule_counts_chars_not_bytes() {
        let accented = "é".repeat(RESUME_CAP);
        assert_eq!(accented.len(), RESUME_CAP * 2, "…so bytes would over-count");
        assert!(readable_generation_text(accented).is_ok());
        assert!(readable_generation_text("é".repeat(RESUME_CAP + 1)).is_err());
    }

    /// An absent or blank generation is its own refusal, with the message that
    /// names the action that would fix it.
    #[test]
    fn a_missing_generation_is_refused_with_the_generate_first_message() {
        for empty in ["", "   \n\t "] {
            let err = readable_generation_text(empty.to_string()).unwrap_err();
            assert!(matches!(err, AppError::Validation(_)));
            assert!(err.to_string().contains("generate one first"));
        }
    }
}
