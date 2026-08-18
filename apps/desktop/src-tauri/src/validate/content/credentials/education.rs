//! A2c — education. The weakest member of the family, kept weak on purpose:
//! see [`unsupported_institutions`] for the measurement that scoped it down
//! from the value comparison it was specified as.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::SectionKind;
use crate::export::types::LineKind;

use super::super::{contains_phrase, Section};
use super::contains_upper_acronym;

// ── Institutions, and when their absence is evidence ───────────────────────

/// Words that name an institution, in the languages this pipeline writes.
/// Matched case-insensitively, word-bounded.
static INSTITUTION_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(universit(?:y|ät|aet|ies|é|e|ad|à|eit|ade)|hochschule|fachhochschule|college|institute|institut|school|akademie|academy|polytechnic|politecnico|[ée]cole|escuela)\b",
    )
    .unwrap()
});

/// Institution ABBREVIATIONS, matched case-SENSITIVELY as uppercase tokens.
///
/// `mit` is the German preposition "with" and `tu` is a pronoun in three of the
/// languages here — matched case-insensitively, each turns an ordinary degree
/// line into an institution. Uppercase-only is what makes them usable at all,
/// and `MIT` is left out entirely: an ALL-CAPS German line would collide.
const INSTITUTION_ABBREVIATIONS: &[&str] = &[
    "TU", "FH", "ETH", "EPFL", "KTH", "RWTH", "LMU", "HTW", "HSG",
];

/// Degree names, matched word-bounded and case-insensitively.
///
/// The institution guard cannot be decided by [`INSTITUTION_MARKER_RE`] alone,
/// because that asks whether this module's LEXICON knows the school rather than
/// whether the document has an education history. `IIT Delhi`, `Caltech`,
/// `INSEAD`, `Sciences Po`, `Bocconi`, `KIT` and `Technion` all miss it, and a
/// source whose education sits under an unrecognised heading then looked like a
/// source with no education at all — the Warning fired on a truthful résumé.
///
/// A degree is the other half of an education entry and is spelled far more
/// uniformly than the institutions are.
const DEGREE_TOKENS: &[&str] = &[
    "bsc",
    "b.sc",
    "msc",
    "m.sc",
    "b.tech",
    "btech",
    "m.tech",
    "mtech",
    "b.eng",
    "m.eng",
    "beng",
    "meng",
    "mba",
    "phd",
    "ph.d",
    "bachelor",
    "bachelors",
    "bachelier",
    "baccalauréat",
    "master",
    "masters",
    "maîtrise",
    "magister",
    "diplom",
    "diploma",
    "diplôme",
    "doctorate",
    "doctorat",
    "doktor",
    "licence",
    "licenciatura",
    "laurea",
    "staatsexamen",
    "abitur",
    "vordiplom",
    "ingénieur",
    "ingenieur",
];

/// True when `text` names a degree anywhere.
///
/// `"master"`/`"masters"` are ambiguous in a way none of the other
/// [`DEGREE_TOKENS`] are: `Certified Scrum Master` names a CERTIFICATION, not
/// a degree, and `contains_phrase` is word-bounded, so it reads "Master" out
/// of that phrase just as readily as out of "Master of Science". Reading it
/// as a degree there lets a source with NO education section at all satisfy
/// `source_has_education`, silencing `unsourced_institution` for a document
/// that invented one whole. Scoped to the exact phrase rather than dropped
/// outright — a bare "Master, IIT Delhi" line is the shape this layer exists
/// to catch, and the family's own posture is to skip an ambiguous comparison
/// rather than guess at it either way.
fn names_a_degree(text: &str) -> bool {
    let lower = text.to_lowercase();
    DEGREE_TOKENS.iter().any(|d| {
        if matches!(*d, "master" | "masters") {
            contains_phrase(&lower, d) && !contains_phrase(&lower, &format!("scrum {d}"))
        } else {
            contains_phrase(&lower, d)
        }
    })
}

/// True when `text` names an institution anywhere.
pub fn names_an_institution(text: &str) -> bool {
    INSTITUTION_MARKER_RE.is_match(text)
        || INSTITUTION_ABBREVIATIONS
            .iter()
            .any(|abbr| contains_upper_acronym(text, abbr))
}

/// The institution segments an education section names.
///
/// An education line is `Degree, Institution, Dates` far more often than not,
/// so the date column is dropped and the remaining segments are filtered to the
/// ones carrying an institution marker. A line whose institution cannot be
/// isolated yields nothing — the degree half is the part that translates, and
/// quoting it would be exactly the accusation this check is scoped to avoid.
pub fn institutions(sections: &[Section]) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for section in sections.iter().filter(|s| s.kind == SectionKind::Education) {
        for line in &section.lines {
            if matches!(line.kind, LineKind::Blank) || line.text.trim().is_empty() {
                continue;
            }
            let body = crate::documents::evidence::trailing_date_column(&line.text)
                .map_or(line.text.as_str(), |(label, _)| label);
            // Parentheses split too: a date column is written `(2012 - 2016)`
            // as often as `, 2012 - 2016`, and only the comma form reaches
            // `trailing_date_column`.
            for segment in body.split(['|', '·', ',', '(', ')']) {
                let segment = segment.trim();
                if !segment.is_empty() && names_an_institution(segment) {
                    out.push((segment.to_string(), section.heading.clone()));
                }
            }
        }
    }
    out
}

/// Institutions the source cannot support AT ALL — the translation-safe residue
/// of A2c.
///
/// Fires only when the generated document has an education section naming an
/// institution while the source has NEITHER an education section NOR any
/// institution word anywhere in it. That is an education history invented
/// whole, and it is decidable without comparing a single translatable string:
/// both sides are asked the same language-independent question, "does this
/// document name a place of study at all".
///
/// The guard is threefold, and each layer covers a hole the one before it left.
/// `classify_section` does not recognise every heading a real CV writes, so a
/// source whose education sits under `QUALIFICATIONS` looks sectionless; the
/// institution-marker scan over the whole source text covers that, until the
/// school is `IIT Delhi` or `Caltech` and carries no marker word at all; so a
/// DEGREE token counts too ([`names_a_degree`]). Measured: without the third
/// layer this Warning fired on a truthful `B.Tech Computer Science, IIT Delhi`.
///
/// ## What was measured, and why the VALUE comparison is not here
///
/// A2c was specified as "compare the institution NAME against the source", on
/// the reasoning that institutions are proper nouns and proper nouns survive
/// translation. They do; their CITIES do not.
/// `institution_value_comparison_fires_on_a_correctly_translated_institution`
/// in `test.rs` runs that version and pins its result: a correct German
/// rendering of an English source's "Technical University of Munich" —
/// "Technische Universität München" — shares no token with the source and reads
/// as an invention. One false finding out of thirteen truthful documents, on
/// exactly the cross-language path #1004 shipped. Degree titles were the
/// predicted hazard; city names are the same hazard one level down, and
/// "compare institutions, not degrees" does not escape it.
///
/// So the value comparison ships in neither tier — not as a Critical, and not
/// as the Warning it was scoped to be, because "when uncertain, warn" is about
/// findings that are uncertain, not findings that are known-wrong on a document
/// class we generate every day.
pub fn unsupported_institutions(
    generated_sections: &[Section],
    source: &str,
    source_sections: &[Section],
) -> Vec<(String, Option<String>)> {
    let source_has_education = source_sections
        .iter()
        .any(|s| s.kind == SectionKind::Education)
        || names_an_institution(source)
        || names_a_degree(source);
    if source_has_education {
        return Vec::new();
    }
    // One invented school is one finding, the same rule the tenure and
    // certification arms already apply — without it, the same institution
    // named on two Education lines (or reachable through two of the
    // `['|', '·', ',', '(', ')']` delimiters on one line) reports twice with
    // identical evidence.
    let mut seen = HashSet::new();
    institutions(generated_sections)
        .into_iter()
        .filter(|(name, _)| seen.insert(name.to_lowercase()))
        .collect()
}
