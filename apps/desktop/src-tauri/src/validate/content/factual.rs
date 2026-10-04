//! Factual grounding — the Critical family.
//!
//! Every check here compares the generated text against the candidate's own
//! source document(s). Nothing in this file consults a model, and nothing
//! guesses: if a comparison cannot be made reliably it is skipped, because a
//! false "you fabricated this" is worse than a missed one.

use std::collections::HashSet;

use crate::documents::keywords::{keywords_normalized, SHORT_TECH_TERMS, SYNONYMS};

use super::{issue, Analysis, ContentIssue, DocKind, FACTUAL_UNSOURCED_TERM};

mod employment;
mod links;
mod metrics;

use self::employment::{dropped_role_issues, unsupported_date_issues};
use self::links::project_link_issues;
use self::metrics::unsourced_metric_issues;

pub(super) use self::employment::count_roles;
pub use self::employment::{MAX_SCANNED_ENTRIES, MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS};
pub use self::links::{canonical_link, link_href, names_a_resource, urls_in};
// The metric surface below is reached only by the tests, which measure the extractors
// directly rather than through a report.
#[cfg(test)]
pub use self::employment::MIN_SURVIVAL_COMPANY_TOKEN_CHARS;
#[cfg(test)]
pub use self::metrics::{
    metrics_in, normalize_number, MetricKind, MIN_BARE_PHONE_LINE_DIGITS,
    MIN_WORDS_IN_LETTER_BODY_LINE,
};

/// Recognised technical vocabulary — the only tokens `unsourced_term` polices.
///
/// A term counts as technical when it is a short tech acronym the keyword
/// kernel already allowlists, or either side of one of the kernel's synonym
/// pairs. Restricting to the kernel's own vocabulary keeps this from firing on
/// ordinary prose the model legitimately rephrased.
fn is_technical_term(token: &str) -> bool {
    SHORT_TECH_TERMS.contains(&token)
        || SYNONYMS
            .iter()
            .any(|(alias, canon)| *alias == token || *canon == token)
}

/// `factual.unsourced_term` — a technical skill the output claims that neither
/// the source résumé nor the posting mentions.
///
/// A Warning, not a Critical: the source résumé's own phrasing may simply have
/// used a different word for the same thing, and the posting is a legitimate
/// second source for vocabulary the candidate genuinely has.
fn unsourced_term_issues(generated: &str, truth_texts: &[&str]) -> Vec<ContentIssue> {
    let known: HashSet<String> = truth_texts
        .iter()
        .flat_map(|t| keywords_normalized(t))
        .collect();
    let mut terms: Vec<String> = keywords_normalized(generated)
        .into_iter()
        .filter(|t| is_technical_term(t))
        .filter(|t| !known.contains(t))
        .collect();
    terms.sort(); // Deterministic order — the token set is a HashSet.
    terms
        .into_iter()
        .map(|term| {
            issue(
                FACTUAL_UNSOURCED_TERM,
                None,
                format!(
                    "\"{term}\" appears in the generated document but in neither your source \
                     résumé nor the job ad. Keep it only if you can speak to it in an interview."
                ),
                Some(term),
            )
        })
        .collect()
}

/// Every factual check for a résumé, in a stable order.
pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = unsourced_metric_issues(
        ctx.input.generated,
        ctx.input.source_resume,
        DocKind::Resume,
    );
    issues.extend(dropped_role_issues(ctx));
    issues.extend(unsupported_date_issues(ctx));
    issues.extend(project_link_issues(ctx));
    issues.extend(unsourced_term_issues(
        ctx.input.generated,
        &[ctx.input.source_resume, ctx.input.job_ad],
    ));
    issues
}

/// The letter variant: a cover letter's truth base is the source résumé AND the
/// job ad (a letter legitimately quotes the posting's own numbers back), and it
/// has no roles, dates or projects section of its own to check.
pub(super) fn validate_letter(ctx: &Analysis) -> Vec<ContentIssue> {
    let truth = format!("{}\n{}", ctx.input.source_resume, ctx.input.job_ad);
    let mut issues = unsourced_metric_issues(ctx.input.generated, &truth, DocKind::CoverLetter);
    issues.extend(unsourced_term_issues(
        ctx.input.generated,
        &[ctx.input.source_resume, ctx.input.job_ad],
    ));
    issues
}
