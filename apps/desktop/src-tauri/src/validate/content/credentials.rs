//! Credential grounding — the invention classes the metric family cannot see.
//!
//! `factual::unsourced_metric` polices exactly three number shapes (a
//! percentage, a multiplier, an integer of three digits or more). A bare one-
//! or two-digit number is deliberately not policed, because "3 engineers" is
//! far too common to accuse anyone over — which leaves `8+ years of experience`
//! on a source résumé showing four years invisible to every check we ship. A
//! certification and an education entry are invisible for a simpler reason:
//! neither carries a figure at all.
//!
//! Three checks live here. They are NOT equally strong, and the difference is
//! the whole design:
//!
//! * **Years of experience** compares a NUMBER against the source's own stated
//!   years and against the span its dates reach back over. A number survives
//!   translation and paraphrase intact — but only if the evidence that SPARES a
//!   claim is read just as well, which is why the number words cover every
//!   language this pipeline writes and why the span is computed from raw text
//!   rather than from a section classifier.
//! * **Certifications** are proper nouns and acronyms that survive translation
//!   verbatim (`AWS Certified Solutions Architect`, `PMP`, `CISSP`). The
//!   trigger set is CURATED — an explicit issuer list plus an explicit acronym
//!   list — never "looks capitalised", which would fire on every ordinary
//!   proper noun in the document.
//! * **Education** is the weak one, and it is kept weak on purpose. Degree
//!   titles TRANSLATE (`Diplom-Informatiker` ↔ `MSc Computer Science`), so a
//!   value comparison on the degree string fires on correct cross-language
//!   output. Only the institution is looked at, and only in the one shape that
//!   is translation-safe (see [`unsupported_institutions`]).
//!
//! Same posture as `factual`: deterministic, model-free, and a comparison that
//! cannot be made reliably is skipped rather than guessed at. Absence of a term
//! in the source is evidence ONLY when the term is the kind that survives
//! translation.

mod certifications;
mod education;
mod tenure;

use super::{
    issue, Analysis, ContentIssue, DocKind, FACTUAL_INFLATED_EXPERIENCE,
    FACTUAL_UNSOURCED_CERTIFICATION, FACTUAL_UNSOURCED_INSTITUTION,
};

// Re-exported flat, so the three families stay ONE vocabulary to their callers
// and to `test.rs`: the split into `credentials/` is about the LOC cap in
// `docs/architecture-rules.md` R8, not about the API.
pub(super) use self::certifications::unsupported_certs;
pub(super) use self::education::unsupported_institutions;
pub(super) use self::tenure::{inflated_years_claims, reference_year};

// The rest of the surface is reached only by `test.rs`, where the extractors
// are measured DIRECTLY (`credential_extractor_calibration`) rather than
// through [`validate`] — a report cannot say whether a check went quiet or
// never looked, and telling those two apart is the whole point of that harness.
// Gated the way `language`'s test-only surface already is, so the lib build
// carries no unused re-export.
#[cfg(test)]
pub(super) use self::certifications::{
    cert_claims, CERT_EVIDENCE_LINE_CHARS, CERT_ISSUER_WINDOW_CHARS, CERT_ROLE_NOUN_WINDOW_CHARS,
};
#[cfg(test)]
pub(super) use self::education::{institutions, names_an_institution};
#[cfg(test)]
pub(super) use self::tenure::{
    career_span_years, stated_years, supported_years, years_claims, CAREER_SPAN_SLACK_YEARS,
    CLAIM_CONTEXT_CHARS, MAX_PLAUSIBLE_TENURE_YEARS, SPAN_TAIL_CHARS,
};

/// True when `acronym` appears in `line` as an UPPERCASE, word-bounded token.
///
/// Case-sensitive on the claims side on purpose: the lowercase readings of
/// these tokens are ordinary words and abbreviations in running prose, and an
/// acronym written in caps is the shape a résumé actually names a certification
/// in.
pub(super) fn contains_upper_acronym(line: &str, acronym: &str) -> bool {
    line.match_indices(acronym).any(|(i, m)| {
        let before = line[..i].chars().next_back();
        let after = line[i + m.len()..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        boundary(before) && boundary(after)
    })
}

// ── The dispatcher ──────────────────────────────────────────────────────────

/// Every credential check, in a stable order.
///
/// Run for a LETTER as well as a résumé, minus the education arm: "I bring 12
/// years of experience" and "as an AWS Certified Solutions Architect" are
/// letter sentences at least as often as résumé lines, and the truth base for
/// both is the source RÉSUMÉ alone. Not the job ad — unlike a metric, which a
/// letter may legitimately quote back from the posting, a posting's "5+ years
/// required" is a statement about the ROLE, and letting it vouch for the
/// candidate would make every ad its own alibi. The education arm is skipped
/// because a letter has no education section to read; `institutions` would find
/// nothing anyway, and saying so here is cheaper than making a reader prove it.
pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let source = ctx.input.source_resume;
    let reference = reference_year(source, ctx.input.generated);
    let mut issues: Vec<ContentIssue> =
        inflated_years_claims(&ctx.generated_sections, source, reference)
            .into_iter()
            .map(|(claim, supported)| {
                issue(
                    FACTUAL_INFLATED_EXPERIENCE,
                    claim.section.as_deref(),
                    format!(
                        "\"{}\" claims more experience than your source résumé supports: what it \
                 states, and how far its own dates reach back, come to at most {supported} \
                 years. Correct it to a figure your own document backs.",
                        claim.raw
                    ),
                    Some(claim.raw),
                )
            })
            .collect();

    issues.extend(
        unsupported_certs(&ctx.generated_sections, source)
            .into_iter()
            .map(|claim| {
                issue(
                    FACTUAL_UNSOURCED_CERTIFICATION,
                    claim.section.as_deref(),
                    format!(
                        "\"{}\" is not in your source résumé. A certification is checkable by \
                         the employer — remove it, or add it to your own résumé first.",
                        claim.raw
                    ),
                    Some(claim.raw),
                )
            }),
    );

    if ctx.input.doc_kind == DocKind::Resume {
        issues.extend(
            unsupported_institutions(&ctx.generated_sections, source, &ctx.source_sections)
                .into_iter()
                .map(|(name, section)| {
                    issue(
                        FACTUAL_UNSOURCED_INSTITUTION,
                        section.as_deref(),
                        format!(
                            "\"{name}\" appears here, but your source résumé names no place of \
                             study at all. Add it to your own résumé, or remove the section."
                        ),
                        Some(name),
                    )
                }),
        );
    }
    issues
}
