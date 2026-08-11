//! `repair` — up to [`Budget::max_repair_attempts`] rounds of section-scoped
//! correction, and the four rules that keep it from making things worse.
//!
//! 1. **Criticals only.** Warnings are advice; regenerating a section to chase
//!    one spends a provider call to trade a stylistic nit for a fresh chance at
//!    a factual error.
//! 2. **Failing sections only**, spliced back in. A whole-document rewrite
//!    would re-roll the sections that were already correct.
//! 3. **A truncated section is a FAILED attempt**, not a smaller section —
//!    splicing one in deletes content silently (see
//!    [`sections::is_usable_replacement`]).
//! 4. **Strictly more criticals ⇒ revert and stop.** The repair is a bet that
//!    the model can fix what it broke; when the bet loses, the honest move is
//!    to hand back the draft that was merely wrong rather than the one that is
//!    now wrong in more places. Equal is not worse — a round that swaps one
//!    Critical for another has not lost ground, and stopping there would give
//!    up the second round the budget allows.
//!
//! Never cached: a repair reads a validator verdict, and a cached correction to
//! a document that no longer exists is the worst possible hit.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde_json::json;

use crate::error::AppResult;
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::prompts::{repair_system, repair_user};
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::QualityCtx;
use crate::pipeline::{Completer, Stage};
use crate::validate::content::{ContentIssue, ContentReport};
use crate::validate::Severity;

use super::sections;
use super::validate::{counts, validate_documents};

pub struct Repair;

/// Sections one round may regenerate.
///
/// The round's cost is one provider call per section, and the run deadline is
/// derived from at most one draft-equivalent per round (see
/// `qualityRunDeadlineSecs`). Four is every section a quality-depth draft
/// actually has that can carry a factual Critical (summary, skills, experience,
/// projects) — a bound, not a target: a round almost always touches one.
const MAX_SECTIONS_PER_ROUND: usize = 4;

/// One issue rendered for the prompt: the code, the human message, and the
/// offending span. Content-bearing by necessity — this goes to the MODEL, not
/// to a log — and it rides inside `<section_issues>`, fenced like everything
/// else in the user turn.
fn issue_line(issue: &ContentIssue) -> String {
    match &issue.evidence {
        Some(evidence) => format!(
            "[{}] {} — offending text: {evidence}",
            issue.code, issue.message
        ),
        None => format!("[{}] {}", issue.code, issue.message),
    }
}

/// Group a report's CRITICALS by the section that has to be regenerated.
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
pub(crate) fn criticals_by_section(
    document: &str,
    report: &ContentReport,
) -> BTreeMap<String, Vec<String>> {
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
    grouped
}

/// Whether a repair round's candidate must be discarded: it produced STRICTLY
/// more criticals than the draft it was trying to fix.
///
/// The strictness is the decision, and it has a gradient in both directions.
/// `>=` would abandon a round that traded one Critical for another — no ground
/// lost, and the budget's second round is exactly the chance to get it right.
/// A looser rule (`after > before + n`) would let a repair ship a document that
/// is measurably worse than the one it replaced, which is the failure mode the
/// revert exists for. Named and tested rather than inlined so that choice is
/// pinned instead of re-litigated by whoever next reads the comparison.
pub(crate) fn round_is_worse(before: usize, after: usize) -> bool {
    after > before
}

/// Regenerate ONE section against a list of issues and splice it back.
///
/// The shared primitive behind both the repair loop and the per-section
/// regenerate button, which is why it takes plain values and returns the whole
/// document rather than mutating a context. `Ok(None)` means the attempt
/// FAILED usably — a truncated or heading-less replacement — and the caller
/// must keep the original text.
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
) -> AppResult<Option<String>> {
    let split = sections::split(document);
    let Some(section) = sections::find(&split, key) else {
        return Ok(None);
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
        return Ok(None);
    }
    Ok(Some(sections::splice(document, section, replacement)))
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Repair {
    fn name(&self) -> &'static str {
        "repair"
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let mut rounds = 0u32;
        let mut reverted = false;
        let mut truncated = 0u32;

        while rounds < ctx.budget.max_repair_attempts as u32 {
            let Some(report) = ctx.report.as_ref() else {
                break; // validate did not run — nothing to repair against
            };
            let grouped = criticals_by_section(&ctx.draft, report);
            if grouped.is_empty() {
                break; // clean, or only document-wide criticals
            }
            let before = ctx.critical_count();

            let mut candidate = ctx.draft.clone();
            let mut changed = false;
            for (wire_key, issues) in grouped.iter().take(MAX_SECTIONS_PER_ROUND) {
                let Some(key) = SectionKey::from_wire(wire_key) else {
                    continue;
                };
                match regenerate_one_section(
                    ctx.completer,
                    ctx.input.source_resume,
                    ctx.input.target_language,
                    &candidate,
                    key,
                    issues,
                    None,
                )
                .await?
                {
                    Some(spliced) => {
                        candidate = spliced;
                        changed = true;
                    }
                    None => truncated += 1,
                }
                ctx.ledger.count_call(false);
            }
            rounds += 1;
            if !changed {
                // Every section in this round came back unusable. Another round
                // would ask the same model the same question.
                break;
            }

            let (candidate_report, candidate_letter) = validate_documents(
                candidate.clone(),
                ctx.input.source_resume.to_string(),
                ctx.input.job_ad.to_string(),
                ctx.input.top_requirements.to_vec(),
                ctx.input.target_language.to_string(),
                ctx.input.cover_letter.to_string(),
            )
            .await?;
            let (_, after) = counts(&candidate_report);

            if round_is_worse(before, after) {
                // Revert AND stop. Nothing has to be undone: `ctx.draft` was
                // never written — the round worked on a CLONE, and the clone is
                // simply dropped. That is what makes the revert total rather
                // than a partial rollback of whichever sections landed first.
                reverted = true;
                break;
            }
            ctx.draft = candidate;
            ctx.report = Some(candidate_report);
            ctx.letter_report = candidate_letter;
            if after == 0 {
                break;
            }
        }

        // Still failing after the budget: the run keeps its best document but
        // must never present it as clean.
        if ctx.critical_count() > 0 {
            ctx.ledger.stop(StoppedReason::MaxRepairs);
        }
        ctx.ledger.note_repair(rounds, reverted);
        // Counts only (ADR-027).
        ctx.ledger.record(
            "repair",
            json!({
                "rounds": rounds,
                "reverted": reverted,
                "truncatedAttempts": truncated,
                "criticalsRemaining": ctx.critical_count(),
            }),
        );
        Ok(())
    }
}
