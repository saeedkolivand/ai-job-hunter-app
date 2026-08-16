//! `validate` — ZERO provider calls.
//!
//! Every Critical in this pipeline comes from here, and everything here is a
//! deterministic comparison against the SOURCE résumé. That is the mechanical
//! form of the core rule: **a model may never emit a Critical.**
//!
//! Never cached, and not because it would be hard: a validator verdict is the
//! thing the user is asked to trust, and a stale one reports a document that no
//! longer exists as clean.

use async_trait::async_trait;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::pipeline::resume::QualityCtx;
use crate::pipeline::Stage;
use crate::validate::content::{validate_content, ContentInput, ContentReport, DocKind};
use crate::validate::Severity;

pub struct Validate;

/// Run the deterministic checks for the résumé and, when one is in scope, the
/// cover letter.
///
/// Off the async workers via `spawn_blocking`, the same placement
/// `commands::resume::resume_validate_content` uses (the now-deleted
/// `agent::tools_quality` handlers did too): this is regex/token scanning
/// over up to three 200 KB documents plus capped
/// `O(n²)` duplicate detection, and a staged run does it up to three times
/// (once, then once per repair round). One scan, one answer about where it
/// runs.
///
/// Takes owned `String`s because that is what crossing a `spawn_blocking`
/// boundary costs; the alternative (borrowing across the join) does not exist.
pub async fn validate_documents(
    generated: String,
    source_resume: String,
    job_ad: String,
    top_requirements: Vec<String>,
    target_language: String,
    cover_letter: String,
) -> AppResult<(ContentReport, Option<ContentReport>)> {
    tokio::task::spawn_blocking(move || {
        let resume = validate_content(&ContentInput {
            generated: &generated,
            source_resume: &source_resume,
            job_ad: &job_ad,
            top_requirements: &top_requirements,
            target_language: &target_language,
            doc_kind: DocKind::Resume,
        });
        // The letter is measured against the candidate's own résumé AND the
        // posting, under the letter ruleset (`validate::content::letter`) — the
        // résumé-structure checks assume sections and bullets a letter does not
        // have. Its findings never gate the résumé's `ok`: they are a separate
        // report on a separate document.
        let letter = (!cover_letter.trim().is_empty()).then(|| {
            validate_content(&ContentInput {
                generated: &cover_letter,
                source_resume: &source_resume,
                job_ad: &job_ad,
                top_requirements: &top_requirements,
                target_language: &target_language,
                doc_kind: DocKind::CoverLetter,
            })
        });
        (resume, letter)
    })
    .await
    .map_err(|e| AppError::Storage(format!("content validation task failed: {e}")))
}

/// A report's issue counts, by severity — the summary every stage that produces
/// or consumes one records.
pub(crate) fn counts(report: &ContentReport) -> (usize, usize) {
    let criticals = report
        .issues
        .iter()
        .filter(|issue| issue.severity == Severity::Critical)
        .count();
    (report.issues.len(), criticals)
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Validate {
    fn name(&self) -> &'static str {
        "validate"
    }

    /// Deterministic — the module title's "ZERO provider calls", as something
    /// the run can act on. A document nothing checked is a document nothing may
    /// present as reviewed, so this has to survive the same deadline `assemble`
    /// does.
    fn costs_a_provider_call(&self) -> bool {
        false
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let (report, letter) = validate_documents(
            ctx.draft.clone(),
            ctx.input.source_resume.to_string(),
            ctx.input.job_ad.to_string(),
            // The RESOLVED list (analyze_job's own extraction, falling back
            // to the request's) — never `ctx.input.top_requirements`
            // directly. See `QualityCtx::top_requirements`'s doc.
            ctx.top_requirements(),
            ctx.input.target_language.to_string(),
            // The `cover_letter` stage's own letter when it produced one,
            // falling back to the renderer-supplied validate-only text — see
            // `QualityCtx::letter_text`.
            ctx.letter_text().to_string(),
        )
        .await?;

        let (issues, criticals) = counts(&report);
        // Codes and counts only — never the offending span (ADR-027). The full
        // report, evidence included, lives in the DB.
        ctx.ledger.record(
            "validate",
            json!({
                "issues": issues,
                "criticals": criticals,
                "letterIssues": letter.as_ref().map(|r| r.issues.len()),
                "codes": code_histogram(&report),
            }),
        );
        ctx.report = Some(report);
        ctx.letter_report = letter;
        Ok(())
    }
}

/// `code -> count` for one report. Codes are a fixed vocabulary
/// (`CONTENT_ISSUE_CODES`), never user text, so this is safe to log and to
/// persist in the event trail.
pub(crate) fn code_histogram(report: &ContentReport) -> serde_json::Value {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for issue in &report.issues {
        *counts.entry(issue.code).or_default() += 1;
    }
    json!(counts)
}
