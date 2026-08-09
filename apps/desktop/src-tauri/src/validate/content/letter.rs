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
//!
//! ## Not implemented: company-name mismatch
//!
//! The planned check "does the letter address the right company" needs the
//! posting's company name, which [`super::ContentInput`] does not carry, and
//! there is no code for it in [`super::CONTENT_ISSUE_CODES`]. Deriving the
//! company from the ad's prose would be a guess, and a wrong "you addressed the
//! wrong employer" is the least forgivable false positive this module could
//! produce. It stays out until the contract carries the name.

use super::{factual, voice, Analysis, ContentIssue};

pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = factual::validate_letter(ctx);
    issues.extend(voice::validate_letter(ctx));
    issues
}
