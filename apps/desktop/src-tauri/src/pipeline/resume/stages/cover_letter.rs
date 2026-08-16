//! `cover_letter` — the letter body, in one streamed call, GATED by
//! [`QualityInput::include_cover_letter`].
//!
//! ## Why a gate instead of a second pipeline
//!
//! Every existing caller of [`quality_pipeline`](super::super::quality_pipeline)
//! predates this stage and never asked for a letter — `false` is the wire
//! default (`ResumePipelineRunRequest::include_cover_letter`), and every other
//! `QualityInput` construction in this crate sets it explicitly. A stage that
//! no-ops instantly at zero cost when the flag is unset is what keeps the whole
//! addition a ZERO-behavior-change diff for those callers, rather than a second
//! stage list to keep in step with the first.
//!
//! ## Streams under the SAME job id `draft` used
//!
//! Exactly like [`super::draft::Draft`]: a second `chat_stream` under the run's
//! umbrella `jobId` re-marks the job's tracker record complete when the
//! letter's own last delta lands — display-only, same as the draft's own
//! stream. The run's completion signal stays its terminal `pipeline:stage`
//! event, never a stream resolving; see that stage's module doc.

use async_trait::async_trait;
use serde_json::json;

use crate::commands::ai_provider::{AiGenerateRequest, AiGenerateRequestMessage};
use crate::error::AppResult;
use crate::pipeline::resume::prompts::{letter_system, letter_user, LETTER_INTENT};
use crate::pipeline::resume::QualityCtx;
use crate::pipeline::Stage;

pub struct CoverLetter;

const NAME: &str = "cover_letter";

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for CoverLetter {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        if !ctx.input.include_cover_letter {
            ctx.ledger.record(NAME, json!({ "skipped": true }));
            return Ok(());
        }

        let completer = ctx.completer_for(NAME);
        // Deliberately NOT cached — same reasoning as `Draft::run`: a cache hit
        // emits no `ai:stream` deltas, so the user would watch an empty pane
        // while an already-known letter was "generated".
        let req = AiGenerateRequest {
            model: String::new(), // overwritten by `Completer::stream` with the resolved model
            messages: vec![
                AiGenerateRequestMessage {
                    role: "system".to_string(),
                    content: letter_system(
                        ctx.input.target_language,
                        ctx.input.market,
                        !ctx.input.today.trim().is_empty(),
                    ),
                },
                AiGenerateRequestMessage {
                    role: "user".to_string(),
                    content: letter_user(
                        ctx.input.source_resume,
                        ctx.input.job_ad,
                        &ctx.strategy,
                        ctx.input.market,
                        ctx.input.today,
                    ),
                },
            ],
            locale: ctx.input.target_language.to_string(),
            temperature: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            repeat_penalty: None,
            max_tokens: None,
            context_window: completer.context_window(),
            effort: ctx.input.effort.map(str::to_string),
            intent: Some(LETTER_INTENT.to_string()),
        };

        let text = completer.stream_captured(ctx.input.job_id, req).await?;
        ctx.ledger.count_call(false);
        // Length only — never the letter itself (ADR-027).
        ctx.ledger.record(
            NAME,
            json!({
                "chars": text.chars().count(),
                "lines": text.lines().count(),
            }),
        );
        ctx.letter = text;
        Ok(())
    }
}
