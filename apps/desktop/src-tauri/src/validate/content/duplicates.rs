//! Near-duplicate bullets.
//!
//! Two bullets that say the same thing in different words waste the only
//! resource a résumé has — the reader's attention — and are a classic symptom
//! of a model padding a role it has little evidence for.

use std::collections::HashSet;

use crate::documents::evidence::SectionKind;
use crate::export::types::ParsedLine;

use super::factual::section_blocks;
use super::{
    issue, jaccard, Analysis, ContentIssue, DUPLICATE_BULLET, DUPLICATE_CROSS_ROLE_BULLET,
};

/// Token-set overlap at or above which two bullets are "the same bullet
/// twice". 0.8 means four of every five distinct content words are shared —
/// high enough that reordering or a synonym swap is still caught, low enough
/// that two bullets about the same system are not.
pub const DUPLICATE_JACCARD_THRESHOLD: f64 = 0.8;

/// Below this many content tokens a bullet is too short to compare: "Ran the
/// standup" and "Ran the retro" both normalize to one or two tokens, where
/// Jaccard is all noise.
pub const MIN_TOKENS_FOR_DUPLICATE: usize = 4;

/// Hard cap on how many bullets the near-duplicate scan below considers. The
/// scan is O(n²) — every bullet compared against every later bullet via
/// [`super::jaccard`] — so an unbounded bullet count (a malformed or hostile
/// "generated" document with thousands of bullet-shaped lines) held a tokio
/// worker for seconds on every save. A real résumé/letter never has anywhere
/// near this many bullets; the cap only ever bites on already-broken input.
/// Mirrors the count-cap discipline the now-deleted `agent::tools_quality`'s
/// `MAX_ISSUES`/`MAX_SKILLS` used for the same "untrusted size, bound before
/// doing the expensive thing" reason.
pub const MAX_DUP_BULLETS: usize = 400;

/// Returns the duplicate issues and the `duplicateRatio` metric — the share of
/// bullets involved in at least one near-duplicate pair (0.0–1.0), among the
/// first [`MAX_DUP_BULLETS`] bullets in document order.
///
/// ## The stemming decision is the DOCUMENT's, not the posting's
///
/// Both bullets come from the generated document; the posting is not a party to
/// the comparison. Under [`Analysis::tokens`] — the résumé↔POSTING decision —
/// the `duplicateRatio` of one unchanged document changed with the AD's
/// language, which is a measurement depending on an input it does not measure.
/// So this reads [`super::DocumentTokens`], the same per-document decision
/// `consistency`'s two self-comparisons take.
///
/// The direction differs from theirs and is stated rather than hidden: merging
/// tokens raises Jaccard, so this check can now report a pair a foreign-language
/// ad used to hide. That is the correct reading — two German bullets differing
/// only in declension ARE the same bullet twice — and it is a Warning either way.
pub(super) fn validate(ctx: &Analysis) -> (Vec<ContentIssue>, f64) {
    let mut bullets: Vec<&ParsedLine> = ctx
        .generated_sections
        .iter()
        .flat_map(|s| s.bullets())
        .collect();
    bullets.truncate(MAX_DUP_BULLETS);
    // Which role/project entry each bullet sits under (by identity of the line).
    // Only Experience/Projects have entries: a Summary line is not "another role".
    let mut entry_of: std::collections::HashMap<*const ParsedLine, (usize, Option<&str>)> =
        std::collections::HashMap::new();
    let mut n = 0;
    for section in &ctx.generated_sections {
        if !matches!(
            section.kind,
            SectionKind::Experience | SectionKind::Projects
        ) {
            continue;
        }
        for block in section_blocks(section) {
            n += 1;
            for line in block.body {
                entry_of.insert(line as *const _, (n, section.heading.as_deref()));
            }
        }
    }
    if bullets.len() < 2 {
        return (Vec::new(), 0.0);
    }
    let tokens: Vec<HashSet<String>> = bullets
        .iter()
        .map(|b| ctx.document.tokens(&b.text))
        .collect();

    let mut issues = Vec::new();
    let mut involved: HashSet<usize> = HashSet::new();
    // One issue per bullet that repeats ANY earlier one, reported on the LATER
    // bullet (the one to cut). Cross-entry is decided over ALL its earlier
    // matches, so a Summary copy seen first cannot mask a role-to-role copy.
    let comparable = |k: usize| tokens[k].len() >= MIN_TOKENS_FOR_DUPLICATE;
    let entry = |b: &ParsedLine| entry_of.get(&(b as *const _)).copied();
    for j in 1..bullets.len() {
        if !comparable(j) {
            continue;
        }
        let matches: Vec<usize> = (0..j)
            .filter(|&i| {
                comparable(i) && jaccard(&tokens[i], &tokens[j]) >= DUPLICATE_JACCARD_THRESHOLD
            })
            .collect();
        if matches.is_empty() {
            continue;
        }
        involved.extend(matches.iter().copied());
        involved.insert(j);
        let cross = matches.iter().any(|&i| {
            entry(bullets[i])
                .zip(entry(bullets[j]))
                .is_some_and(|(a, b)| a.0 != b.0)
        });
        let (code, lead) = if cross {
            (
                DUPLICATE_CROSS_ROLE_BULLET,
                "This bullet is copied from another role. A claim belongs to one role \
                 only; cut it or replace it with something this role actually did",
            )
        } else {
            (
                DUPLICATE_BULLET,
                "This bullet repeats an earlier one almost word for word. Cut it, or \
                 replace it with a different result",
            )
        };
        // Only the cross-role Critical needs a section: it is what
        // `repair` regenerates, so the run cannot park unresolvable.
        let section = entry(bullets[j]).and_then(|e| e.1).filter(|_| cross);
        issues.push(issue(
            code,
            section,
            format!("{lead}: \"{}\"", bullets[j].text),
            Some(bullets[j].text.clone()),
        ));
    }

    let ratio = involved.len() as f64 / bullets.len() as f64;
    (issues, ratio)
}
