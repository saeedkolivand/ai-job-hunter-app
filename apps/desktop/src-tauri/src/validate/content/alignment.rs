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

/// How many PERCENTAGE POINTS of coverage a generated document may lose against
/// the source before it is worth reporting.
///
/// Coverage is a share of a posting's keyword set, so on a typical 40-keyword ad
/// one dropped word moves it 2.5 points. Firing on any drop at all reported
/// every legitimate edit — a bullet reworded, a redundant line cut — as
/// "something relevant was dropped". A drop only means something once it is
/// bigger than the granularity of the measure.
pub const MIN_COVERAGE_DROP_POINTS: f64 = 5.0;

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
///
/// The metric is `None` for **unmeasured**, never `0`. Two cases reach it:
/// a posting nothing can be compared against ([`Analysis::posting_comparable`]),
/// and a run given no `top_requirements` at all — the JD-analysis step is
/// optional, so a report routinely arrives with an empty list. A bare "0" in the
/// quality panel reads as "your document answers none of this posting's top
/// requirements", which is a measurement nobody took; the renderer prints "—"
/// for the absent value instead.
pub(super) fn validate(ctx: &Analysis) -> (Vec<ContentIssue>, Option<u32>) {
    // Nothing extractable on the posting side, or the output is in the wrong
    // language: every comparison below would be noise.
    if !ctx.posting_comparable() {
        return (Vec::new(), None);
    }

    let mut issues = Vec::new();

    // `alignment.low_coverage` — tailoring made the document match the posting
    // MEANINGFULLY less well than the candidate's untouched résumé already did.
    if let (Some(generated), Some(source)) = (
        ctx.coverage(&ctx.generated_keywords),
        ctx.coverage(&ctx.source_keywords),
    ) {
        if source - generated >= MIN_COVERAGE_DROP_POINTS {
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

    // An empty requirements list is not "zero hits" — the JD-analysis step is
    // optional and routinely absent, and the loop above never ran. Note this is
    // deliberately NOT an early return: `alignment.low_coverage` compares the
    // two documents' coverage of the posting and has nothing to do with the
    // requirements list.
    let measured = (!ctx.input.top_requirements.is_empty()).then_some(hits);
    (issues, measured)
}
