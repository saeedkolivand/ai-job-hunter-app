//! Alignment with the posting — did tailoring actually help?
//!
//! Both checks are RELATIVE, never absolute. An absolute coverage threshold
//! would punish a candidate for a posting that asks for things they have not
//! done, which is not a document defect. Comparing the generated document
//! against the SOURCE's own coverage of the same posting isolates what the
//! generation step did.

use super::{
    issue, Analysis, ContentIssue, ALIGNMENT_LOW_COVERAGE, ALIGNMENT_MISSING_TOP_REQUIREMENT,
};

/// Share of a requirement's own keywords a document must carry before the
/// requirement counts as evidenced. Half is deliberately forgiving: a
/// requirement is a sentence, and a résumé bullet answering it will echo some
/// of its words, never all of them.
pub const TOP_REQUIREMENT_MATCH_RATIO: f64 = 0.5;

/// How well one side of the report answers a single requirement, 0.0–1.0.
///
/// `None` when the requirement has no extractable keywords ("Team player!") —
/// unanswerable, so it must not count for or against anything.
fn requirement_ratio_in(ctx: &Analysis, requirement: &str, in_source: bool) -> Option<f64> {
    let needed = ctx.tokens(requirement);
    if needed.is_empty() {
        return None;
    }
    let haystack = if in_source {
        &ctx.source_keywords
    } else {
        &ctx.generated_keywords
    };
    Some(needed.iter().filter(|t| haystack.contains(*t)).count() as f64 / needed.len() as f64)
}

/// Returns the alignment issues and the `topRequirementHits` metric.
pub(super) fn validate(ctx: &Analysis) -> (Vec<ContentIssue>, u32) {
    // Nothing extractable on the posting side, or the output is in the wrong
    // language: every comparison below would be noise.
    if !ctx.posting_comparable() {
        return (Vec::new(), 0);
    }

    let mut issues = Vec::new();

    // `alignment.low_coverage` — tailoring made the document match the posting
    // LESS well than the candidate's untouched résumé already did.
    if let (Some(generated), Some(source)) = (
        ctx.coverage(&ctx.generated_keywords),
        ctx.coverage(&ctx.source_keywords),
    ) {
        if generated < source {
            issues.push(issue(
                ALIGNMENT_LOW_COVERAGE,
                None,
                format!(
                    "The generated document covers {generated:.0}% of this posting's keywords \
                     where your source résumé already covered {source:.0}%. Something relevant \
                     was dropped — compare the two before sending."
                ),
                Some(format!("{generated:.0}% vs {source:.0}%")),
            ));
        }
    }

    // `alignment.missing_top_requirement` — a requirement the SOURCE evidences
    // that the generated document no longer does. Requirements the candidate
    // simply does not meet are not reported: that is a gap in their experience,
    // not a defect in the document, and the match score already says so.
    let mut hits = 0u32;
    for requirement in ctx.input.top_requirements {
        let Some(generated) = requirement_ratio_in(ctx, requirement, false) else {
            continue;
        };
        if generated >= TOP_REQUIREMENT_MATCH_RATIO {
            hits += 1;
            continue;
        }
        let evidenced_in_source = requirement_ratio_in(ctx, requirement, true)
            .is_some_and(|r| r >= TOP_REQUIREMENT_MATCH_RATIO);
        if evidenced_in_source {
            issues.push(issue(
                ALIGNMENT_MISSING_TOP_REQUIREMENT,
                None,
                format!(
                    "Your source résumé shows evidence for \"{requirement}\", but the generated \
                     document does not. Bring that evidence back — it is one of this posting's \
                     top requirements."
                ),
                Some(requirement.clone()),
            ));
        }
    }

    (issues, hits)
}
