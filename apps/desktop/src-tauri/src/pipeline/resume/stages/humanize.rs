//! `humanize` — the LAST stage, and the Warnings-side counterpart to
//! `repair`'s Criticals-only loop.
//!
//! ## Deterministic-first, exactly like `validate`
//!
//! Every `voice.*` finding on both documents is counted BEFORE a single
//! provider call is considered. Zero flags means the run is already clean by
//! the prompt's own bans, and this stage costs nothing — no call, no cache
//! lookup, nothing. A flagged document gets AT MOST ONE rewrite attempt
//! ([`attempt::humanize_one`]), never a loop: the model is asked to fix ONLY
//! the flagged lines, and the deterministic accept/revert rule below is what
//! actually decides whether the answer ships.
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
//! [`predicates::voice_findings`] drops any flagged line that also carries a
//! URL BEFORE it ever reaches the model, and the system prompt repeats the
//! ban as a hard contract. The résumé candidate additionally runs back
//! through [`projects::normalize_projects`] before it is graded — the SAME
//! deterministic, zero-cost pass `draft`/`repair` already run — so a rewrite
//! cannot silently alter a project link even if it tried to.
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
//! before sending it, and nothing checked for that. Every guard that already
//! existed — [`predicates::is_usable_rewrite`]'s length floor,
//! [`predicates::humanize_is_worse`]'s Critical/voice-flag comparison — grades
//! CONTENT, and a wrapper only ADDS length and introduces no new finding, so
//! the corrupt candidate sailed through both clean. `is_usable_rewrite` now
//! also rejects any candidate containing a registered fence tag
//! ([`crate::prompt_fence::contains_fence_tag`], checked against the whole
//! [`crate::prompt_fence`] registry, not just this one tag) — a REVERT, not a
//! strip-and-keep: a model that echoed the wrapper may have echoed other
//! scaffolding too, and this stage's job is cosmetic polish, so its failure
//! mode must be "no improvement", never "corrupted document".

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::prompts::{humanize_system, humanize_user, HumanizeTier};
use crate::pipeline::resume::{projects, QualityCtx};
use crate::pipeline::Stage;
use crate::validate::content::{ContentMetrics, ContentReport};

use super::validate::validate_documents;

mod attempt;
mod predicates;

pub(crate) use attempt::humanize_one;
pub(crate) use predicates::{
    exceeds_humanize_cap, should_humanize_letter, voice_count, voice_findings,
};

// Test-only surface: `humanize_is_worse`/`is_usable_rewrite` are called
// directly by `attempt::humanize_one` (never through this re-export) — only
// `pipeline::resume::tests` imports them by this path (via `stages::`).
#[cfg(test)]
pub(crate) use predicates::{humanize_is_worse, is_usable_rewrite};

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

/// The stage's ledger artifact — one shape, shared by all three exits
/// (`Stage::run`'s two early returns and its normal end), so a future new
/// field lands in every exit at once instead of being added to the "real"
/// one and forgotten on the early-return copies. `Default` gives the two
/// early exits an all-zero/all-false artifact for free.
#[derive(Debug, Default)]
struct Artifact {
    resume_flagged: usize,
    letter_flagged: usize,
    calls: u32,
    reverted: bool,
    voice_before: usize,
    voice_after: usize,
    failed: bool,
    timed_out: bool,
    capped: bool,
    too_large: bool,
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
        })
    }
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Humanize {
    fn name(&self) -> &'static str {
        NAME
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
            ctx.ledger.record(NAME, Artifact::default().into_json());
            return Ok(());
        }
        let resume_flagged = resume_report.as_ref().map_or(0, voice_count);
        let letter_flagged = ctx.letter_report.as_ref().map_or(0, voice_count);
        let voice_before = resume_flagged + letter_flagged;

        if resume_flagged == 0 && letter_flagged == 0 {
            ctx.ledger.record(NAME, Artifact::default().into_json());
            return Ok(());
        }

        let input = ctx.input;
        let completer = ctx.completer_for(NAME);
        // Computed once, exactly like `Draft::run`'s and `Repair::run`'s own
        // per-run seeding — every candidate reads the same seeds.
        let (seeds, _seed_skip_reason) = projects::seed_projects_for_normalize(input.source_resume);

        let mut calls: u32 = 0;
        let mut reverted = false;
        let mut failed = false;
        let mut timed_out = false;
        let mut capped = false;
        let mut too_large = false;

        // Read out of `ctx` BEFORE building any closure — `input` is `Copy`,
        // `completer` is an owned `&'a Completer`, and this is the letter text
        // the resume's revalidate pass must check alongside it. Mirrors
        // `Repair::run`'s own reasoning: a closure that borrowed `ctx` itself
        // would still be alive (via `humanize_one`'s `.await`) when `ctx.draft`
        // is written below, which the borrow checker rightly refuses.
        let letter_for_resume_revalidate = ctx.letter_text().to_string();
        // Same reason, same timing: the RESOLVED list
        // (`QualityCtx::top_requirements`'s doc), read once before `ctx.draft`
        // starts getting rewritten below.
        let top_requirements = ctx.top_requirements();

        if resume_flagged > 0 {
            // The SAME three gates `humanize_one` itself checks first, mirrored
            // HERE so `charge_daily` — the call that actually spends the
            // user's daily allowance — never fires on a path that was never
            // going to send anything: a document over the cap (`fenced()`
            // would silently truncate it — see `exceeds_humanize_cap`), an
            // already-expired deadline, or every flagged line landing on a
            // link line (`voice_findings` filters them all out).
            if exceeds_humanize_cap(&ctx.draft) {
                too_large = true;
            } else if ctx.deadline.passed() {
                timed_out = true;
            } else {
                // Safe: `resume_flagged > 0` only counts when `ctx.report` is
                // `Some` (see its own `voice_count` above) — the exact idiom,
                // and the exact justification, the letter arm below already
                // uses for `ctx.letter_report`.
                let resume_report = resume_report.clone().unwrap_or_else(empty_ok_report);
                let findings = voice_findings(&resume_report, &ctx.draft);
                if !findings.is_empty() {
                    match completer.charge_daily() {
                        // Limiter refused — don't attempt the rewrite. Neither
                        // `called` nor `failed`: nothing was sent.
                        Err(_) => capped = true,
                        Ok(()) => {
                            // ONE value, used for both the prompt tier AND the
                            // usable-rewrite floor below — see
                            // `is_usable_rewrite`'s own doc for why a single
                            // `HumanizeTier` (not two independent literals) is
                            // what makes "letter prompt, résumé floor" a type
                            // a caller cannot construct.
                            let tier = HumanizeTier::Resume;
                            let attempt = humanize_one(
                                ctx.deadline,
                                ctx.draft.clone(),
                                resume_report,
                                findings,
                                |text, findings| async move {
                                    completer
                                        .complete(
                                            &humanize_system(tier, input.target_language),
                                            &humanize_user(&text, &findings),
                                            None,
                                        )
                                        .await
                                },
                                |candidate: &str| projects::normalize_projects(candidate, &seeds),
                                |candidate| {
                                    let letter = letter_for_resume_revalidate.clone();
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
                                tier,
                            )
                            .await?;
                            calls += u32::from(attempt.called);
                            failed |= attempt.failed;
                            reverted |= attempt.reverted;
                            timed_out |= attempt.timed_out;
                            too_large |= attempt.too_large;
                            ctx.draft = attempt.text;
                            ctx.report = Some(attempt.report);
                        }
                    }
                }
                // else: every flag landed on a link line — nothing to ask
                // about, `ctx.report` stays `resume_report`'s own content.
            }
        }

        // `ctx.letter` DIRECTLY — never `ctx.letter_text()`'s fallback. See
        // `should_humanize_letter`'s own doc for why the field-read has to be
        // this, not the "whichever letter is in scope" convenience accessor
        // every OTHER reader (validate, repair, persist) correctly uses.
        let letter_body = ctx.letter.clone();
        if should_humanize_letter(letter_flagged, &letter_body, input.include_cover_letter) {
            // Same three gates, same reason, as the résumé arm above.
            if exceeds_humanize_cap(&letter_body) {
                too_large = true;
            } else if ctx.deadline.passed() {
                timed_out = true;
            } else {
                // Safe: `letter_flagged > 0` only counts when `ctx.letter_report`
                // is `Some` (see its own `voice_count` above).
                let letter_report = ctx.letter_report.clone().unwrap_or_else(empty_ok_report);
                let findings = voice_findings(&letter_report, &letter_body);
                if !findings.is_empty() {
                    match completer.charge_daily() {
                        Err(_) => capped = true,
                        Ok(()) => {
                            let tier = HumanizeTier::Letter;
                            let draft_for_revalidate = ctx.draft.clone();
                            let attempt = humanize_one(
                                ctx.deadline,
                                letter_body,
                                letter_report,
                                findings,
                                |text, findings| async move {
                                    completer
                                        .complete(
                                            &humanize_system(tier, input.target_language),
                                            &humanize_user(&text, &findings),
                                            None,
                                        )
                                        .await
                                },
                                // A letter has no Projects section to re-render.
                                |_candidate: &str| None,
                                |candidate| {
                                    let draft = draft_for_revalidate.clone();
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
                                        // FAIL CLOSED: `None` here means a
                                        // non-empty candidate produced no
                                        // letter report at all (unreachable
                                        // today — see `empty_ok_report`'s own
                                        // doc). An `Err` is caught by
                                        // `humanize_one`'s revalidate-error
                                        // path (kept original, `failed`), so
                                        // a future contract change on that
                                        // `None` arm REVERTS instead of
                                        // shipping an ungraded letter under a
                                        // fabricated clean report.
                                        letter_report.ok_or_else(|| {
                                            AppError::Validation(
                                                "the letter revalidate produced no report for a \
                                                 non-empty candidate"
                                                    .to_string(),
                                            )
                                        })
                                    }
                                },
                                tier,
                            )
                            .await?;
                            calls += u32::from(attempt.called);
                            failed |= attempt.failed;
                            reverted |= attempt.reverted;
                            timed_out |= attempt.timed_out;
                            too_large |= attempt.too_large;
                            ctx.letter = attempt.text;
                            ctx.letter_report = Some(attempt.report);
                        }
                    }
                }
                // else: every flag landed on a link line — nothing to ask
                // about, `ctx.letter`/`ctx.letter_report` stay as they are.
            }
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
            }
            .into_json(),
        );
        Ok(())
    }
}
