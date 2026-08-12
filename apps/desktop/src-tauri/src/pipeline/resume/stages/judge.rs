//! `llm_judge` — max depth only, Warning-only, and SKIPPABLE.
//!
//! The one stage in this pipeline where a model's opinion reaches the report at
//! all. Three properties make that safe, and all three are structural rather
//! than procedural:
//!
//! 1. **A model may never emit a Critical.** [`JudgeItem`] has no severity
//!    field, and every issue this stage builds is constructed with
//!    [`Severity::Warning`] written at the construction site. There is no filter
//!    to forget and no field to invert.
//! 2. **A remark must be findable.** Its `quote` has to appear VERBATIM in the
//!    document, or the item is dropped — the same rule `match_evidence` applies
//!    to a source quote, for the same reason: advice about a sentence the reader
//!    cannot locate is not advice.
//! 3. **Nothing here can fail a run.** A provider error, an expired deadline, a
//!    refused daily cap, an unparseable answer — every one of them records
//!    `{skipped: true, reasonCode}` and returns `Ok`. The judge is the last and
//!    least important thing a max run does; a run that produced a validated,
//!    repaired document must never be reported as failed because an optional
//!    opinion did not arrive.
//!
//! ## Why it runs LAST, after `repair`
//!
//! The settled stage order put the judge between `validate` and `repair`. It
//! runs after both instead, for two reasons that only show up once the repair
//! loop is real:
//!
//! * `repair` REPLACES `ctx.report` with the one it re-validated, so judge items
//!   merged in before it are silently discarded on exactly the documents that
//!   were repaired — the ones most worth commenting on;
//! * a repaired section is rewritten text, so a quote taken before the repair
//!   names a sentence the finished document no longer contains, and rule 2 above
//!   would drop it (or, worse, keep it and point the reader at nothing).
//!
//! Running last, the judge reads the document the user will actually see.

use async_trait::async_trait;
use serde_json::json;

use crate::error::AppResult;
use crate::pipeline::resume::section_prompts::{judge_system, judge_user};
use crate::pipeline::resume::types_max::{JudgeItem, JudgeOut};
use crate::pipeline::resume::QualityCtx;
use crate::pipeline::Stage;
use crate::validate::content::{
    ContentIssue, ISSUE_EVIDENCE_MAX_BYTES, ISSUE_MESSAGE_MAX_BYTES, ISSUE_SECTION_MAX_BYTES,
    JUDGE_CLARITY, JUDGE_EVIDENCE, JUDGE_NOTE, JUDGE_TAILORING,
};
use crate::validate::Severity;

pub struct Judge;

pub const NAME: &str = "llm_judge";

/// Remarks one judge pass may contribute.
///
/// The prompt asks for at most six; this is the ceiling that holds when the
/// model ignores it. Deliberately small: the report already carries every
/// deterministic finding, and a judge that adds thirty advisory notes buries
/// them.
pub const MAX_JUDGE_ITEMS: usize = 6;

/// Why a judge pass produced nothing. A CLOSED vocabulary — these land in the
/// stage artifact and in the app log, so they are codes, never a provider's
/// error text (ADR-027).
const SKIP_NO_DOCUMENT: &str = "no_document";
const SKIP_CALL_FAILED: &str = "call_failed";
const SKIP_NO_USABLE_ITEMS: &str = "no_usable_items";

/// The `kind` → issue-code table. A kind outside it is not an error and not
/// dropped: it becomes the generic note code, because the remark may still be
/// useful and the model's taxonomy is the least important part of it.
fn code_for(kind: &str) -> &'static str {
    match kind.trim().to_ascii_lowercase().as_str() {
        "clarity" => JUDGE_CLARITY,
        "evidence" => JUDGE_EVIDENCE,
        "tailoring" => JUDGE_TAILORING,
        _ => JUDGE_NOTE,
    }
}

/// Turn one judge remark into a report issue — or drop it.
///
/// **`Severity::Warning` is written HERE, at the construction site.** Not read
/// from a table, not filtered afterwards: the whole point is that no code path
/// exists by which this stage can produce anything else.
///
/// Dropped when the remark has no note, or when its `quote` is not in the
/// document verbatim. The quote is what makes the advice actionable, and a
/// model that paraphrases the document it was shown is a model whose remark the
/// reader cannot check.
///
/// The three untrusted fields are clamped to the SAME byte caps
/// `validate::content::issue` applies, because they are the same fields with
/// the same job — and unlike a validator's spans, these come straight out of a
/// model.
pub fn issue_from(item: &JudgeItem, document: &str) -> Option<ContentIssue> {
    let note = item.note.trim();
    if note.is_empty() {
        return None;
    }
    let quote = item.quote.trim();
    if quote.is_empty() || !document.contains(quote) {
        return None;
    }
    let section = item.section.trim();
    Some(ContentIssue {
        severity: Severity::Warning,
        code: code_for(&item.kind),
        section: (!section.is_empty()).then(|| clamp(section.to_string(), ISSUE_SECTION_MAX_BYTES)),
        message: clamp(note.to_string(), ISSUE_MESSAGE_MAX_BYTES),
        evidence: Some(clamp(quote.to_string(), ISSUE_EVIDENCE_MAX_BYTES)),
    })
}

fn clamp(text: String, max: usize) -> String {
    crate::applications::clamp_to_bytes(text, max)
}

/// Every usable remark from one answer, capped.
pub fn issues_from(answer: &JudgeOut, document: &str) -> Vec<ContentIssue> {
    answer
        .items
        .iter()
        .filter_map(|item| issue_from(item, document))
        .take(MAX_JUDGE_ITEMS)
        .collect()
}

/// `code -> count` for the judge's own contribution, for the stage artifact.
/// Codes are the fixed `judge.*` vocabulary, never model text.
fn histogram(issues: &[ContentIssue]) -> serde_json::Value {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for issue in issues {
        *counts.entry(issue.code).or_default() += 1;
    }
    json!(counts)
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Judge {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let skip = |ctx: &mut QualityCtx<'a>, reason: &'static str| {
            ctx.ledger
                .record(NAME, json!({ "skipped": true, "reasonCode": reason }));
        };

        // Nothing to judge: an empty document, or a run whose validate stage
        // never produced a report to merge into.
        if ctx.draft.trim().is_empty() || ctx.report.is_none() {
            skip(ctx, SKIP_NO_DOCUMENT);
            return Ok(());
        }

        // Deliberately NOT cached. A judge reads the FINAL document, which is
        // the one thing in a run that repair can still change, and a cached
        // opinion of a document that no longer exists is the same stale-verdict
        // failure `validate` refuses caching for.
        let answer: JudgeOut = match ctx
            .completer
            .complete_json(
                ctx.deadline_guard(),
                &judge_system(ctx.input.target_language),
                &judge_user(&ctx.draft, ctx.input.job_ad, &ctx.analysis),
                JudgeOut::EXAMPLE,
                Some(&JudgeOut::schema()),
            )
            .await
        {
            Ok(answer) => answer,
            // EVERY failure mode lands here — a provider error, the day's cap
            // refusing the call, the run deadline expiring inside
            // `complete_json`'s guard, and an answer that would not parse after
            // its one re-ask. None of them is worth failing a run over, and the
            // reason code is what the log carries instead of the error text.
            Err(_) => {
                skip(ctx, SKIP_CALL_FAILED);
                return Ok(());
            }
        };

        let issues = issues_from(&answer, &ctx.draft);
        if issues.is_empty() {
            skip(ctx, SKIP_NO_USABLE_ITEMS);
            return Ok(());
        }

        ctx.ledger.count_call(false);
        // Counts and codes only (ADR-027): `dropped` is how many remarks failed
        // the verbatim-quote rule, which is the one number that says whether the
        // model is quoting the document or paraphrasing it.
        ctx.ledger.record(
            NAME,
            json!({
                "skipped": false,
                "items": issues.len(),
                "dropped": answer.items.len().saturating_sub(issues.len()),
                "codes": histogram(&issues),
            }),
        );
        if let Some(report) = ctx.report.as_mut() {
            report.issues.extend(issues);
            // …and re-apply the SAME cap `validate_content` applied, because
            // this merge just put the list back over it. `MAX_CONTENT_ISSUES`
            // is what `QUALITY_REPORT_MAX_BYTES` is derived from, and a report
            // that overshoots it is truncated mid-JSON at the save path and
            // silently discarded in favour of the previous one. Criticals-first
            // is why a judge Warning can never be what evicts a real Critical —
            // and calling the one capper again, rather than re-implementing the
            // truncation here, is why there is still only one of them.
            crate::validate::content::cap_issues(&mut report.issues);
            // `ok` is NOT recomputed, on purpose: it means "no Critical", the
            // judge cannot produce one, and a Warning has never flipped it.
        }
        Ok(())
    }
}
