//! `draft` — the whole résumé body, in one streamed call.
//!
//! ## Why this one streams and the others do not
//!
//! A JSON stage produces an artifact nobody reads; streaming it would show the
//! user a wall of braces. The draft is the document, and it takes the longest,
//! so it streams under the run's own umbrella `jobId` — the id `jobs.cancel`
//! already reaches and the id the renderer already filters `ai:stream` on.
//!
//! **That stream is DISPLAY-ONLY.** The shared stream machinery marks the job
//! completed when the last delta lands, which is several stages before the run
//! is finished; the run's completion signal is the terminal `pipeline:stage`
//! event. The renderer contract says so explicitly, because treating
//! `awaitAiStream` resolving as "done" would show an unvalidated, unrepaired
//! draft as the final document.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::commands::ai_provider::{AiGenerateRequest, AiGenerateRequestMessage};
use crate::error::AppResult;
use crate::pipeline::resume::projects::{self, ProjectsNormalizeOutcome};
use crate::pipeline::resume::prompts::{draft_system, draft_user};
use crate::pipeline::resume::QualityCtx;
use crate::pipeline::Stage;

pub struct Draft;

const NAME: &str = "draft";

/// The intent this stage declares.
///
/// `prose_grounded`, not `prose`: the output makes factual claims about the
/// candidate that must stay traceable to the résumé, which is exactly the
/// distinction `Intent::ProseGrounded` encodes (it drops the presence-penalty
/// knob, because that knob pushes a model toward NEW topics — i.e. toward
/// invented candidate facts).
const DRAFT_INTENT: &str = "prose_grounded";

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Draft {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let completer = ctx.completer_for(NAME);
        // Deliberately NOT cached: a cache hit emits no `ai:stream` deltas, so
        // the user would watch an empty pane while an already-known answer was
        // "generated". See `pipeline::resume::cache`'s module doc.
        let req = AiGenerateRequest {
            model: String::new(), // overwritten by `Completer::stream` with the resolved model
            messages: vec![
                AiGenerateRequestMessage {
                    role: "system".to_string(),
                    content: draft_system(ctx.input.target_language),
                },
                AiGenerateRequestMessage {
                    role: "user".to_string(),
                    content: draft_user(ctx.input.source_resume, ctx.input.job_ad, &ctx.strategy),
                },
            ],
            locale: ctx.input.target_language.to_string(),
            // Sampling comes from the provider's own profile for the declared
            // intent (#958) — an explicit temperature here would override every
            // adapter's tuned value with one number for all ten of them.
            temperature: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            repeat_penalty: None,
            max_tokens: None,
            // The user's configured window for the model this stage resolved
            // to. The longest prompt the pipeline builds (see `prompts`:
            // RESUME_CAP + JOB_CAP ≈ 32k chars), so it is the call that most
            // needs the setting to arrive.
            context_window: completer.context_window(),
            effort: ctx.input.effort.map(str::to_string),
            intent: Some(DRAFT_INTENT.to_string()),
        };

        let text = completer.stream_captured(ctx.input.job_id, req).await?;
        ctx.ledger.count_call(false);

        let (draft, artifact) = apply_projects_normalization(ctx.input.source_resume, text);
        // Length + projects-normalize counts only — never the draft itself
        // (ADR-027).
        ctx.ledger.record("draft", artifact);
        ctx.draft = draft;
        Ok(())
    }
}

/// The DETERMINISTIC half of this stage — normalize the streamed text's
/// Projects section and build the ledger artifact — pulled out of `run` so it
/// is testable without a live provider (the model call above it is the only
/// part that needs one). Mirrors why `repair::repair_loop` is its own
/// function rather than inlined in `Repair::run`.
///
/// Deterministic, zero-cost: the Projects section is CODE-OWNED at quality
/// depth too, the same way `project_render::render_project` already owns it
/// at max — an entry this pass can confidently match to a seed gets its
/// links/stack restored verbatim, never re-asked; anything it cannot match
/// with confidence is left exactly as the model wrote it (see
/// `projects`' module doc) rather than deleted.
///
/// A skipped/no-op pass is still reported (`projectsNormalizeSkipped`) — a
/// silent "did nothing" is otherwise unobservable on the run's ledger.
pub(crate) fn apply_projects_normalization(source_resume: &str, text: String) -> (String, Value) {
    let (seeds, seed_skip_reason) = projects::seed_projects_for_normalize(source_resume);
    let (draft, matched, dropped, links_restored, skipped) =
        match projects::normalize_projects_outcome(&text, &seeds) {
            ProjectsNormalizeOutcome::Applied(draft, stats) => (
                draft,
                stats.matched,
                stats.dropped,
                stats.links_restored,
                None,
            ),
            ProjectsNormalizeOutcome::Skipped(reason) => (text, 0, 0, 0, Some(reason)),
            ProjectsNormalizeOutcome::NoOp => (text, 0, 0, 0, seed_skip_reason),
        };
    let mut artifact = json!({
        "chars": draft.chars().count(),
        "lines": draft.lines().count(),
        "projectsMatched": matched,
        "projectsDropped": dropped,
        "linksRestored": links_restored,
    });
    if let Some(reason) = skipped {
        artifact["projectsNormalizeSkipped"] = json!(reason);
    }
    (draft, artifact)
}
