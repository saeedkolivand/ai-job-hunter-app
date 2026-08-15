//! `repair` — up to [`Budget::max_repair_attempts`] rounds of section-scoped
//! correction, and the rules that keep it from making things worse.
//!
//! 1. **Criticals only.** Warnings are advice; regenerating a section to chase
//!    one spends a provider call to trade a stylistic nit for a fresh chance at
//!    a factual error.
//! 2. **Failing sections only**, spliced back in. A whole-document rewrite
//!    would re-roll the sections that were already correct.
//! 3. **A truncated section is a FAILED attempt**, not a smaller section —
//!    splicing one in deletes content silently (see
//!    [`sections::is_usable_replacement`]).
//! 4. **A worse round ⇒ revert and stop**, where "worse" is strictly more
//!    criticals OR a newly INTRODUCED absence. The repair is a bet that the
//!    model can fix what it broke; when the bet loses, the honest move is to
//!    hand back the draft that was merely wrong rather than the one that is now
//!    wrong in more places. Equal is not worse — a round that swaps one
//!    Critical for another has not lost ground, and stopping there would give
//!    up the second round the budget allows. But a count cannot express LOSS: a
//!    rewrite that traded two fabricated metrics for one dropped employer
//!    scored as an improvement and deleted a job from the résumé. See
//!    [`round_is_worse`].
//! 5. **No error here fails the run.** One section's provider error is a FAILED
//!    ATTEMPT (the other sections still get their turn); the day's provider cap
//!    refusing a call is [`StoppedReason::Budgeted`], which keeps the progress
//!    already accumulated. A `?` on either used to throw away a document the
//!    run had already produced — the exact opposite of what every
//!    `StoppedReason` in this crate promises.
//! 6. **The run deadline is checked HERE**, between rounds and between
//!    per-section calls. `StageHooks::before` cannot bound this stage: it is
//!    the last one, so there is no later boundary, and it is the only stage
//!    that fans out (≤2 × 4 provider calls at up to `OLLAMA_COMPLETION_BASELINE`
//!    each — `Completer::complete` carries no effort to scale that by).
//!
//! Never cached: a repair reads a validator verdict, and a cached correction to
//! a document that no longer exists is the worst possible hit.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use async_trait::async_trait;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::prompts::{repair_system, repair_user};
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::{projects, QualityCtx, RunDeadline};
use crate::pipeline::{Completer, Stage};
use crate::validate::content::{
    ContentIssue, ContentReport, FACTUAL_ALTERED_PROJECT_LINK, FACTUAL_DROPPED_ROLE,
};
use crate::validate::Severity;

use super::sections;
use super::validate::{counts, validate_documents};

pub struct Repair;

/// This stage's name — also the override a per-section REGENERATE click routes
/// through when it takes the text-rewrite path, which is this stage's own
/// prompt and grounding. See `commands::resume_pipeline::resume_pipeline_regenerate_section`.
pub const NAME: &str = "repair";

/// Sections one round may regenerate.
///
/// The round's cost is one provider call per section, and the run deadline is
/// derived from exactly this number (`QUALITY_RUN_FIXED_SECS` counts
/// `max_repair_attempts × MAX_SECTIONS_PER_ROUND` calls at
/// `timeouts::OLLAMA_COMPLETION_BASELINE`), which is why
/// `quality_run_deadline_clears_the_inner_per_call_bounds` reads this constant
/// rather than a literal: raising it without raising the deadline fails that
/// test. Four is every section a quality-depth draft actually has that can
/// carry a factual Critical (summary, skills, experience, projects) — a bound,
/// not a target: a round almost always touches one.
pub const MAX_SECTIONS_PER_ROUND: usize = 4;

/// One issue rendered for the prompt: the code, the human message, and the
/// offending span. Content-bearing by necessity — this goes to the MODEL, not
/// to a log — and it rides inside `<section_issues>`, fenced like everything
/// else in the user turn.
///
/// `pub(super)` (rather than private) so `humanize.rs` — the SIBLING stage
/// with the same "render one issue for the model" need — reuses this exact
/// format for its own `<humanize_findings>` block instead of a second,
/// driftable copy.
pub(super) fn issue_line(issue: &ContentIssue) -> String {
    match &issue.evidence {
        Some(evidence) => format!(
            "[{}] {} — offending text: {evidence}",
            issue.code, issue.message
        ),
        None => format!("[{}] {}", issue.code, issue.message),
    }
}

/// Group a report's CRITICALS by the section that has to be regenerated,
/// WORST FIRST.
///
/// Two ways to locate one, in order:
///
/// 1. the validator's own `section` label, when it set one;
/// 2. otherwise, the section whose text CONTAINS the offending span.
///
/// The fallback is load-bearing, not defensive: the `factual.*` family — the
/// codes this loop exists for — reports `section: None` by design, because a
/// fabricated metric is found by comparing the document's numbers against the
/// source's, not by walking sections. Grouping on the label alone left the
/// commonest Critical unrepairable, which is the silent version of a repair
/// loop that does nothing.
///
/// A Critical that resolves to neither (a document-wide language mismatch, a
/// finding in the leading band) is deliberately excluded: there is no section to
/// regenerate, and re-running one at random would not fix it. Those survive to
/// the terminal review.
///
/// **The ORDER is the reason this returns a `Vec` rather than the `BTreeMap` it
/// builds.** A round can only afford [`MAX_SECTIONS_PER_ROUND`] sections, and a
/// map is ordered by wire key — `education` < `experience:0` < `projects` <
/// `skills` < `summary` — so a document with five failing sections would starve
/// `summary` deterministically, every round, forever. Worst-first (most
/// criticals, wire key as a stable tie-break) spends the budget where the
/// document is most wrong.
pub(crate) fn criticals_by_section(
    document: &str,
    report: &ContentReport,
) -> Vec<(String, Vec<String>)> {
    let split = sections::split(document);
    let lines: Vec<&str> = document.lines().collect();
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for issue in report
        .issues
        .iter()
        .filter(|issue| issue.severity == Severity::Critical)
    {
        let key = sections::key_for_label(issue.section.as_deref()).or_else(|| {
            let span = issue.evidence.as_deref()?;
            sections::containing(&split, &lines, span)
                .and_then(|section| sections::key_of(section.kind))
        });
        let Some(key) = key else { continue };
        grouped
            .entry(key.to_wire())
            .or_default()
            .push(issue_line(issue));
    }
    let mut ordered: Vec<(String, Vec<String>)> = grouped.into_iter().collect();
    // `sort_by_key` is stable, and the input came out of a BTreeMap, so equal
    // counts keep their wire-key order — deterministic without a second key.
    ordered.sort_by_key(|(_, issues)| std::cmp::Reverse(issues.len()));
    ordered
}

/// Whether a repair round's candidate must be discarded.
///
/// TWO terms, and the second is not a count.
///
/// **The count term.** Strictly more criticals than the draft it was trying to
/// fix. The strictness is a decision with a gradient in both directions: `>=`
/// would abandon a round that traded one Critical for another — no ground lost,
/// and the budget's second round is exactly the chance to get it right — while
/// `after > before + n` would let a repair ship a document measurably worse
/// than the one it replaced.
///
/// **The ABSENCE term, and why a count alone was not enough.** A repair round
/// hands the model a whole section as free text and splices the answer back, so
/// it can lose content the source had. Losing content is not commensurable with
/// fixing a fabrication, and the count said it was: an assembled document
/// carrying TWO `factual.unsourced_metric` Criticals, "repaired" by a rewrite
/// that removed the invented figures and also dropped an employer, came back
/// with ONE `factual.dropped_role` — 1 < 2, an improvement by the only measure
/// the loop had. The round was kept, the document was saved, and an employer
/// the candidate actually worked for was gone from the résumé. Worse still, the
/// user could not undo it: an absence has no span, so it is deliberately not a
/// reviewable finding (see `commands::resume_pipeline::report::fabrications`) —
/// the run says "needs review" and the panel shows nothing to act on.
///
/// So a round that INTRODUCES an absence is worse whatever the totals say. The
/// comparison is by `(code, evidence)` PAIR, not by code: a document that
/// already lost a role must still be repairable (its own pair is carried, not
/// new), while a round that swaps WHICH employer is missing has introduced a
/// loss and is caught.
///
/// This is a compatible TIGHTENING of rule 4 in the module doc — that rule was
/// always "never hand back a worse document"; this says what the count could
/// not express. It fixes quality depth as well as max: quality's repair loop is
/// the same loop, and its draft carries the same employers.
pub(crate) fn round_is_worse(
    before: &ContentReport,
    before_text: &str,
    after: &ContentReport,
    after_text: &str,
) -> bool {
    if criticals_of(after) > criticals_of(before) {
        return true;
    }
    let carried = absences(before, before_text);
    absences(after, after_text)
        .into_iter()
        .any(|pair| !carried.contains(&pair))
}

/// The ABSENCE-shaped Criticals in one report, as `(code, evidence)` pairs.
///
/// An absence-shaped finding names something the document is MISSING, so its
/// evidence is by definition not in the document — which is exactly why the
/// review panel cannot offer a verdict on one, and why a repair round is not
/// allowed to create one.
///
/// Two codes qualify, and the second only conditionally:
///
/// * `factual.dropped_role` — always. It names an employer the source has and
///   the output does not.
/// * `factual.altered_project_link` — only on its ABSENCE arm. That code is
///   emitted from two: a link the model INVENTED sits in the generated text (a
///   fabrication, reviewable, and a repair that produces one is caught by the
///   count like any other), while a SOURCE link missing or altered in the
///   output names a loss. The discriminator is the same one
///   `commands::resume_pipeline::report::fabrications` uses to keep that arm out
///   of the panel — is the evidence present in the document — so the two places
///   that decide "is this an absence" cannot disagree.
///
/// Scoped to those two rather than a general "evidence not in the text" gate,
/// for the reason `report::fabrications` records: `factual.unsourced_term`'s
/// evidence is a NORMALIZED token ("kubernetes" for a document that says
/// "Kubernetes"), so a blanket presence test would call ordinary fabrications
/// absences and freeze the repair loop.
///
/// **An issue with NO evidence is skipped**, because the pair is what makes a
/// pre-existing absence carryable rather than a permanent block. Both codes
/// always carry one — `factual.dropped_role`'s is the employer name it could
/// not find (`factual::dropped_roles` passes `Some(company)` unconditionally),
/// and the link check's is the URL — which is pinned by
/// `a_repair_rewrite_that_drops_a_seeded_employer_raises_a_dropped_role_critical`
/// rather than assumed here.
///
/// **Accepted blind spot, stated rather than hidden:** `validate_content` caps
/// a report at `MAX_CONTENT_ISSUES` (200) criticals-first, so a report holding
/// more than 200 Criticals could in principle have an absence truncated out of
/// it and this would not see it. Reaching that needs BOTH reports pinned at the
/// cap (the count term reverts anything that grows past it), i.e. a document
/// with 200+ deterministic Criticals — one already so broken that "which
/// finding got cut" is not the user's problem. The alternatives are a
/// pre-cap count field on `ContentReport` (a wire + persisted contract change)
/// or exempting absences from truncation (a second ordering rule inside the one
/// capper); neither is worth buying at that reachability, and this comment is
/// the deliberate choice rather than an oversight.
fn absences<'a>(report: &'a ContentReport, text: &str) -> BTreeSet<(&'a str, &'a str)> {
    report
        .issues
        .iter()
        .filter(|issue| issue.severity == Severity::Critical)
        .filter_map(|issue| {
            let evidence = issue.evidence.as_deref()?.trim();
            let absent = issue.code == FACTUAL_DROPPED_ROLE
                || (issue.code == FACTUAL_ALTERED_PROJECT_LINK && !text.contains(evidence));
            absent.then_some((issue.code, evidence))
        })
        .collect()
}

/// What ONE section-regeneration attempt did. Three outcomes, not two, because
/// the middle one costs a provider call and the last one does not — and
/// counting a call that was never made inflates the run's own metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionOutcome {
    /// The whole document, with the section replaced.
    Replaced(String),
    /// A call was made and its answer was unusable (truncated / heading-less).
    /// A FAILED attempt, never spliced.
    Unusable,
    /// The document has no such section — nothing was asked of any provider.
    Missing,
}

/// Regenerate ONE section against a list of issues and splice it back.
///
/// The shared primitive behind both the repair loop and the per-section
/// regenerate button, which is why it takes plain values and returns the whole
/// document rather than mutating a context.
///
/// Charges the per-provider daily ceiling itself: this goes through
/// `Completer::complete`, which records spend but does not charge (its other
/// callers charge at admission), and a repair round is exactly the fan-out that
/// must not sit outside the day's cap.
pub async fn regenerate_one_section(
    completer: &Completer,
    source_resume: &str,
    target_language: &str,
    document: &str,
    key: SectionKey,
    issues: &[String],
    note: Option<&str>,
) -> AppResult<SectionOutcome> {
    let split = sections::split(document);
    let Some(section) = sections::find(&split, key) else {
        return Ok(SectionOutcome::Missing);
    };
    let lines: Vec<&str> = document.lines().collect();
    let current = section.text(&lines);

    completer.charge_daily()?;
    let replacement = completer
        .complete(
            &repair_system(target_language),
            &repair_user(source_resume, &current, issues, note),
            None,
        )
        .await?;
    let replacement = replacement.trim();
    if !sections::is_usable_replacement(replacement) {
        return Ok(SectionOutcome::Unusable);
    }
    Ok(SectionOutcome::Replaced(sections::splice(
        document,
        section,
        replacement,
    )))
}

/// What one whole repair loop did — everything the ledger and the stage
/// artifact report, so the loop itself stays pure.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepairStats {
    pub rounds: u32,
    pub reverted: bool,
    /// Calls that came back truncated/heading-less.
    pub truncated: u32,
    /// Calls that errored (not a budget refusal).
    pub failed: u32,
    /// Provider round-trips actually made.
    pub calls: u32,
    /// The day's provider ceiling refused a call mid-loop.
    pub budgeted: bool,
    /// The run's wall clock ran out mid-loop.
    pub timed_out: bool,
}

/// The whole repair loop, with the PROVIDER CALL injected.
///
/// Same seam shape, and the same reason, as
/// [`complete_json_with`](crate::pipeline::complete_json_with) and
/// `Completer::from_config`: a `Completer` is a concrete struct bound to an
/// `AppHandle`, this crate has no Tauri harness, and the loop's decisions —
/// revert-on-worse, per-section error policy, the deadline checks, worst-first
/// ordering — are exactly the ones that must be provable by a test rather than
/// by reading the code. `revalidate` is a seam too, but production and tests
/// both pass the REAL [`validate_documents`]: it needs no provider, and a
/// stubbed validator would let the loop's arithmetic pass against numbers no
/// validator would ever produce.
///
/// Only a `revalidate` failure is an `Err` — that is a `spawn_blocking` join
/// failure, i.e. the process, not the model.
///
/// `normalize` runs on the round's candidate AFTER the section splices and
/// BEFORE `revalidate` — the deterministic projects-normalization pass
/// (`pipeline::resume::projects::normalize_projects`) so the document the
/// validator grades is the one that is actually kept, and a repair round
/// "fixing" an unrelated Critical cannot leave a project link altered behind
/// it. `None` means no change, exactly like `normalize_projects`'s own
/// contract.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn repair_loop<F, Fut, N, G, GFut>(
    mut draft: String,
    mut report: ContentReport,
    mut letter: Option<ContentReport>,
    max_rounds: u32,
    deadline: RunDeadline,
    mut regenerate: F,
    normalize: N,
    mut revalidate: G,
) -> AppResult<(String, ContentReport, Option<ContentReport>, RepairStats)>
where
    F: FnMut(SectionKey, String, Vec<String>) -> Fut,
    Fut: Future<Output = AppResult<SectionOutcome>>,
    N: Fn(&str) -> Option<String>,
    G: FnMut(String) -> GFut,
    GFut: Future<Output = AppResult<(ContentReport, Option<ContentReport>)>>,
{
    let mut stats = RepairStats::default();

    while stats.rounds < max_rounds {
        if deadline.passed() {
            stats.timed_out = true;
            break;
        }
        let grouped = criticals_by_section(&draft, &report);
        if grouped.is_empty() {
            break; // clean, or only document-wide criticals
        }

        let mut candidate = draft.clone();
        let mut changed = false;
        for (wire_key, issues) in grouped.iter().take(MAX_SECTIONS_PER_ROUND) {
            // Between calls, not just between rounds: four calls at up to
            // `OLLAMA_COMPLETION_BASELINE` each is 20 minutes a round-granular
            // check would not interrupt.
            if deadline.passed() {
                stats.timed_out = true;
                break;
            }
            let Some(key) = SectionKey::from_wire(wire_key) else {
                continue;
            };
            match regenerate(key, candidate.clone(), issues.clone()).await {
                Ok(SectionOutcome::Replaced(spliced)) => {
                    stats.calls += 1;
                    candidate = spliced;
                    changed = true;
                }
                Ok(SectionOutcome::Unusable) => {
                    stats.calls += 1;
                    stats.truncated += 1;
                }
                // No call was made, so nothing is counted — a metric that
                // reported a round-trip here would over-report every run whose
                // validator named a section the document does not have.
                Ok(SectionOutcome::Missing) => {}
                // The day's ceiling refused this call. Every later section
                // would be refused the same way, and the run keeps whatever it
                // has: that is what `Budgeted` means.
                Err(AppError::RateLimited(_)) => {
                    stats.budgeted = true;
                    break;
                }
                // One section's provider error is one failed attempt. The
                // remaining sections still get their turn; if they all fail,
                // `changed` stays false and the loop ends on its own.
                Err(_) => stats.failed += 1,
            }
        }
        stats.rounds += 1;
        if !changed {
            // Every section in this round came back unusable, errored, or was
            // refused. Another round would ask the same model the same
            // question.
            break;
        }

        // Deterministic and zero-cost: re-render the Projects section from the
        // source-seeded truth BEFORE the candidate is graded, so the report
        // `revalidate` returns describes the document that is actually kept.
        if let Some(normalized) = normalize(&candidate) {
            candidate = normalized;
        }

        let (candidate_report, candidate_letter) = revalidate(candidate.clone()).await?;
        let (_, after) = counts(&candidate_report);

        // Both REPORTS and both TEXTS: the second term of the rule asks whether
        // this round INTRODUCED an absence, and an absence is a property of a
        // report read against the document it describes.
        if round_is_worse(&report, &draft, &candidate_report, &candidate) {
            // Revert AND stop. Nothing has to be undone: `draft` was never
            // written — the round worked on a CLONE, and the clone is simply
            // dropped. That is what makes the revert total rather than a
            // partial rollback of whichever sections landed first.
            stats.reverted = true;
            break;
        }
        draft = candidate;
        report = candidate_report;
        letter = candidate_letter;
        // A round that ran out of time still KEEPS what it produced (it is not
        // worse, and it is validated) — the deadline ends the loop, it does not
        // discard the work.
        if stats.timed_out || stats.budgeted || after == 0 {
            break;
        }
    }

    Ok((draft, report, letter, stats))
}

/// Criticals in one report — the same count [`counts`] returns, without
/// re-walking for the total.
fn criticals_of(report: &ContentReport) -> usize {
    counts(report).1
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Repair {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let Some(report) = ctx.report.take() else {
            // validate did not run — nothing to repair against.
            ctx.ledger.record("repair", json!({ "rounds": 0 }));
            return Ok(());
        };

        // Copied out of `ctx` so the two closures below borrow neither it nor
        // each other: `QualityInput` is `Copy` and `completer` is a shared ref.
        let input = ctx.input;
        let completer = ctx.completer_for(NAME);
        // Computed ONCE per run — every round's normalize call reads the same
        // seeds, exactly like `Draft::run`'s. The skip reason is discarded
        // here: only the draft-stage ledger reports it (rule 5).
        let (seeds, _seed_skip_reason) = projects::seed_projects_for_normalize(input.source_resume);
        // The `cover_letter` stage's own letter when it produced one, falling
        // back to the renderer-supplied validate-only text — see
        // `QualityCtx::letter_text`. Read BEFORE the closures below so neither
        // one borrows `ctx`, for the exact reason its own comment states.
        let letter_text = ctx.letter_text().to_string();

        let (draft, report, letter, stats) = repair_loop(
            std::mem::take(&mut ctx.draft),
            report,
            ctx.letter_report.take(),
            ctx.budget.max_repair_attempts as u32,
            ctx.deadline,
            |key, document, issues| async move {
                regenerate_one_section(
                    completer,
                    input.source_resume,
                    input.target_language,
                    &document,
                    key,
                    &issues,
                    None,
                )
                .await
            },
            |candidate: &str| projects::normalize_projects(candidate, &seeds),
            |candidate| {
                validate_documents(
                    candidate,
                    input.source_resume.to_string(),
                    input.job_ad.to_string(),
                    input.top_requirements.to_vec(),
                    input.target_language.to_string(),
                    letter_text.clone(),
                )
            },
        )
        .await?;

        ctx.draft = draft;
        ctx.report = Some(report);
        ctx.letter_report = letter;

        // Ordered most-specific first: `RunLedger::stop` keeps the EARLIEST
        // reason, so a run already cancelled or already out of time upstream
        // keeps its own, and `MaxRepairs` never masks a stop that has a
        // remedy the user can act on.
        if stats.timed_out {
            ctx.ledger.stop(StoppedReason::RunTimeout);
        }
        if stats.budgeted {
            ctx.ledger.stop(StoppedReason::Budgeted);
        }
        // Still failing after the budget: the run keeps its best document but
        // must never present it as clean.
        if ctx.critical_count() > 0 {
            ctx.ledger.stop(StoppedReason::MaxRepairs);
        }
        for _ in 0..stats.calls {
            ctx.ledger.count_call(false);
        }
        ctx.ledger.note_repair(stats.rounds, stats.reverted);
        // Counts only (ADR-027).
        ctx.ledger.record(
            "repair",
            json!({
                "rounds": stats.rounds,
                "reverted": stats.reverted,
                "truncatedAttempts": stats.truncated,
                "failedAttempts": stats.failed,
                "budgeted": stats.budgeted,
                "timedOut": stats.timed_out,
                "criticalsRemaining": ctx.critical_count(),
            }),
        );
        Ok(())
    }
}
