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
//!
//! ## Opt-in company research, gated the SAME way, non-fatal by construction
//!
//! [`QualityInput::research_company`] is a second gate, independent of
//! `include_cover_letter`'s: `false` (the wire default) is a zero-cost no-op,
//! same reasoning as above. When it IS set, [`research_company_brief`] admits
//! against the shared `"ai_research"` bucket
//! ([`Completer::admit_research`](crate::pipeline::Completer::admit_research))
//! — the same billable-web-search ceiling `commands::ai::ai_research_company`
//! admits against — and degrades to `""` on ANY refusal, search failure,
//! timeout, or unresolved company name. There is no `?` on that path: a
//! research outcome can only ever change whether `letter_user` fences a
//! `<company_research>` block, never whether the letter itself generates.

use async_trait::async_trait;
use serde_json::json;

use crate::commands::ai_provider::timeouts::research_deadline;
use crate::commands::ai_provider::{AiGenerateRequest, AiGenerateRequestMessage};
use crate::cover_letter::research::CompanyResearch;
use crate::error::AppResult;
use crate::pipeline::resume::prompts::{letter_system, letter_user, LETTER_INTENT};
use crate::pipeline::resume::QualityCtx;
use crate::pipeline::{Completer, Stage};

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

        let brief = if ctx.input.research_company {
            research_company_brief(completer, ctx).await
        } else {
            String::new()
        };

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
                        !brief.trim().is_empty(),
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
                        &brief,
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
        // Length only — never the letter or the brief itself (ADR-027).
        ctx.ledger.record(
            NAME,
            json!({
                "chars": text.chars().count(),
                "lines": text.lines().count(),
                "researchAttempted": ctx.input.research_company,
                "researchBriefChars": brief.chars().count(),
            }),
        );
        ctx.letter = text;
        Ok(())
    }
}

/// Research the run's company for the letter's "why this company" paragraph —
/// opt-in ([`crate::pipeline::resume::QualityInput::research_company`]), and
/// non-fatal BY CONSTRUCTION: there is no `?` anywhere in this function, so an
/// admission refusal, a search failure, a timeout, or an unresolved company
/// name all fall through to `""` — exactly how
/// `commands::ai::ai_research_company` degrades to `{"brief": ""}` rather than
/// a command error. Admits against the SAME shared `"ai_research"` bucket that
/// command goes through (see [`Completer::admit_research`]'s doc): this is a
/// SECOND billable, no-other-ceiling provider web search per run, and a run
/// whose toggle is on must not open a path around that ceiling.
async fn research_company_brief(completer: &Completer, ctx: &QualityCtx<'_>) -> String {
    let Some(_guard) = completer.admit_research(NAME) else {
        return String::new();
    };
    let deadline = research_deadline(ctx.input.effort);
    let company = ctx.input.company_name.trim();
    let role = ctx.analysis.role_title.trim();
    CompanyResearch
        .enrich_with(
            completer,
            ctx.input.job_ad,
            (!company.is_empty()).then_some(company),
            (!role.is_empty()).then_some(role),
            deadline,
        )
        .await
        .content
}
