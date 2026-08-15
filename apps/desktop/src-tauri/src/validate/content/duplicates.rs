//! Near-duplicate bullets.
//!
//! Two bullets that say the same thing in different words waste the only
//! resource a résumé has — the reader's attention — and are a classic symptom
//! of a model padding a role it has little evidence for.

use std::collections::HashSet;

use crate::export::types::ParsedLine;

use super::{issue, jaccard, Analysis, ContentIssue, DUPLICATE_BULLET};

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
    if bullets.len() < 2 {
        return (Vec::new(), 0.0);
    }
    let tokens: Vec<HashSet<String>> = bullets
        .iter()
        .map(|b| ctx.document.tokens(&b.text))
        .collect();

    let mut issues = Vec::new();
    let mut involved: HashSet<usize> = HashSet::new();
    // Document order, first occurrence wins — one issue per pair, reported on
    // the LATER bullet (the one to cut).
    for i in 0..bullets.len() {
        for j in (i + 1)..bullets.len() {
            if tokens[i].len() < MIN_TOKENS_FOR_DUPLICATE
                || tokens[j].len() < MIN_TOKENS_FOR_DUPLICATE
            {
                continue;
            }
            if jaccard(&tokens[i], &tokens[j]) < DUPLICATE_JACCARD_THRESHOLD {
                continue;
            }
            involved.insert(i);
            if involved.insert(j) {
                issues.push(issue(
                    DUPLICATE_BULLET,
                    None,
                    format!(
                        "This bullet repeats an earlier one almost word for word. Cut it, or \
                         replace it with a different result: \"{}\"",
                        bullets[j].text
                    ),
                    Some(bullets[j].text.clone()),
                ));
            }
        }
    }

    let ratio = involved.len() as f64 / bullets.len() as f64;
    (issues, ratio)
}
