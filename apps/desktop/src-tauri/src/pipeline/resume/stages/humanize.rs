//! `humanize` — the LAST stage, and the Warnings-side counterpart to
//! `repair`'s Criticals-only loop.
//!
//! ## Deterministic-first, exactly like `validate`
//!
//! Every `voice.*` finding on both documents is counted BEFORE a single
//! provider call is considered. Zero flags means the run is already clean by
//! the prompt's own bans, and this stage costs nothing — no call, no cache
//! lookup, nothing. A flagged document gets AT MOST ONE attempt
//! ([`attempt::humanize_one`]), never a loop.
//!
//! ## Line patches, not a re-emitted document
//!
//! The model is shown the flagged lines (numbered, with neighbours) and
//! answers `{patches: [{id, replacement}]}` ([`patches`]); Rust applies them to
//! flagged ids only and drops any replacement that is empty, multi-line,
//! fence-tagged or changes a number. The patched document then goes through
//! the same accept/revert rule below. A flag with no locatable line
//! (document-wide rhythm/dash density) cannot be patched: it rides along as
//! context. A document with ONLY such flags falls back to the old
//! whole-document rewrite (`patches::humanize_mode`); the ledger records
//! which `mode` ran.
//!
//! The résumé and the letter run CONCURRENTLY (`tokio::join!` in this task —
//! `Completer` is not `'static`, so no `spawn`). Each is revalidated against
//! the OTHER document as it stood before humanize.
//!
//! ## The revert rule ([`predicates::humanize_is_worse`])
//!
//! Reuses [`super::repair::round_is_worse`] — the SAME "introduced a Critical
//! (code, evidence) pair the draft did not already carry" discipline `repair`
//! enforces — and adds one more way to lose: MORE `voice.*` flags than before.
//! A rewrite that traded one AI tell for two, or that fixed the flagged line by
//! inventing an unsourced number, is worse either way, and this stage's whole
//! job is to leave the document no worse than it found it.
//!
//! ## Never cached, one attempt, no loop
//!
//! Same reasoning as `repair`'s module doc: a cached correction to a document
//! that has since changed is the worst possible hit, and a model that cannot
//! fix a flagged line once will not fix it by being asked twice.
//!
//! ## Link lines are safe by construction, not by trust
//!
//! [`patches::flagged_lines`] never flags a line that also carries a URL, and
//! only flagged ids can be patched, so a link line is unreachable even for a
//! model that ignores the system prompt's ban. The résumé candidate
//! additionally runs back through [`projects::normalize_projects`] before it
//! is graded — the SAME deterministic, zero-cost pass `draft`/`repair` already
//! run — so a rewrite cannot silently alter a project link even if it tried
//! to.
//!
//! ## Language residual
//!
//! A humanize rewrite in the target language (e.g., DE) using an English-lexicon
//! rule dictionary (antiAiTellProse) can seed English vocabulary into non-English
//! prose undetected by the per-language lexicon checks. Locale dispatch (using the
//! per-language prose checks, not the English ones) is the future fix.
//!
//! ## Shape, not just content (the `<humanize_document>` leak)
//!
//! A real run shipped an exported résumé with `<humanize_document>` as the
//! candidate's name and `</humanize_document>` as its last line: the model
//! returned the document WRAPPED in the fence tag `humanize_user` wraps it in
//! before sending it, and nothing checked for that. [`predicates::is_usable_rewrite`]
//! rejects any candidate containing a registered fence tag
//! ([`crate::prompt_fence::contains_fence_tag`]), and [`patches::apply_patches`]
//! drops any single replacement that carries one — a REVERT/keep, not a
//! strip-and-keep: this stage's job is cosmetic polish, so its failure mode
//! must be "no improvement", never "corrupted document".

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::prompts::HumanizeTier;
use crate::pipeline::resume::{projects, QualityCtx};
use crate::pipeline::Stage;
use crate::validate::content::{ContentMetrics, ContentReport};

use super::validate::validate_documents;

mod attempt;
mod doc;
mod patches;
mod predicates;

use attempt::HumanizeAttempt;
use doc::{humanize_doc, DocEnv};
pub(crate) use patches::Mode;
pub(crate) use patches::HUMAN_VOICE_FLAGS;

pub(crate) use predicates::{should_humanize_letter, voice_count};

// Test-only surface: `humanize_is_worse`/`is_usable_rewrite` are called
// directly by `attempt::humanize_one` (never through this re-export) — only
// `pipeline::resume::tests` imports them by this path (via `stages::`).
#[cfg(test)]
pub(crate) use attempt::humanize_one;
#[cfg(test)]
pub(crate) use patches::{
    apply_patches, excerpt_within, flagged_lines, humanize_mode, FlaggedLine, Patch, PatchList,
};
#[cfg(test)]
pub(crate) use predicates::{
    exceeds_humanize_cap, humanize_is_worse, is_usable_rewrite, voice_findings,
};

pub struct Humanize;

const NAME: &str = "humanize";

/// The empty-but-valid report used ONLY as the "before" baseline when
/// `ctx.letter_report` is somehow `None` even though `letter_flagged > 0` —
/// unreachable in practice (`voice_count`'s own `map_or(0, ..)` is what makes
/// `letter_flagged` positive, and that requires `Some`), kept as a defensive
/// fallback rather than a panic so a future change to that invariant degrades
/// instead of crashing a run.
///
/// **Safe as a BASELINE, wrong as an OUTCOME.** A fabricated clean "before"
/// only makes [`humanize_is_worse`]'s comparison MORE likely to revert (any
/// real issue in the candidate now reads as newly introduced), which is the
/// safe direction. Do not reuse this for a revalidate result: a fabricated
/// clean "after" report is what let an ungraded letter look accepted — see
/// the revalidate closure below, which fails CLOSED (an `Err`, caught by
/// `humanize_one`'s revalidate-error path) instead of calling this on `None`.
fn empty_ok_report() -> ContentReport {
    ContentReport {
        ok: true,
        issues: Vec::new(),
        metrics: ContentMetrics::default(),
    }
}

/// The stage's ledger artifact — one shape, shared by all exits, so a future
/// new field lands in every exit at once. `Default` gives an all-zero/all-false
/// artifact for free.
#[derive(Debug, Default)]
struct Artifact {
    resume_flagged: usize,
    letter_flagged: usize,
    /// Attempts that reached the provider, one per document. A structured
    /// (patch) call's hidden parse re-ask is a second round-trip that
    /// `Completer::complete_json` does not report, so it is NOT counted here
    /// (it is still charged and recorded in spend).
    calls: u32,
    reverted: bool,
    voice_before: usize,
    voice_after: usize,
    failed: bool,
    timed_out: bool,
    capped: bool,
    too_large: bool,
    /// Nothing eligible was flagged: the draft already reads human (no call).
    skipped: bool,
    /// Which path ran: "patch", "rewrite", "mixed" (the two documents
    /// differ), or `None` when no call was routed.
    mode: Option<&'static str>,
}

impl Artifact {
    // Counts only (ADR-027) — never the generated text or the finding lines
    // the model saw.
    fn into_json(self) -> Value {
        json!({
            "resumeFlagged": self.resume_flagged,
            "letterFlagged": self.letter_flagged,
            "calls": self.calls,
            "reverted": self.reverted,
            "voiceBefore": self.voice_before,
            "voiceAfter": self.voice_after,
            "failed": self.failed,
            "timedOut": self.timed_out,
            "capped": self.capped,
            "tooLarge": self.too_large,
            "skipped": self.skipped,
            "mode": self.mode,
        })
    }

    /// The zero-call exit: nothing to humanize.
    fn skip() -> Self {
        Self {
            skipped: true,
            ..Self::default()
        }
    }
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Humanize {
    fn name(&self) -> &'static str {
        NAME
    }

    /// Safe to drop mid-call: `ctx` is written only after both arms finish, so a
    /// cancelled humanize leaves the pre-humanize documents untouched.
    fn abandon_on_cancel(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        // The guard asks about BOTH documents, not just the résumé. It used to
        // bind `ctx.report` with a `let … else`, which made a résumé report the
        // precondition for the WHOLE stage — and `letter_flagged` is computed
        // below it. A cover-letter-only run has no résumé report by design
        // (`stages::validate`), so that shape would return here and record a
        // zero-flag artifact for a letter it never looked at: the polish pass
        // silently skipped, and the trail claiming nothing was flagged.
        //
        // `None` on BOTH is the case the original guard was written for —
        // validate did not run, so there is nothing to grade against.
        let resume_report = ctx.report.clone();
        if resume_report.is_none() && ctx.letter_report.is_none() {
            ctx.ledger.record(NAME, Artifact::skip().into_json());
            return Ok(());
        }
        let resume_flagged = resume_report.as_ref().map_or(0, voice_count);
        let letter_flagged = ctx.letter_report.as_ref().map_or(0, voice_count);
        let voice_before = resume_flagged + letter_flagged;

        // Already reads human: no flag, no call, no cache lookup.
        if voice_before == HUMAN_VOICE_FLAGS {
            ctx.ledger.record(NAME, Artifact::skip().into_json());
            return Ok(());
        }

        let input = ctx.input;
        let completer = ctx.completer_for(NAME);
        // Mechanical stage: the user's effort, else the lowest tier.
        let effort = ctx.stage_effort(NAME);
        let guard = ctx.deadline_guard();
        let env = DocEnv {
            completer,
            deadline: ctx.deadline,
            guard: &guard,
            lang: input.target_language,
            effort,
        };
        // Computed once, exactly like `Draft::run`'s and `Repair::run`'s own
        // per-run seeding — every candidate reads the same seeds.
        let (seeds, _seed_skip_reason) = projects::seed_projects_for_normalize(input.source_resume);

        // Everything both arms read is cloned out of `ctx` BEFORE the join: the
        // arms run concurrently, so each is revalidated against the OTHER
        // document as it stood before humanize, and `ctx` is written only
        // after both finish.
        let draft = ctx.draft.clone();
        let letter_before = ctx.letter_text().to_string();
        let top_requirements = ctx.top_requirements();
        // `ctx.letter` DIRECTLY — never `ctx.letter_text()`'s fallback. See
        // `should_humanize_letter`'s own doc for why the field-read has to be
        // this, not the "whichever letter is in scope" convenience accessor
        // every OTHER reader (validate, repair, persist) correctly uses.
        let letter_body = ctx.letter.clone();
        let letter_report = ctx.letter_report.clone().unwrap_or_else(empty_ok_report);

        let resume_arm = async {
            if resume_flagged == 0 {
                return Ok(None);
            }
            // Safe: `resume_flagged > 0` only counts when `ctx.report` is `Some`.
            let report = resume_report.clone().unwrap_or_else(empty_ok_report);
            humanize_doc(
                &env,
                HumanizeTier::Resume,
                draft.clone(),
                report,
                |candidate: &str| projects::normalize_projects(candidate, &seeds),
                |candidate| {
                    let letter = letter_before.clone();
                    let top_requirements = top_requirements.clone();
                    async move {
                        let (report, _letter_report) = validate_documents(
                            candidate,
                            input.source_resume.to_string(),
                            input.job_ad.to_string(),
                            top_requirements,
                            input.target_language.to_string(),
                            letter,
                        )
                        .await?;
                        Ok(report)
                    }
                },
            )
            .await
            .map(Some)
        };
        let letter_arm = async {
            if !should_humanize_letter(letter_flagged, &letter_body, input.include_cover_letter) {
                return Ok(None);
            }
            humanize_doc(
                &env,
                HumanizeTier::Letter,
                letter_body.clone(),
                letter_report.clone(),
                // A letter has no Projects section to re-render.
                |_candidate: &str| None,
                |candidate| {
                    let draft = draft.clone();
                    let top_requirements = top_requirements.clone();
                    async move {
                        let (_resume_report, letter_report) = validate_documents(
                            draft,
                            input.source_resume.to_string(),
                            input.job_ad.to_string(),
                            top_requirements,
                            input.target_language.to_string(),
                            candidate,
                        )
                        .await?;
                        // FAIL CLOSED: `None` here means a non-empty candidate
                        // produced no letter report at all (unreachable today —
                        // see `empty_ok_report`). An `Err` is caught by
                        // `humanize_one`'s revalidate-error path (kept
                        // original, `failed`), so a future contract change
                        // REVERTS instead of shipping an ungraded letter under
                        // a fabricated clean report.
                        letter_report.ok_or_else(|| {
                            AppError::Validation(
                                "the letter revalidate produced no report for a non-empty \
                                 candidate"
                                    .to_string(),
                            )
                        })
                    }
                },
            )
            .await
            .map(Some)
        };
        let (resume_result, letter_result): (
            AppResult<Option<(HumanizeAttempt, Option<Mode>)>>,
            AppResult<Option<(HumanizeAttempt, Option<Mode>)>>,
        ) = tokio::join!(resume_arm, letter_arm);
        let (resume_attempt, letter_attempt) = (resume_result?, letter_result?);

        let mut calls: u32 = 0;
        let (mut reverted, mut failed, mut timed_out) = (false, false, false);
        let (mut capped, mut too_large) = (false, false);
        let mut modes: Vec<Mode> = Vec::new();
        let mut tally = |(attempt, mode): &(HumanizeAttempt, Option<Mode>)| {
            modes.extend(mode);
            calls += u32::from(attempt.called);
            failed |= attempt.failed;
            reverted |= attempt.reverted;
            timed_out |= attempt.timed_out;
            too_large |= attempt.too_large;
            capped |= attempt.capped;
        };
        if let Some(done) = resume_attempt {
            tally(&done);
            let attempt = done.0;
            ctx.draft = attempt.text;
            ctx.report = Some(attempt.report);
        }
        if let Some(done) = letter_attempt {
            tally(&done);
            let attempt = done.0;
            ctx.letter = attempt.text;
            ctx.letter_report = Some(attempt.report);
        }

        let voice_after = ctx.report.as_ref().map_or(0, voice_count)
            + ctx.letter_report.as_ref().map_or(0, voice_count);

        if timed_out {
            // First-writer-wins: a run already cancelled or already out of
            // time upstream keeps its own reason.
            ctx.ledger.stop(StoppedReason::RunTimeout);
        }
        for _ in 0..calls {
            ctx.ledger.count_call(false);
        }
        ctx.ledger.record(
            NAME,
            Artifact {
                resume_flagged,
                letter_flagged,
                calls,
                reverted,
                voice_before,
                voice_after,
                failed,
                timed_out,
                capped,
                too_large,
                skipped: modes.is_empty() && !timed_out && !too_large,
                mode: match modes.as_slice() {
                    [] => None,
                    [first, rest @ ..] if rest.iter().all(|m| m == first) => Some(first.as_str()),
                    _ => Some("mixed"),
                },
            }
            .into_json(),
        );
        Ok(())
    }
}
