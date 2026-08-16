//! Cover-letter dispatch.
//!
//! A letter is prose, not a résumé: it has no sections, no roles, no bullets
//! and no projects, so every résumé-structure check is skipped rather than
//! made to fail quietly. What it DOES get:
//!
//! * **Factual grounding** against the source résumé ∪ the job ad. The union
//!   matters — a letter legitimately quotes the posting's own numbers ("your
//!   500-person engineering org") and flagging those as fabrication would be
//!   wrong.
//! * **Language** — handled in [`super::validate_content`] for both kinds.
//! * **Voice**, with the full prose tier (template opener, rhythm,
//!   rule-of-three, em-dash, genericness), which résumé bullets are exempt from.
//! * **Template-placeholder detection** — an unfilled slot the model
//!   reproduced verbatim from a letter template (e.g. German "Ihr Name")
//!   fires `letter.template_placeholder` (Critical). See ADR-034
//!   Consequence #2.
//!
//! ## Not implemented: company-name mismatch
//!
//! The planned check "does the letter address the right company" needs the
//! posting's company name, which [`super::ContentInput`] does not carry, and
//! there is no code for it in [`super::CONTENT_ISSUE_CODES`]. Deriving the
//! company from the ad's prose would be a guess, and a wrong "you addressed the
//! wrong employer" is the least forgivable false positive this module could
//! produce. It stays out until the contract carries the name.

use super::{factual, issue, voice, Analysis, ContentIssue, LETTER_TEMPLATE_PLACEHOLDER};
use crate::locale::is_template_placeholder;

/// `letter.template_placeholder` — an unfilled template-placeholder slot
/// (e.g. German "Ihr Name") survived into the rendered letter text. See
/// ADR-034 Consequence #2: the prompt tells the model not to write these, but
/// nothing upstream of this deterministic check catches the model
/// reproducing a template's own slot syntax while otherwise following the
/// prompt. Reuses [`is_template_placeholder`], the same predicate the letter
/// parser uses, so the pattern list lives in exactly one place.
fn template_placeholder_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    ctx.input
        .generated
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && is_template_placeholder(l))
        .map(|l| {
            issue(
                LETTER_TEMPLATE_PLACEHOLDER,
                None,
                format!(
                    "\"{l}\" looks like an unfilled template placeholder, not real content. \
                     Replace it before sending this letter."
                ),
                Some(l.to_string()),
            )
        })
        .collect()
}

pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = factual::validate_letter(ctx);
    issues.extend(voice::validate_letter(ctx));
    issues.extend(template_placeholder_issues(ctx));
    issues
}
