use super::super::cache::StageIdentity;
use crate::validate::content::{
    validate_content, ContentInput, ContentIssue, ContentMetrics, ContentReport, DocKind,
    VOICE_AI_TELL_LEXICAL,
};
use crate::validate::Severity;

/// A routing identity, for the key tests below.
pub(super) fn id<'a>(
    provider: &'a str,
    model: &'a str,
    context_window: Option<u32>,
) -> StageIdentity<'a> {
    StageIdentity {
        provider,
        model,
        context_window,
        effort: None,
    }
}

/// The pipe form `export::parser` + `documents::evidence::split_entry` read is
/// `Title | Company | Dates` (a company-FIRST line is only recognised when the
/// first segment carries a legal form — see `split_entry`'s own doc). The
/// fixture follows that convention so the roster's `company` really is a
/// company; writing it the other way round would make every assertion below
/// about a job title.
pub(super) const THREE_ROLE_RESUME: &str = "Jane Doe\n\nWORK EXPERIENCE\n\nSenior Engineer | Acme Payments | 2021 - Present\n- Built the ledger\n\nEngineer | Beta Systems | 2019 - 2021\n- Shipped the API\n\nJunior Engineer | Gamma Industries | 2017 - 2019\n- Wrote tests\n";

pub(super) const DRAFTED: &str = "PROFESSIONAL SUMMARY\nA payments engineer.\n\nSKILLS\nGo, Rust\n\nWORK EXPERIENCE\nAcme | Engineer | 2021 - Present\n- Built the ledger\n";

/// A report of `n` ordinary (non-absence) Criticals, for the COUNT half of the
/// revert rule. `factual.unsourced_metric` because its evidence is a figure
/// that really is in the document, which is what makes it not an absence.
pub(super) fn criticals(count: usize) -> crate::validate::content::ContentReport {
    crate::validate::content::ContentReport {
        ok: count == 0,
        issues: (0..count)
            .map(|n| crate::validate::content::ContentIssue {
                severity: crate::validate::Severity::Critical,
                code: crate::validate::content::FACTUAL_UNSOURCED_METRIC,
                section: None,
                message: "an invented figure".to_string(),
                evidence: Some(format!("{n}0%")),
            })
            .collect(),
        metrics: crate::validate::content::ContentMetrics::default(),
    }
}

/// The document text these synthetic reports describe. Content-free on purpose:
/// the only presence question the rule asks is about
/// `factual.altered_project_link`'s evidence, which the
/// `..._project_link_...` test below supplies explicitly.
pub(super) const ANY_TEXT: &str = "Work Experience\n\nSenior Engineer, Acme  2021 - Present\n";

/// A source résumé and a draft that fabricates a metric in its summary — the
/// same pair the grouping test uses, so the loop is exercised on a document the
/// validator genuinely flags.
pub(super) const REPAIR_SOURCE: &str = "Jane Doe\n\nPROFESSIONAL SUMMARY\nA payments engineer.\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";

pub(super) const REPAIR_DRAFT: &str = "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 47% across 12 teams.\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";

/// The corrected summary: the fabricated figures are gone.
pub(super) const REPAIR_FIXED_SUMMARY: &str = "PROFESSIONAL SUMMARY\nA payments engineer.";

pub(super) fn repair_report(generated: &str) -> crate::validate::content::ContentReport {
    validate_content(&ContentInput {
        generated,
        source_resume: REPAIR_SOURCE,
        job_ad: "We need a payments engineer with ledger experience.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    })
}

/// The real validator, as the loop's `revalidate` seam.
pub(super) async fn repair_revalidate(
    candidate: String,
) -> crate::error::AppResult<(
    crate::validate::content::ContentReport,
    Option<crate::validate::content::ContentReport>,
)> {
    Ok((repair_report(&candidate), None))
}

/// A deadline that is already spent — `Duration::ZERO` is passed, so no test
/// ever sleeps.
pub(super) fn expired_deadline() -> super::super::RunDeadline {
    super::super::RunDeadline::starting_now(std::time::Duration::ZERO)
}

pub(super) fn live_deadline() -> super::super::RunDeadline {
    super::super::RunDeadline::starting_now(std::time::Duration::from_secs(3_600))
}

/// One `voice.*` Warning, with `evidence` as its offending span.
pub(super) fn voice_issue(evidence: &str) -> ContentIssue {
    ContentIssue {
        severity: Severity::Warning,
        code: VOICE_AI_TELL_LEXICAL,
        section: None,
        message: "on the AI-tell list the generator was told to avoid".to_string(),
        evidence: Some(evidence.to_string()),
    }
}

/// A report carrying one `voice.*` Warning per entry of `evidences`, and
/// nothing else — `ok` stays `true`, since a Warning never gates a report.
pub(super) fn voice_report(evidences: &[&str]) -> ContentReport {
    ContentReport {
        ok: true,
        issues: evidences.iter().map(|e| voice_issue(e)).collect(),
        metrics: ContentMetrics::default(),
    }
}

pub(super) fn ok_report() -> ContentReport {
    ContentReport {
        ok: true,
        issues: Vec::new(),
        metrics: ContentMetrics::default(),
    }
}
