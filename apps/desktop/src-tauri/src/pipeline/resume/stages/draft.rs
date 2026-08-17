//! `draft` — the whole résumé body, in one streamed call, with an at-most-once
//! corrective retry when the streamed draft comes back in the wrong language.
//!
//! ## Why this one streams and the others do not
//!
//! A JSON stage produces an artifact nobody reads; streaming it would show the
//! user a wall of braces. The draft is the document, and it takes the longest,
//! so it streams under the run's own umbrella `jobId` — the id `jobs.cancel`
//! already reaches and the id the renderer already filters `ai:stream` on.
//!
//! **The retry never streams.** The renderer clears its buffer only at
//! `start()`/`reset()` and at the `cover_letter` stage-start event — all
//! outside this stage — so a SECOND stream over the SAME `job_id` would land
//! on top of the first with nothing telling the pane the model restarted:
//! two contact headers, two of every section, for the whole retry. So
//! [`run_draft_attempt`] routes the retry through [`DraftEnv::complete`], a
//! single non-streaming call, instead of [`DraftEnv::stream_captured`] — the
//! pane simply holds its last frame for the retry's duration.
//!
//! **The first attempt's stream is DISPLAY-ONLY.** The shared stream machinery
//! marks the job completed when the last delta lands, which is several stages
//! before the run is finished; the run's completion signal is the terminal
//! `pipeline:stage` event. The renderer contract says so explicitly, because
//! treating `awaitAiStream` resolving as "done" would show an unvalidated,
//! unrepaired draft as the final document.
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
use crate::pipeline::{Completer, Stage};
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

        let env = LiveDraftEnv { completer };
        // A plain reference, established here rather than as `&env` inside
        // the closure below: `&dyn DraftEnv` is `Copy`, so the closure
        // captures a COPY of the reference itself (exactly like `completer`
        // above) instead of borrowing the closure's OWN captured state —
        // the latter cannot outlive an `FnMut` call and is a compile error.
        let env: &dyn DraftEnv = &env;
        let (draft, mut artifact, retry) = draft_with_language_retry(
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
                run_draft_attempt(env, input.job_id, is_retry, attempt)
            },
        )
        .await?;

        // The first call always happened (its own error already propagated
        // above); the retry counts only when it actually made a round trip —
        // `LanguageRetryOutcome::called`'s doc explains why `Errored` (a
        // `charge_daily` refusal before any round trip, OR the retry's own
        // provider error) must NOT count, mirroring `repair.rs`'s
        // no-call-was-made rule for the same metric.
        ctx.ledger.count_call(false);
        if retry.called() {
            ctx.ledger.count_call(false);
        }
        artifact["languageRetry"] = json!(retry.attempted());
        // Length + projects-normalize counts only — never the draft itself
        // (ADR-027).
        ctx.ledger.record("draft", artifact);
        ctx.draft = draft;
        Ok(())
    }
}

/// A draft attempt's provider seam — the ONE decision that separates the
/// streamed first attempt from the never-streamed retry sits behind a trait,
/// exactly like `autopilot_helpers::NoteEnv`: this crate has no way to fake a
/// live `Completer`/`AppHandle`, so [`run_draft_attempt`]'s branch (the fix
/// for the double-render defect the module doc describes) has to be provable
/// with a fake instead. Prod wiring is [`LiveDraftEnv`].
#[async_trait]
pub(crate) trait DraftEnv: Send + Sync {
    /// The FIRST attempt only: streamed under the run's `job_id`, so the
    /// live pane fills in as the model writes. Charges the daily ceiling
    /// itself (see [`Completer::stream_captured`]).
    async fn stream_captured(&self, job_id: &str, req: AiGenerateRequest) -> AppResult<String>;
    /// The RETRY only: a single non-streaming call. Never reaches
    /// `ai:stream`, so a retry can never double the renderer's buffer — see
    /// the module doc.
    async fn complete(&self, system: &str, user: &str) -> AppResult<String>;
    /// Charge one call against the shared per-provider daily ceiling. The
    /// streamed channel charges internally; the captured channel charges
    /// explicitly here, mirroring `repair`/`humanize`'s own
    /// `charge_daily` + `complete` pairing for their non-streaming calls.
    fn charge_daily(&self) -> AppResult<()>;
}

/// Production [`DraftEnv`]: the resolved [`Completer`] for this stage. A
/// thin wrapper rather than implementing the trait on `Completer` directly —
/// `Completer` already has inherent `stream_captured`/`complete`/
/// `charge_daily` methods of its own, and this keeps the two unambiguous.
struct LiveDraftEnv<'a> {
    completer: &'a Completer,
}

#[async_trait]
impl DraftEnv for LiveDraftEnv<'_> {
    async fn stream_captured(&self, job_id: &str, req: AiGenerateRequest) -> AppResult<String> {
        self.completer.stream_captured(job_id, req).await
    }
    async fn complete(&self, system: &str, user: &str) -> AppResult<String> {
        self.completer.complete(system, user, None).await
    }
    fn charge_daily(&self) -> AppResult<()> {
        self.completer.charge_daily()
    }
}

/// One draft attempt through `env` — `is_retry` selects the channel: `false`
/// is the first, canonical call and always streams; `true` is the retry and
/// NEVER streams (see the module doc for why). This function IS the fix for
/// the double-render defect: before it existed, [`Draft::run`]'s closure
/// called the streamed channel unconditionally on both attempts.
pub(crate) async fn run_draft_attempt(
    env: &dyn DraftEnv,
    job_id: &str,
    is_retry: bool,
    req: AiGenerateRequest,
) -> AppResult<String> {
    if is_retry {
        env.charge_daily()?;
        env.complete(&req.messages[0].content, &req.messages[1].content)
            .await
    } else {
        env.stream_captured(job_id, req).await
    }
}

/// What the at-most-once corrective retry did — four outcomes previously
/// flattened into one `bool` (`retried`) that could not distinguish "no
/// round trip happened" from "one happened and was discarded", so
/// [`Draft::run`] counted a call on every non-`NotNeeded` outcome including
/// `Errored`, over-reporting `RunLedger::metrics.calls` whenever the retry's
/// own `charge_daily` refused before any round trip was sent. Mirrors
/// [`super::humanize::HumanizeAttempt`]'s shape for the same class of
/// problem, sized to this stage's simpler (single, at-most-once) retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LanguageRetryOutcome {
    /// The first draft already matched the target language, or the run's
    /// deadline had already passed — no retry was attempted.
    NotNeeded,
    /// The retry's own call errored: a provider failure, OR `charge_daily`
    /// refused before any round trip was sent (see [`DraftEnv::charge_daily`]
    /// on [`run_draft_attempt`]'s retry branch). The first draft is kept.
    /// Either way there is no completed round trip to bill — see
    /// [`Self::called`].
    Errored,
    /// The retry's round trip completed but the result was STILL the wrong
    /// language — the first draft is kept (see the module doc's
    /// never-hand-back-a-worse-document floor). One round trip to bill.
    StillWrong,
    /// The retry's round trip completed and fixed the language — the
    /// retry's draft is kept. One round trip to bill.
    Fixed,
}

impl LanguageRetryOutcome {
    /// Whether an actual provider round trip happened for the retry — the
    /// ONLY gate [`crate::pipeline::resume::RunLedger::count_call`] should
    /// read for this stage's retry. `Errored` never counts: `repair.rs`'s
    /// `Ok(SectionOutcome::Missing) => {}` arm states the same rule for the
    /// same metric ("No call was made, so nothing is counted — a metric
    /// that reported a round-trip here would over-report every run").
    pub(crate) fn called(self) -> bool {
        matches!(self, Self::StillWrong | Self::Fixed)
    }

    /// The diagnostic bit the run artifact's `languageRetry` field has
    /// always recorded: was a corrective retry attempted at all, called or
    /// not. Kept distinct from [`Self::called`] on purpose — the artifact is
    /// a content-free diagnostic (ADR-027), not a billing signal, and its
    /// existing meaning (true whenever the first draft needed a retry,
    /// whether or not that retry actually reached the provider) must not
    /// shift just because the billing gate was fixed.
    pub(crate) fn attempted(self) -> bool {
        !matches!(self, Self::NotNeeded)
    }
}

/// One streamed-or-captured draft, plus the at-most-once corrective retry,
/// with the PROVIDER CALL injected — the same seam shape as
/// `repair::repair_loop` and `humanize::humanize_one`, and the same reason:
/// `Draft::run` needs a live `Completer` and this crate has no Tauri harness
/// to build one from, so the decision here (fire the retry only on a
/// confirmed mismatch and before the deadline, never propagate a failed
/// retry, keep the retry ONLY if it actually fixed the language) has to be
/// provable by a test. See the module doc for why the bound is structural
/// rather than a flag.
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
) -> AppResult<(String, Value, LanguageRetryOutcome)>
where
    F: FnMut(bool) -> Fut,
    Fut: Future<Output = AppResult<String>>,
{
    let text = complete(false).await?;
    let (draft, artifact) = apply_projects_normalization(source_resume, text);

    if deadline.passed()
        || !document_language_mismatch(&draft, source_resume, job_ad, target_language)
    {
        return Ok((draft, artifact, LanguageRetryOutcome::NotNeeded));
    }

    match complete(true).await {
        // The retry itself failing (a provider error, or the day's ceiling
        // refusing it) is not this stage's failure — the run already has a
        // usable, if wrong-language, draft. See the module doc. Also no
        // round trip to bill either way — see `LanguageRetryOutcome::called`.
        Err(_) => Ok((draft, artifact, LanguageRetryOutcome::Errored)),
        Ok(second) => {
            let (candidate, candidate_artifact) =
                apply_projects_normalization(source_resume, second);
            if document_language_mismatch(&candidate, source_resume, job_ad, target_language) {
                // Still wrong. Two wrong-language drafts are equally wrong,
                // and the first came from the canonical prompt, so it is the
                // one that is kept.
                Ok((draft, artifact, LanguageRetryOutcome::StillWrong))
            } else {
                Ok((candidate, candidate_artifact, LanguageRetryOutcome::Fixed))
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
