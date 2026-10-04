//! Track A2: the credential extractors, CALIBRATED before they earn a severity.

use std::collections::BTreeSet;

use super::credential_corpus::*;
use super::*;

/// The VALUE comparison A2c was specified as, run here rather than shipped:
/// does any 4+ character non-numeric token of the institution appear in the
/// source at all?
///
/// It lives in the test file because it is a REJECTED design, and the
/// measurement that rejected it has to keep running — a rejection recorded only
/// in a comment stops being true the moment someone re-derives the idea. See
/// `institution_value_comparison_fires_on_a_correctly_translated_institution`.
pub(super) fn institution_absent_by_value(institution: &str, source: &str) -> bool {
    let source_lower = source.to_lowercase();
    let tokens: Vec<String> = institution
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|t| t.chars().count() >= 4)
        .filter(|t| !t.chars().all(|c| c.is_ascii_digit()))
        .filter(|t| !credentials::names_an_institution(t))
        .collect();
    !tokens.is_empty() && !tokens.iter().any(|t| source_lower.contains(t.as_str()))
}

/// One truthful document's outcome: what the checks FLAGGED, and — the half a
/// precision-only harness leaves out — what they even EXTRACTED.
struct Measured {
    /// `(years, certs, education-by-value, education-by-absence)` findings.
    flagged: (usize, usize, usize, usize),
    /// `(tenure, certification, institution)` — did the extractor see anything
    /// at all in this document?
    extracted: (bool, bool, bool),
}

/// Truthful fixtures whose TENURE the claims side must read.
///
/// Every document here states the candidate's own years of experience in a
/// phrasing a real résumé uses, and an inflated version of it must be catchable.
/// Six of them stopped being read when the summary admission rule was removed
/// wholesale — `en_generated_clean.txt` ("Eight years of backend work, most of
/// it on payment systems") and `tests/corpus/synthetic_swe.txt` ("Backend
/// engineer with 8 years building distributed services") among them — and the
/// flagged-column assertion stayed green throughout.
const EXTRACTS_A_TENURE: &[&str] = &[
    "es_actualidad",
    "fr_quinze_ans",
    "two_block_history",
    "en_generated_clean",
    "en_generated_paraphrased",
    "de_generated_clean",
    "de_generated_paraphrased",
    "en_generated_projects_tier2",
    "en_generated_projects_tier3",
    "corpus_synthetic_swe",
    "de_generated_from_en_source",
    "decade_tenure",
    "numeric_month_opener",
    "per_year_impact",
];

/// Truthful fixtures that name a certification. `cert_prose_verb` is
/// deliberately ABSENT: every "certified" in it is a verb, and reading one as a
/// credential is the false Critical this list guards against.
const EXTRACTS_A_CERTIFICATION: &[&str] = &[
    "en_cert_reworded",
    "de_cert_from_en_source",
    "cert_acronym_from_expansion",
    "cert_expansion_from_acronym",
];

/// Truthful fixtures that name a place of study.
const EXTRACTS_AN_INSTITUTION: &[&str] = &[
    "degree_without_marker",
    "en_generated_clean",
    "en_generated_paraphrased",
    "de_generated_clean",
    "de_generated_paraphrased",
    "en_generated_projects_tier2",
    "en_generated_projects_tier3",
    "corpus_synthetic_swe",
    "corpus_synthetic_designer",
    "de_generated_from_en_source",
    "de_translated_institution",
];

/// **The measurement that chose the severities, kept as a guard.**
///
/// This repo has already shipped one false Critical on a truthful résumé, and a
/// false Critical here is not a cosmetic miss: it blanks `keywordCoverage` and
/// suppresses every alignment finding on a document that is fine. So each
/// extractor was run over every truthful document the repo owns BEFORE it was
/// wired into `validate_content`, and the tiers were picked from this table.
///
/// It still runs, and now asserts, because a printed table nobody checks is a
/// comment: the zero false positives that made `factual.inflated_experience`
/// and `factual.unsourced_certification` Critical have to keep being zero.
///
/// Run with `cargo test --all-features credential_extractor_calibration -- --nocapture`
/// to read the table.
/// A FROZEN reference year for the calibration corpus, not `reference_year`'s
/// own clock read.
///
/// Every open-ended fixture here (`es_actualidad`, `two_block_history`,
/// `numeric_month_opener`, `de_generated_from_en_source`) is sized against
/// "today" — and an open span's allowance only ever WIDENS as the calendar
/// advances (`career_span_years`'s own doc calls this "monotone-loosening"),
/// so the zero-false-positive assertion below stays safe forever going
/// forward on the live clock too. What a live clock costs is reproducibility:
/// the table cannot be re-run at an earlier date to audit a past run, and the
/// recall arms carry no explicit anchor. Pinned at the value the live clock
/// already resolves to, so freezing it changes nothing this test currently
/// asserts — the boundary tests below already use this same pattern
/// (`career_span_years(&source, Some(2026))`).
const CALIBRATION_REFERENCE_YEAR: u32 = 2026;

#[test]
fn credential_extractor_calibration() {
    let probe = |label: &str, generated: &str, source: &str| {
        let generated_sections = split_sections(generated, DocKind::Resume);
        let source_sections = split_sections(source, DocKind::Resume);
        let reference = Some(CALIBRATION_REFERENCE_YEAR);
        let supported = credentials::supported_years(source, reference);
        let years = credentials::inflated_years_claims(&generated_sections, source, reference);
        let certs = credentials::unsupported_certs(&generated_sections, source);
        let by_value: Vec<(String, Option<String>)> =
            credentials::institutions(&generated_sections)
                .into_iter()
                .filter(|(name, _)| institution_absent_by_value(name, source))
                .collect();
        let by_absence =
            credentials::unsupported_institutions(&generated_sections, source, &source_sections);
        let seen_years: Vec<u32> = credentials::years_claims(&generated_sections)
            .iter()
            .map(|c| c.years)
            .collect();
        let seen_certs: Vec<String> = credentials::cert_claims(&generated_sections)
            .iter()
            .map(|c| c.keys.join("/"))
            .collect();
        let seen_edu: Vec<String> = credentials::institutions(&generated_sections)
            .iter()
            .map(|(n, _)| n.clone())
            .collect();
        // The EXTRACTED spans are printed next to the FLAGGED ones on purpose.
        // "zero false positives" is worthless if the extractor saw nothing at
        // all: `en_generated_clean` has to show its "Eight years" claim being
        // read and then cleared, or the zero is a tautology.
        println!(
            "{label:<28} supported={supported:?} seen: yrs={:?} cert={:?} edu={:?}\n\
             {:28} FLAGGED: years={:?} certs={:?} edu_by_value={:?} edu_by_absence={:?}",
            seen_years,
            seen_certs,
            seen_edu,
            "",
            years
                .iter()
                .map(|(c, _)| format!("{}({})", c.raw, c.years))
                .collect::<Vec<_>>(),
            certs.iter().map(|c| c.raw.as_str()).collect::<Vec<_>>(),
            by_value.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            by_absence
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>(),
        );
        Measured {
            flagged: (years.len(), certs.len(), by_value.len(), by_absence.len()),
            extracted: (
                !seen_years.is_empty(),
                !seen_certs.is_empty(),
                !seen_edu.is_empty(),
            ),
        }
    };

    println!("\n── truthful documents (every finding here is a FALSE POSITIVE) ──");
    let truthful = truthful_documents();
    let mut fp = (0, 0, 0, 0);
    let mut extracts_tenure: BTreeSet<&str> = BTreeSet::new();
    let mut extracts_certification: BTreeSet<&str> = BTreeSet::new();
    let mut extracts_institution: BTreeSet<&str> = BTreeSet::new();
    let mut by_value_false_positives: BTreeSet<&str> = BTreeSet::new();
    for (label, generated, source) in &truthful {
        let m = probe(label, generated, source);
        fp = (
            fp.0 + m.flagged.0,
            fp.1 + m.flagged.1,
            fp.2 + m.flagged.2,
            fp.3 + m.flagged.3,
        );
        if m.extracted.0 {
            extracts_tenure.insert(label);
        }
        if m.extracted.1 {
            extracts_certification.insert(label);
        }
        if m.extracted.2 {
            extracts_institution.insert(label);
        }
        if m.flagged.2 > 0 {
            by_value_false_positives.insert(label);
        }
    }

    println!("\n── documents carrying a known invention (recall) ──");
    for (label, generated, source) in invented_documents() {
        let m = probe(label, generated, source);
        // Recall is asserted next to the false-positive budget on purpose: a
        // check that fires on nothing scores a perfect zero above.
        let found = match label {
            "inflated_years" | "inflated_years_beside_per_year_impact" => m.flagged.0,
            "invented_certification" => m.flagged.1,
            "invented_education" => m.flagged.3,
            other => panic!("unlabelled invention fixture: {other}"),
        };
        assert!(found > 0, "{label}: the planted invention was not reported");
    }

    println!(
        "\nFALSE POSITIVES over {} truthful documents: years={} certs={} \
         edu_by_value={} edu_by_absence={}\n",
        truthful.len(),
        fp.0,
        fp.1,
        fp.2,
        fp.3
    );

    // ── The RECALL half of the measurement, asserted rather than printed ────
    //
    // Precision-only calibration cannot tell a fix from an OFF SWITCH: both
    // score zero. Two over-corrections shipped past a green flagged-column
    // assertion — a source-side guard that any bullet saying "$1.2M per year"
    // disabled for the whole document, and a claims-side gate that stopped
    // reading six of these fixtures' own tenure sentences. Both are invisible
    // above and loud here.
    //
    // Pinned as a hand-written membership list, not derived from the corpus:
    // a set compared against itself cannot notice a deletion, which is exactly
    // the failure being guarded. `assert_eq` on the whole set, so a fixture
    // silently GAINING an extraction is caught too.
    assert_eq!(
        extracts_tenure,
        EXTRACTS_A_TENURE.iter().copied().collect::<BTreeSet<_>>(),
        "the tenure extractor must read every truthful document that states one — and \
         must keep reading NONE of the others"
    );
    assert_eq!(
        extracts_certification,
        EXTRACTS_A_CERTIFICATION
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        "the certification extractor's reach"
    );
    assert_eq!(
        extracts_institution,
        EXTRACTS_AN_INSTITUTION
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        "the institution extractor's reach"
    );

    // The two SHIPPED Criticals, and the shipped Warning: absolute zero, which
    // is the measurement their tier was chosen on.
    assert_eq!(
        (fp.0, fp.1, fp.3),
        (0, 0, 0),
        "a credential check fired on a truthful document — a Critical here blanks the \
         quality panel on a résumé that is fine; re-open the tier decision before \
         loosening this"
    );
    // …and the rejected one, pinned to the SPECIFIC fixture that rejected it —
    // not a corpus-wide count. A count trips on any unrelated truthful fixture
    // whose institution also happens to translate, and points a reader at this
    // design decision instead of at their new fixture; a hand-written
    // membership list (the same pattern the three `extracts_*` sets above use)
    // keeps its meaning as the corpus grows.
    assert_eq!(
        by_value_false_positives,
        ["de_translated_institution"]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "the by-VALUE institution comparison is recorded as producing exactly one false \
         positive on this corpus (the translated institution). If that changed, the \
         reason A2c ships as an absence check needs restating, not silently updating"
    );
}
