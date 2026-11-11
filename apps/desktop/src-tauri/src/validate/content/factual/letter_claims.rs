//! The same entry grounding for a cover letter, scoped to what a letter can
//! tie to an employer. A sentence is tied to a source employer only when it
//! names the employer the way a person would: the FULL distinctive name, each
//! word capitalised ("Digital Logistics", never "digital payment systems"), or
//! a single-word name capitalised mid-sentence. Only that tie can raise a
//! Critical. A sentence that merely carries the employer's first word
//! capitalised is Warning-only, and anything else is left alone, as are
//! sentences naming several employers.

use std::collections::HashSet;

use crate::documents::evidence::identity_tokens;
use crate::validate::content::text::sentences;
use crate::validate::content::{Analysis, ContentIssue};

use super::blocks::{all_blocks, keys_of, Block, Kind};
use super::conflation::{term_issues, Grounding};
use super::terms::terms_in;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tie {
    Full,
    Partial,
}

const FUNCTION_WORDS: &[&str] = &["the", "der", "die", "das", "and", "und"];

fn name_tokens(company: &str) -> Vec<String> {
    identity_tokens(company, 3)
        .into_iter()
        .filter(|t| !FUNCTION_WORDS.contains(&t.as_str()))
        .collect()
}

/// Lowercased words of `sentence` written with a capital, and the same
/// excluding the sentence's first word (where a capital proves nothing).
fn capitalised(sentence: &str) -> (HashSet<String>, HashSet<String>) {
    let mut any = HashSet::new();
    let mut mid = HashSet::new();
    let words = sentence
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty());
    for (i, word) in words.enumerate() {
        if word.starts_with(char::is_uppercase) {
            any.insert(word.to_lowercase());
            if i > 0 {
                mid.insert(word.to_lowercase());
            }
        }
    }
    (any, mid)
}

fn tie(sentence: &str, company: &str) -> Option<Tie> {
    let tokens = name_tokens(company);
    let (any, mid) = capitalised(sentence);
    match tokens.as_slice() {
        [] => None,
        [only] => mid.contains(only).then_some(Tie::Full),
        [first, ..] => {
            if tokens.iter().all(|t| any.contains(t)) {
                Some(Tie::Full)
            } else {
                mid.contains(first).then_some(Tie::Partial)
            }
        }
    }
}

pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let g = Grounding::new(ctx);
    let source = all_blocks(&ctx.source_sections);
    let roles: Vec<&Block> = source.iter().filter(|b| b.kind == Kind::Role).collect();
    let mut out = Vec::new();
    for sentence in sentences(ctx.input.generated) {
        let named: Vec<(&Block, Tie)> = roles
            .iter()
            .filter_map(|b| tie(&sentence, &b.identity).map(|t| (*b, t)))
            .collect();
        let Some((first, _)) = named.first() else {
            continue;
        };
        let same_employer = named
            .iter()
            .all(|(b, _)| name_tokens(&b.identity) == name_tokens(&first.identity));
        if !same_employer {
            continue;
        }
        let full = named.iter().any(|(_, t)| *t == Tie::Full);
        let blocks: Vec<&Block> = named.iter().map(|(b, _)| *b).collect();
        let terms = terms_in(ctx, &sentence, &g.declared);
        out.extend(term_issues(
            &g,
            &terms,
            Some(&keys_of(ctx, &blocks)),
            full,
            &first.identity,
            None,
            None,
        ));
    }
    out
}
