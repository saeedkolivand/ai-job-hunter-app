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
use serde_json::json;

use crate::commands::ai_provider::{AiGenerateRequest, AiGenerateRequestMessage};
use crate::error::AppResult;
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
        // Length only — never the draft itself (ADR-027).
        ctx.ledger.record(
            "draft",
            json!({ "chars": text.chars().count(), "lines": text.lines().count() }),
        );
        ctx.draft = text;
        Ok(())
    }
}
