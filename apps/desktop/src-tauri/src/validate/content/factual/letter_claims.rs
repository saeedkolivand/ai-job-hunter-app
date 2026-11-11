//! The same entry grounding for a cover letter, scoped to what a letter can
//! tie to an employer: a sentence that names exactly one of the candidate's
//! source employers is checked against that employer's source entry. Sentences
//! naming no employer, or several, are left alone.

use crate::validate::content::text::sentences;
use crate::validate::content::{contains_phrase, Analysis, ContentIssue};

use super::blocks::{all_blocks, keys_of, Kind};
use super::conflation::{term_issues, Grounding};
use super::terms::terms_in;

/// First identity word of an employer name, the one a letter would use ("at
/// Acme"). Short function words are not an identity.
fn primary_token(company: &str) -> Option<String> {
    crate::documents::evidence::identity_tokens(company, 3)
        .into_iter()
        .find(|t| !["the", "der", "die", "das", "and", "und"].contains(&t.as_str()))
}

pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let g = Grounding::new(ctx);
    let source = all_blocks(&ctx.source_sections);
    let roles: Vec<_> = source.iter().filter(|b| b.kind == Kind::Role).collect();
    let mut out = Vec::new();
    for sentence in sentences(ctx.input.generated) {
        let lower = sentence.to_lowercase();
        let named: Vec<_> = roles
            .iter()
            .copied()
            .filter(|b| primary_token(&b.identity).is_some_and(|t| contains_phrase(&lower, &t)))
            .collect();
        let Some(first) = named.first() else { continue };
        let same_employer = named
            .iter()
            .all(|b| primary_token(&b.identity) == primary_token(&first.identity));
        if !same_employer {
            continue;
        }
        let terms = terms_in(ctx, &sentence, &g.declared);
        out.extend(term_issues(
            &g,
            &terms,
            Some(&keys_of(ctx, &named)),
            true,
            &first.identity,
            None,
            None,
        ));
    }
    out
}
