//! `draft` — the whole résumé body, in one streamed call, with an at-most-once
//! corrective retry when the streamed draft comes back in the wrong language.
//!
//! ## Why this one streams and the others do not
//!
//! A JSON stage produces an artifact nobody reads; streaming it would show the
//! user a wall of braces. The draft is the document, and it takes the longest,
//! so it streams under the run's own umbrella `jobId` — the id `jobs.cancel`
//! already reaches and the id the renderer already filters `ai:stream` on. A
//! retry streams a SECOND draft over the first, under the same job id — the
//! renderer just sees more deltas, exactly as if the model kept writing.
//!
//! **That stream is DISPLAY-ONLY.** The shared stream machinery marks the job
//! completed when the last delta lands, which is several stages before the run
//! is finished; the run's completion signal is the terminal `pipeline:stage`
//! event. The renderer contract says so explicitly, because treating
//! `awaitAiStream` resolving as "done" would show an unvalidated, unrepaired
//! draft as the final document.
//!
//! ## The language retry is structurally at-most-once
//!
//! [`draft_with_language_retry`] is a single straight-line `if`: call the
//! model once, and call it a SECOND time only when
//! [`document_language_mismatch`] fires on the first draft and the run's own
//! deadline has not passed. There is no flag and no counter — `retried` is a
//! local returned once and dropped, `Draft` is a unit struct with no fields,
//! and a fresh one is constructed per run. Nothing here persists across
//! calls, so there is nothing to reset and nothing a caller could get wrong —
//! this repo has a recorded history of "run this once" guards that latched
//! and never released. Making this run twice means turning the `if` in
//! [`draft_with_language_retry`] into a `while`, a visible diff to that
//! function's own body, not a state that could silently drift.
//!
//! The retry's own call failing (a provider error, or the day's provider
//! ceiling refusing it) is never THIS stage's failure: the run already has a
//! usable, if wrong-language, first draft, so the failure is caught and the
//! first draft is kept — the same never-propagate-a-best-effort-failure rule
//! `humanize::humanize_one` already holds for its one rewrite attempt. And
//! the retry is kept ONLY when it actually fixed the language: two
//! wrong-language drafts are equally wrong, and the first came from the
//! canonical prompt, so a tie keeps the first — the same
//! never-hand-back-a-worse-document floor `repair::round_is_worse` holds.

use std::future::Future;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::commands::ai_provider::{AiGenerateRequest, AiGenerateRequestMessage};
use crate::error::AppResult;
use crate::pipeline::resume::projects::{self, ProjectsNormalizeOutcome};
use crate::pipeline::resume::prompts::{draft_language_retry_note, draft_system, draft_user};
use crate::pipeline::resume::{QualityCtx, RunDeadline};
use crate::pipeline::Stage;
use crate::validate::content::document_language_mismatch;

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
        // Read out of `ctx` before it is borrowed mutably again below —
        // `top_requirements()` reads `ctx.analysis`, which `analyze_job`
        // already finished writing by the time this stage runs.
        let top_requirements = ctx.top_requirements();
        // `QualityInput` is `Copy` — a local copy so the retry closure below
        // can move it without holding a borrow of `ctx` across the `.await`.
        let input = ctx.input;
        // Deliberately NOT cached: a cache hit emits no `ai:stream` deltas, so
        // the user would watch an empty pane while an already-known answer was
        // "generated". See `pipeline::resume::cache`'s module doc.
        let req = AiGenerateRequest {
            model: String::new(), // overwritten by `Completer::stream` with the resolved model
            messages: vec![
                AiGenerateRequestMessage {
                    role: "system".to_string(),
                    content: draft_system(input.target_language, input.market),
                },
                AiGenerateRequestMessage {
                    role: "user".to_string(),
                    content: draft_user(
                        input.source_resume,
                        input.job_ad,
                        &ctx.strategy,
                        &top_requirements,
                    ),
                },
            ],
            locale: input.target_language.to_string(),
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
            effort: input.effort.map(str::to_string),
            intent: Some(DRAFT_INTENT.to_string()),
        };

        let (draft, mut artifact, retried) = draft_with_language_retry(
            input.source_resume,
            input.job_ad,
            input.target_language,
            ctx.deadline,
            move |is_retry| {
                let mut attempt = req.clone();
                if is_retry {
                    // Rust-owned corrective note, appended to the SYSTEM slot —
                    // ADR-010 holds, nothing renderer-supplied reaches it.
                    attempt.messages[0]
                        .content
                        .push_str(&draft_language_retry_note(input.target_language));
                }
                completer.stream_captured(input.job_id, attempt)
            },
        )
        .await?;

        // The first call always happened (its own error already propagated
        // above); the retry is a second round-trip only when it fired.
        ctx.ledger.count_call(false);
        if retried {
            ctx.ledger.count_call(false);
        }
        artifact["languageRetry"] = json!(retried);
        // Length + projects-normalize counts only — never the draft itself
        // (ADR-027).
        ctx.ledger.record("draft", artifact);
        ctx.draft = draft;
        Ok(())
    }
}

/// One streamed draft, plus the at-most-once corrective retry, with the
/// PROVIDER CALL injected — the same seam shape as `repair::repair_loop` and
/// `humanize::humanize_one`, and the same reason: `Draft::run` needs a live
/// `Completer` and this crate has no Tauri harness to build one from, so the
/// decision here (fire the retry only on a confirmed mismatch and before the
/// deadline, never propagate a failed retry, keep the retry ONLY if it
/// actually fixed the language) has to be provable by a test. See the module
/// doc for why the bound is structural rather than a flag.
///
/// `complete(false)` makes the first call; its failure IS this stage's
/// failure and propagates via `?`, exactly as before this retry existed.
/// `complete(true)` makes the retry, at most once.
pub(crate) async fn draft_with_language_retry<F, Fut>(
    source_resume: &str,
    job_ad: &str,
    target_language: &str,
    deadline: RunDeadline,
    mut complete: F,
) -> AppResult<(String, Value, bool)>
where
    F: FnMut(bool) -> Fut,
    Fut: Future<Output = AppResult<String>>,
{
    let text = complete(false).await?;
    let (draft, artifact) = apply_projects_normalization(source_resume, text);

    if deadline.passed()
        || !document_language_mismatch(&draft, source_resume, job_ad, target_language)
    {
        return Ok((draft, artifact, false));
    }

    match complete(true).await {
        // The retry itself failing (a provider error, or the day's ceiling
        // refusing it) is not this stage's failure — the run already has a
        // usable, if wrong-language, draft. See the module doc.
        Err(_) => Ok((draft, artifact, true)),
        Ok(second) => {
            let (candidate, candidate_artifact) =
                apply_projects_normalization(source_resume, second);
            if document_language_mismatch(&candidate, source_resume, job_ad, target_language) {
                // Still wrong. Two wrong-language drafts are equally wrong,
                // and the first came from the canonical prompt, so it is the
                // one that is kept.
                Ok((draft, artifact, true))
            } else {
                Ok((candidate, candidate_artifact, true))
            }
        }
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
