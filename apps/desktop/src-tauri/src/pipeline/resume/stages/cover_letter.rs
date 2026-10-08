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
//! ## Streams under its OWN id
//!
//! The letter streams under a child id of the run's `jobId` (see
//! [`Completer::stream_captured_child`]), never the
//! run's umbrella `jobId` the draft uses: with the draft streaming beside it
//! (see [`super::letter_ahead`]) one shared id would interleave the two
//! documents' tokens. Cancelling the umbrella job cancels it too
//! (`JobTracker::is_cancelled`). Display-only, like the draft's stream: the
//! run's completion signal stays its terminal `pipeline:stage` event.
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
use tokio::sync::oneshot;

use crate::commands::ai_provider::call_trace;
use crate::commands::ai_provider::timeouts::research_deadline;
use crate::commands::ai_provider::{AiGenerateRequest, AiGenerateRequestMessage};
use crate::cover_letter::research::CompanyResearch;
use crate::error::AppResult;
use crate::pipeline::resume::prompts::{letter_system, letter_user, LETTER_INTENT};
use crate::pipeline::resume::QualityCtx;
use crate::pipeline::{Completer, Stage};

pub struct CoverLetter;

const NAME: &str = LETTER_STAGE;

/// The stage name, also the routing key the early research lookup resolves its
/// completer under (so it runs on the model the letter would have used).
pub(crate) const LETTER_STAGE: &str = "cover_letter";

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

        let LetterOut { text, brief_chars } = match ctx.letter_ahead.take() {
            // `draft` wrote it beside itself: its calls were traced under their
            // own log, so they are reported here, with this stage.
            Some(ahead) => {
                call_trace::merge(ahead.calls);
                ahead.result?
            }
            None => {
                let brief = take_brief(ctx);
                write_letter(LetterJob::new(ctx, brief)).await?
            }
        };
        ctx.ledger.count_call(false);
        // Length only — never the letter or the brief itself (ADR-027).
        ctx.ledger.record(
            NAME,
            json!({
                "chars": text.chars().count(),
                "lines": text.lines().count(),
                "researchAttempted": ctx.input.research_company,
                "researchBriefChars": brief_chars,
            }),
        );
        ctx.letter = text;
        Ok(())
    }
}

/// What a finished letter hands to the ledger.
pub(crate) struct LetterOut {
    text: String,
    brief_chars: usize,
}

/// Everything [`write_letter`] reads, borrowed from the context so the call can
/// run beside `draft` (which reads the same fields, immutably).
pub(crate) struct LetterJob<'c, 'a> {
    ctx: &'c QualityCtx<'a>,
    /// The early company-research lookup's brief, when one was armed.
    brief: Option<oneshot::Receiver<String>>,
}

impl<'c, 'a> LetterJob<'c, 'a> {
    pub(crate) fn new(ctx: &'c QualityCtx<'a>, brief: Option<oneshot::Receiver<String>>) -> Self {
        Self { ctx, brief }
    }
}

/// The armed early research's brief, taken out of the context (`None` when the
/// run did not ask for research, or never armed a lookup).
pub(crate) fn take_brief(ctx: &mut QualityCtx<'_>) -> Option<oneshot::Receiver<String>> {
    if !ctx.input.research_company {
        return None;
    }
    ctx.early_research.as_mut().and_then(|e| e.take_brief())
}

/// Write the letter: the (opt-in) company brief, then one streamed call.
pub(crate) async fn write_letter(job: LetterJob<'_, '_>) -> AppResult<LetterOut> {
    let LetterJob { ctx, brief } = job;
    let completer = ctx.completer_for(NAME);

    let brief = if ctx.input.research_company {
        // Started right after `analyze_job` when armed (see `early_research`);
        // awaited only if it has not finished. A dropped lookup is "no brief".
        // Unarmed callers research inline as before.
        match brief {
            Some(brief) => brief.await.unwrap_or_default(),
            None => research_company_brief(completer, ctx).await,
        }
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

    let text = completer
        .stream_captured_child(ctx.input.job_id, "letter", req)
        .await?;
    Ok(LetterOut {
        text,
        brief_chars: brief.chars().count(),
    })
}

/// Research the run's company for the letter's "why this company" paragraph —
/// opt-in ([`crate::pipeline::resume::QualityInput::research_company`]), and
/// non-fatal BY CONSTRUCTION: the `-> String` return type below makes it a
/// COMPILE ERROR for this function's body to contain a `?` on any
/// `Result`/`Option` sub-expression (`String` implements neither
/// `FromResidual<Result<Infallible, _>>` nor `FromResidual<Option<Infallible>>`),
/// so an admission refusal, a search failure, a timeout, or an unresolved
/// company name can only ever fall through to `""` — exactly how
/// `commands::ai::ai_research_company` degrades to `{"brief": ""}` rather than
/// a command error. Admits against the SAME shared `"ai_research"` bucket that
/// command goes through (see [`Completer::admit_research`]'s doc): this is a
/// SECOND billable, no-other-ceiling provider web search per run, and a run
/// whose toggle is on must not open a path around that ceiling.
///
/// `pub(crate)`, not private: `pipeline::resume::tests` pins this exact
/// signature at compile time (`research_company_brief_returns_a_plain_string`)
/// instead of scraping this file's source text for a `?` — see that test's
/// doc for why the scrape it replaced was a weaker guarantee than the type
/// system already gives us for free.
pub(crate) async fn research_company_brief(completer: &Completer, ctx: &QualityCtx<'_>) -> String {
    research_brief(
        completer,
        ctx.input.job_ad,
        ctx.input.company_name,
        &ctx.analysis.role_title,
        ctx.input.effort,
    )
    .await
}

/// The ctx-free body of [`research_company_brief`], shared with the early
/// lookup. Same non-fatal `String` return, same admission before any spend.
pub(crate) async fn research_brief(
    completer: &Completer,
    job_ad: &str,
    company: &str,
    role: &str,
    effort: Option<&str>,
) -> String {
    let Some(_guard) = completer.admit_research(NAME) else {
        return String::new();
    };
    let deadline = research_deadline(effort);
    let (company, role) = (company.trim(), role.trim());
    CompanyResearch
        .enrich_with(
            completer,
            job_ad,
            (!company.is_empty()).then_some(company),
            (!role.is_empty()).then_some(role),
            deadline,
        )
        .await
        .content
}
