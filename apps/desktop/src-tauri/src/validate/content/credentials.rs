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

use std::collections::HashSet;
use std::sync::LazyLock;

use chrono::Datelike;
use regex::Regex;

use crate::documents::evidence::{is_open_ended, years_in, SectionKind};
use crate::export::types::LineKind;

use super::{
    contains_phrase, issue, Analysis, ContentIssue, DocKind, Section, FACTUAL_INFLATED_EXPERIENCE,
    FACTUAL_UNSOURCED_CERTIFICATION, FACTUAL_UNSOURCED_INSTITUTION,
};

// ── A2a: years of experience ────────────────────────────────────────────────

/// Slack, in years, added to a career span computed from YEAR NUMBERS ONLY.
///
/// A résumé's date column carries years, not months: `2018 - 2021` is anything
/// from 24 months (Dec 2018 → Jan 2021) to 47 (Jan 2018 → Dec 2021). The
/// difference of the two year numbers is therefore a LOWER bound on the true
/// span, and one year is the exact amount by which it can understate. Rounding
/// the other way — accusing a candidate whose eight years are really 7.6 —
/// is the failure this family cannot afford.
pub(super) const CAREER_SPAN_SLACK_YEARS: u32 = 1;

/// How far either side of a `<number> <year-word>` span an experience-context
/// word may sit and still make the span a TENURE CLAIM.
///
/// Wide enough for the shapes real documents write ("8+ years of experience",
/// "acht Jahre Erfahrung", "Erfahrung: acht Jahre", "ten years of professional
/// experience"), narrow enough that an unrelated sentence later in the same
/// bullet cannot supply the context.
pub(super) const CLAIM_CONTEXT_CHARS: usize = 40;

/// The largest tenure this reads as a claim about a person.
///
/// Above it the number is about something else — a system, a company, a
/// dataset — and a check that accuses someone of overstating a 150-year tenure
/// is reporting a parse failure as a fabrication.
pub(super) const MAX_PLAUSIBLE_TENURE_YEARS: u32 = 60;

/// Number words a tenure is spelled with, in EVERY language `make_stemmer`
/// supports — not just the two whose lexicons this module curates.
///
/// Digits are handled by the regex; this table exists because a truthful résumé
/// spells its tenure out ("eight years", "acht Jahre", "quinze ans"), and the
/// side that reads the SOURCE is the side that SPARES a claim. An en/de-only
/// table therefore did not merely miss French — it read `quinze années
/// d'expérience` as a source that states nothing, and turned a faithful
/// `15 années` output into a Critical. That is the whole "translation-safe by
/// construction" argument failing at the one place it mattered: the comparison
/// is on a number, but the EXTRACTION of the sparing evidence was not.
///
/// Duplicates across languages ("six" en/fr, "vier" de/nl, "acht" de/nl) are
/// harmless and deliberate — every spelling maps to the same value, and the
/// regex alternation is deduplicated where it is built.
const SPELLED_NUMBERS: &[(&str, u32)] = &[
    // English
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("twenty", 20),
    ("ein", 1),
    ("eins", 1),
    ("zwei", 2),
    ("drei", 3),
    ("vier", 4),
    ("fünf", 5),
    ("sechs", 6),
    ("sieben", 7),
    ("acht", 8),
    ("neun", 9),
    ("zehn", 10),
    ("elf", 11),
    ("zwölf", 12),
    ("dreizehn", 13),
    ("vierzehn", 14),
    ("fünfzehn", 15),
    ("sechzehn", 16),
    ("siebzehn", 17),
    ("achtzehn", 18),
    ("neunzehn", 19),
    ("zwanzig", 20),
    // French
    ("un", 1),
    ("une", 1),
    ("deux", 2),
    ("trois", 3),
    ("quatre", 4),
    ("cinq", 5),
    ("sept", 7),
    ("huit", 8),
    ("neuf", 9),
    ("dix", 10),
    ("onze", 11),
    ("douze", 12),
    ("treize", 13),
    ("quatorze", 14),
    ("quinze", 15),
    ("seize", 16),
    ("dix-sept", 17),
    ("dix-huit", 18),
    ("dix-neuf", 19),
    ("vingt", 20),
    // Spanish
    ("uno", 1),
    ("una", 1),
    ("dos", 2),
    ("tres", 3),
    ("cuatro", 4),
    ("cinco", 5),
    ("seis", 6),
    ("siete", 7),
    ("ocho", 8),
    ("nueve", 9),
    ("diez", 10),
    ("once", 11),
    ("doce", 12),
    ("trece", 13),
    ("catorce", 14),
    ("quince", 15),
    ("dieciséis", 16),
    ("dieciseis", 16),
    ("diecisiete", 17),
    ("dieciocho", 18),
    ("diecinueve", 19),
    ("veinte", 20),
    // Italian
    ("due", 2),
    ("tre", 3),
    ("quattro", 4),
    ("cinque", 5),
    ("sei", 6),
    ("sette", 7),
    ("otto", 8),
    ("nove", 9),
    ("dieci", 10),
    ("undici", 11),
    ("dodici", 12),
    ("tredici", 13),
    ("quattordici", 14),
    ("quindici", 15),
    ("sedici", 16),
    ("diciassette", 17),
    ("diciotto", 18),
    ("diciannove", 19),
    ("venti", 20),
    // Dutch
    ("een", 1),
    ("één", 1),
    ("twee", 2),
    ("drie", 3),
    ("vijf", 5),
    ("zes", 6),
    ("zeven", 7),
    ("negen", 9),
    ("tien", 10),
    ("twaalf", 12),
    ("dertien", 13),
    ("veertien", 14),
    ("vijftien", 15),
    ("zestien", 16),
    ("zeventien", 17),
    ("achttien", 18),
    ("negentien", 19),
    ("twintig", 20),
    // Portuguese
    ("um", 1),
    ("uma", 1),
    ("dois", 2),
    ("duas", 2),
    ("três", 3),
    ("tres", 3),
    ("sete", 7),
    ("oito", 8),
    ("dez", 10),
    ("doze", 12),
    ("treze", 13),
    ("catorze", 14),
    ("quatorze", 14),
    ("dezesseis", 16),
    ("dezessete", 17),
    ("dezoito", 18),
    ("dezenove", 19),
    ("vinte", 20),
];

/// The word for "year" in the languages `make_stemmer` supports.
///
/// Bare French `an` is deliberately absent: it is one letter away from the
/// English article in every document this runs on, and a singular tenure is
/// not worth that.
const YEAR_WORDS: &str = "years|year|yrs|yr|jahren|jahre|jahr|années|année|annees|annee|ans|años|año|anos|ano|anni|anno|jaren|jaar";

/// `<number> <year-word>`, where the separator is whitespace or a `+`.
///
/// The separator rule is what keeps the hyphenated ADJECTIVE out: "a 20-year-old
/// legacy stack" is not a claim about anybody's tenure, and it is the single
/// most common two-digit `year` collocation in engineering prose. `\b` on the
/// number keeps the check out of a four-digit date ("2014 - 2018" offers no
/// position where one or two digits end on a word boundary).
static YEARS_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Longest spelled form first: `\b` alone does not decide an alternation,
    // and `vier` would otherwise win against `vierzehn`.
    let mut words: Vec<&str> = SPELLED_NUMBERS.iter().map(|(w, _)| *w).collect();
    words.sort_by_key(|w| std::cmp::Reverse(w.len()));
    words.dedup();
    Regex::new(&format!(
        r"(?i)\b(\d{{1,2}}|{})(?:\s*\+\s*|\s+)({})\b",
        words.join("|"),
        YEAR_WORDS
    ))
    .unwrap()
});

/// A year-word preceded by a WORD rather than a digit — the shape
/// [`states_an_unreadable_tenure`] inspects.
///
/// Deliberately looser than [`YEARS_RE`]: this one has to see the quantifiers
/// that table CANNOT read, which is the whole point of it.
static UNREADABLE_TENURE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)\b(\p{{L}}[\p{{L}}-]*)\s+({YEAR_WORDS})\b")).unwrap()
});

/// Words that make a nearby `<number> years` a claim about the CANDIDATE's own
/// tenure rather than about a system, a contract or a migration.
///
/// Curated per language rather than inferred: without it, "cut the 12-month
/// rollout to 3 years of runway" is a tenure claim, and the check starts
/// accusing people over project prose.
const EXPERIENCE_CONTEXT: &[&str] = &[
    "experience",
    "experienced",
    "career",
    "professional",
    "industry",
    "tenure",
    "working",
    "worked",
    "erfahrung",
    "berufserfahrung",
    "berufliche",
    "beruflicher",
    "laufbahn",
    "karriere",
    "tätig",
    "praxis",
    "expérience",
    "carrière",
    "professionnelle",
    "experiencia",
    "carrera",
    "profesional",
    "esperienza",
    "carriera",
    "professionale",
    "ervaring",
    "werkervaring",
    "loopbaan",
    "experiência",
    "carreira",
    "profissional",
];

/// One `<number> years` span a document states, with where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct YearsClaim {
    /// The number, whether the document wrote it in digits or in words. This is
    /// the only field a cross-language comparison may look at.
    pub(super) years: u32,
    /// The exact span the document wrote — the evidence a finding quotes.
    pub(super) raw: String,
    /// The heading of the section the span sits in, for a section-scoped
    /// finding.
    pub(super) section: Option<String>,
}

/// The number a `YEARS_RE` capture states, in digits or in words.
fn claimed_years(raw: &str) -> Option<u32> {
    if let Ok(n) = raw.parse::<u32>() {
        return Some(n);
    }
    let lower = raw.to_lowercase();
    SPELLED_NUMBERS
        .iter()
        .find(|(word, _)| *word == lower)
        .map(|(_, n)| *n)
}

/// `line[start..end]` widened by `chars` characters on each side, clamped to
/// the line and to character boundaries.
fn context_window(line: &str, start: usize, end: usize, chars: usize) -> &str {
    let left = line[..start]
        .char_indices()
        .rev()
        .nth(chars.saturating_sub(1))
        .map_or(0, |(i, _)| i);
    let right = line[end..]
        .char_indices()
        .nth(chars)
        .map_or(line.len(), |(i, _)| end + i);
    &line[left..right]
}

/// Every `<number> years` span on `line`, unfiltered — the SOURCE side's
/// reading, which needs no context word.
fn years_spans(line: &str) -> Vec<(u32, String, usize, usize)> {
    YEARS_RE
        .captures_iter(line)
        .filter_map(|c| {
            let whole = c.get(0)?;
            let years = claimed_years(c.get(1)?.as_str())?;
            (years <= MAX_PLAUSIBLE_TENURE_YEARS).then(|| {
                (
                    years,
                    whole.as_str().trim().to_string(),
                    whole.start(),
                    whole.end(),
                )
            })
        })
        .collect()
}

/// The largest tenure the SOURCE states anywhere, in any shape.
///
/// Deliberately context-free, the same leniency `factual::all_numbers` gives
/// the truth side of the metric comparison: a statement in the candidate's own
/// document may only ever SPARE a claim, so reading too much of it is safe
/// while reading too little manufactures accusations.
pub(super) fn stated_years(source: &str) -> Option<u32> {
    source
        .lines()
        .flat_map(years_spans)
        .map(|(years, _, _, _)| years)
        .max()
}

/// Every tenure CLAIM the generated document makes: a `<number> <year-word>`
/// span with an [`EXPERIENCE_CONTEXT`] word within [`CLAIM_CONTEXT_CHARS`] of
/// it.
///
/// **The context word is the ONLY admission rule**, and an earlier version that
/// also admitted anything in the SUMMARY — on the reasoning that a summary's
/// whole job is to state what the candidate has done — was wrong in the way
/// this family cannot afford. A summary is also where a candidate writes
/// "replaced a 30 year old mainframe", "retired 12 years of accumulated schema
/// drift", "cut a 40 year legacy batch": all three fired, all three are
/// achievements, and the user was told to delete them. A genuine tenure claim
/// carries `experience` / `Erfahrung` / `experiencia` anyway.
pub(super) fn years_claims(sections: &[Section]) -> Vec<YearsClaim> {
    let mut out = Vec::new();
    for section in sections {
        for line in &section.lines {
            for (years, raw, start, end) in years_spans(&line.text) {
                let window =
                    context_window(&line.text, start, end, CLAIM_CONTEXT_CHARS).to_lowercase();
                if EXPERIENCE_CONTEXT
                    .iter()
                    .any(|word| contains_phrase(&window, word))
                {
                    out.push(YearsClaim {
                        years,
                        raw,
                        section: section.heading.clone(),
                    });
                }
            }
        }
    }
    out
}

/// A year-word in the SOURCE preceded by a quantifier [`SPELLED_NUMBERS`]
/// cannot read — "several years", "over the years", or a number word in a
/// language this table does not cover.
///
/// Such a source states a tenure of UNKNOWN size, and unknown is not zero. The
/// whole check goes quiet rather than compare a claim against evidence it
/// failed to read: the sparing side going blind is what manufactures an
/// accusation, and a word list can always be missing a word.
fn states_an_unreadable_tenure(source: &str) -> bool {
    UNREADABLE_TENURE_RE.captures_iter(source).any(|c| {
        c.get(1).is_some_and(|word| {
            let lower = word.as_str().to_lowercase();
            !SPELLED_NUMBERS.iter().any(|(w, _)| *w == lower)
        })
    })
}

/// The year an open-ended span in the source is measured against — `None` when
/// there is no trustworthy answer.
///
/// This is the ONE non-hermetic input in the file, and `factual.rs`'s
/// neighbouring date check is proud of never reading the clock at all. It has
/// to be read here: "2021 – Present" is what almost every résumé's current role
/// says, and without a today there is no span to compare a tenure against, so a
/// clock-free version of this check would be inert on the majority of real
/// documents.
///
/// **Self-validating rather than trusted.** A clock reading EARLIER than a year
/// the documents themselves name is wrong — a dead CMOS battery, a fresh VM
/// before its first NTP sync — and using it would shrink the allowance and
/// manufacture the exact false Critical this family exists to avoid (a source
/// dated "2016 – Present" with a clock stuck at 1970 makes a truthful "8 years"
/// read as an eight-fold exaggeration). So the clock is checked against the
/// documents and DISCARDED when it fails, taking the span evidence with it: the
/// check then falls back on what the source states in words, or goes quiet.
///
/// **What this costs in determinism, stated plainly.** Two runs a day apart
/// across 31 December can disagree, so `validation_is_deterministic` is true
/// WITHIN a calendar year rather than absolutely. The disagreement is
/// monotone-loosening — a later reading only ever widens the allowance — so a
/// report that passed cannot later fail on the clock alone; only the reverse,
/// and only towards silence.
///
/// A clock that is AHEAD needs no guard — it only ever widens the allowance.
/// The cost of the rule is a document carrying a future year (a typo, a
/// start-date-in-advance entry), which reads as an untrustworthy clock and goes
/// quiet. A missed check, which is this family's chosen direction of error.
pub(super) fn reference_year(source: &str, generated: &str) -> Option<u32> {
    let documented = years_in(source)
        .into_iter()
        .chain(years_in(generated))
        .max()
        .unwrap_or(0);
    let clock = chrono::Utc::now().year().max(0) as u32;
    (clock >= documented).then_some(clock)
}

/// How much text after a span separator is read looking for the span's END.
///
/// Long enough for `Mar 2021` and `bis Dezember 2021`, short enough that the
/// next sentence cannot supply a year and make an open span look closed.
pub(super) const SPAN_TAIL_CHARS: usize = 16;

/// `<year> <span separator> <tail>` — the shape a date column has, read off raw
/// TEXT rather than off a parsed entry.
static SPAN_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?:19|20)\d{{2}}\s*(?:[-–—]|\bto\b|\bbis\b|\buntil\b|\bhasta\b|\bau\b|\bà\b|\ba\b|\btot\b|\bfino\b|\baté\b)\s*([^\n]{{0,{SPAN_TAIL_CHARS}}})"
    ))
    .unwrap()
});

/// True when the source shows a role that has NOT ended.
///
/// Asked of raw text, and asked STRUCTURALLY: a date column whose separator is
/// followed by something that is not another year is open, whatever word sits
/// there. That is what makes it work for `2015 - Actualidad`, `2019 -
/// Aujourd'hui` and `2020 - Today`, none of which `PRESENT_MARKERS` carries —
/// and for the next spelling nobody has thought of either. The marker list is
/// still consulted, because `seit 2021` is open with no separator at all.
///
/// Every direction of error here is generous: an unrelated "in 2019 - a big
/// year" reads as an open span and WIDENS the allowance. A false "closed" is
/// the only dangerous answer, and it needs every span in the document to have a
/// year within [`SPAN_TAIL_CHARS`] of its separator — which is what a genuinely
/// closed history looks like.
fn source_is_ongoing(source: &str) -> bool {
    source.lines().any(|line| {
        if years_in(line).is_empty() {
            return false;
        }
        is_open_ended(line)
            || SPAN_TAIL_RE.captures_iter(line).any(|c| {
                c.get(1)
                    .is_none_or(|tail| years_in(tail.as_str()).is_empty())
            })
    })
}

/// The span between the earliest year the source names and its latest end.
///
/// ## Read from the TEXT, never from the parse
///
/// The lenient half of this comparison ([`stated_years`]) reads raw text, so
/// the accusing half must too. Three separate false Criticals were measured
/// when it did not:
///
/// * a history whose second block is headed `EARLIER ROLES` (`career` is in
///   `classify_section`'s lexicon, `roles` is not) lost its 2010-2015 role, and
///   a truthful "15 years" became a Critical;
/// * a Spanish résumé whose current role ends in `Actualidad` — a spelling
///   `PRESENT_MARKERS` does not carry — had its span closed at the role's own
///   START year;
/// * the same shape again wherever `EXPERIENCIA` or any other heading fails to
///   classify, because then there are no entries to read an end from at all.
///
/// Every one of those is the same defect: an under-parse SHRANK the allowance,
/// and a shrunk allowance is an accusation. So neither end is parsed now. The
/// start is the earliest year anywhere in the source, and the end is today when
/// [`source_is_ongoing`] sees a role that has not finished, otherwise the latest
/// year the source names.
///
/// The price, stated rather than hidden: an education entry dated 2012-2016
/// widens the allowance for a career that began in 2019, because this no longer
/// measures "how long did you work" but "can your own document reach back that
/// far at all". That is the strongest claim that can be made without trusting a
/// section classifier in seven languages, and it still catches the class this
/// check was built for — a source whose whole history spans four years against
/// an output claiming eight.
///
/// `None` when the source names no year, or when a role is open and
/// [`reference_year`] found no trustworthy today to close it with.
pub(super) fn career_span_years(source: &str, reference: Option<u32>) -> Option<u32> {
    let years = years_in(source);
    let earliest = years.iter().copied().min()?;
    let latest = if source_is_ongoing(source) {
        reference?
    } else {
        years.iter().copied().max()?
    };
    Some(latest.saturating_sub(earliest))
}

/// The largest tenure the source supports: whatever it states, or the span its
/// dates cover plus [`CAREER_SPAN_SLACK_YEARS`], whichever is larger.
///
/// `None` when the source supports NO reading at all — it states no tenure and
/// carries no dated entry, or it states one this file cannot READ
/// ([`states_an_unreadable_tenure`]). The comparison is then unmakeable and the
/// check must stay silent rather than treat "unknown" as "zero".
pub(super) fn supported_years(source: &str, reference: Option<u32>) -> Option<u32> {
    if states_an_unreadable_tenure(source) {
        return None;
    }
    let stated = stated_years(source);
    let span = career_span_years(source, reference).map(|s| s + CAREER_SPAN_SLACK_YEARS);
    match (stated, span) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0).max(b.unwrap_or(0))).filter(|n| *n > 0),
    }
}

/// Tenure claims the source cannot support — INFLATION only.
///
/// A claim SMALLER than what the source supports is never reported. An
/// understatement is not a fabrication, and making the rule "the number must
/// appear in the source" rather than "the number must not exceed it" is what
/// would turn every honest rounding-down into a Critical.
pub(super) fn inflated_years_claims(
    generated_sections: &[Section],
    source: &str,
    reference: Option<u32>,
) -> Vec<(YearsClaim, u32)> {
    let Some(supported) = supported_years(source, reference) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    years_claims(generated_sections)
        .into_iter()
        .filter(|claim| claim.years > supported)
        .filter(|claim| seen.insert(claim.years))
        .map(|claim| (claim, supported))
        .collect()
}

// ── A2b: certifications ────────────────────────────────────────

/// How far apart an issuer and a certification word may sit, on the SOURCE
/// side, and still name one certification.
///
/// Source-side only — see [`Side`]. The claims side requires ADJACENCY, and the
/// asymmetry is the point: a window this wide reads "certified the release on
/// AWS each Thursday" as a credential, which is fine when it SPARES a claim and
/// a false accusation when it makes one.
pub(super) const CERT_ISSUER_WINDOW_CHARS: usize = 60;

/// A line at or under this length is quoted WHOLE as the evidence for a
/// certification finding: an entry in a Certifications section is already the
/// exact span the user has to look at, and "AWS Certified" alone loses the half
/// of the name that says which certification.
pub(super) const CERT_EVIDENCE_LINE_CHARS: usize = 120;

/// Which document a certification scan is reading. The two sides ask different
/// questions of the same text and the difference is deliberate — see
/// [`cert_claims_on_line`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The generated document: a certification here is a CLAIM, so the shape
    /// must be unambiguous.
    Claims,
    /// The candidate's own résumé: whatever it names may only ever spare a
    /// claim, so the reading is as generous as it can be made.
    Source,
}

/// Punctuation that may sit between an issuer and a certification word without
/// breaking their adjacency: `AWS Certified …`, `Microsoft Certified: Azure
/// …`, `Zertifizierter Kubernetes-Administrator`.
///
/// Anything else between them is a WORD, and a word is what tells "AWS
/// Certified Solutions Architect" (a credential) from "certified the release on
/// AWS" (a Thursday).
const CERT_ADJACENCY_PUNCTUATION: &[char] = &[
    '-', '\u{2013}', '\u{2014}', ':', ',', '(', ')', '.', '\u{b7}', '|', '/', '\u{2019}', '\'',
];

/// Certification acronyms, each mapped to the KEY its expansion resolves to and
/// to the expansion itself.
///
/// **One namespace, three columns, and the middle one is load-bearing.** An
/// earlier version emitted `cka` from the acronym pass and `kubernetes` from
/// the issuer pass, and `unsupported_certs` compares keys: a source writing
/// `Certified Kubernetes Administrator` therefore did not support a generated
/// `CKA`, nor the reverse. Both directions were measured as false Criticals.
/// The key is the issuer's, so a certification from an issuer the source
/// already holds a certification from is spared — deliberately generous, and
/// the alternative is exactly the two-namespace bug this column exists to kill.
///
/// The expansion is matched on the SOURCE side only, for the acronyms whose
/// long form carries no issuer token of its own (`CISSP`, `PMP`, `CFA`): without
/// it a source spelling the certification out in full would not support the
/// output's abbreviation of it.
///
/// Curated, and short on purpose. Every entry is a token that means one thing
/// on a résumé in any language — which is why it is checkable, and why the
/// list may not be grown by pattern-matching on capitalisation. Two deliberate
/// exclusions worth naming: `CSM` (also "customer success manager") and `CPA`
/// (also "cost per acquisition" on a marketing résumé). Both are real
/// certifications; neither is UNAMBIGUOUS, and an ambiguous entry here is a
/// false Critical waiting for the first marketing CV.
const CERT_ACRONYMS: &[(&str, &str, &str)] = &[
    ("PMP", "pmi", "project management professional"),
    ("CAPM", "pmi", "certified associate in project management"),
    ("PRINCE2", "prince2", ""),
    ("TOGAF", "togaf", ""),
    ("ITIL", "itil", ""),
    (
        "CISSP",
        "isc2",
        "certified information systems security professional",
    ),
    ("CCSP", "isc2", "certified cloud security professional"),
    ("CISM", "isaca", "certified information security manager"),
    ("CISA", "isaca", "certified information systems auditor"),
    ("CEH", "eccouncil", "certified ethical hacker"),
    (
        "OSCP",
        "offsec",
        "offensive security certified professional",
    ),
    ("CCNA", "cisco", "cisco certified network associate"),
    ("CCNP", "cisco", "cisco certified network professional"),
    ("CCIE", "cisco", "cisco certified internetwork expert"),
    ("CKA", "kubernetes", "certified kubernetes administrator"),
    (
        "CKAD",
        "kubernetes",
        "certified kubernetes application developer",
    ),
    (
        "CKS",
        "kubernetes",
        "certified kubernetes security specialist",
    ),
    ("RHCE", "redhat", "red hat certified engineer"),
    ("RHCSA", "redhat", "red hat certified system administrator"),
    ("CFA", "cfa", "chartered financial analyst"),
    ("FRM", "frm", "financial risk manager"),
    ("PSM", "scrum", "professional scrum master"),
    ("PSPO", "scrum", "professional scrum product owner"),
];

/// Certification issuers, each mapped to the KEY a claim is compared on — the
/// same namespace [`CERT_ACRONYMS`]' middle column uses.
///
/// Two spellings of one issuer share a key, so a source writing "Amazon Web
/// Services" supports a generated "AWS Certified …".
const CERT_ISSUERS: &[(&str, &str)] = &[
    ("amazon web services", "aws"),
    ("amazon", "aws"),
    ("aws", "aws"),
    ("microsoft", "microsoft"),
    ("azure", "microsoft"),
    ("google cloud", "google"),
    ("google", "google"),
    ("cisco", "cisco"),
    ("oracle", "oracle"),
    ("comptia", "comptia"),
    ("red hat", "redhat"),
    ("redhat", "redhat"),
    ("kubernetes", "kubernetes"),
    ("cncf", "kubernetes"),
    ("linux foundation", "linux-foundation"),
    ("salesforce", "salesforce"),
    ("scrum alliance", "scrum"),
    ("scrum", "scrum"),
    ("pmi", "pmi"),
    ("isaca", "isaca"),
    ("isc2", "isc2"),
    ("ec-council", "eccouncil"),
    ("offensive security", "offsec"),
    ("hashicorp", "hashicorp"),
    ("terraform", "hashicorp"),
    ("docker", "docker"),
    ("mongodb", "mongodb"),
    ("databricks", "databricks"),
    ("snowflake", "snowflake"),
    ("tableau", "tableau"),
    ("vmware", "vmware"),
    ("juniper", "juniper"),
    ("sap", "sap"),
    ("six sigma", "six-sigma"),
];

/// The word "certified"/"certification" in the languages this pipeline writes.
///
/// German compounds it (`Zertifizierter`, `Zertifizierung`), so the German
/// entries are matched as PREFIXES via the regex's own `\w*` tail rather than
/// spelled out one inflection at a time.
static CERT_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(certified|certification|certificate|zertifizier\w*|zertifikat\w*|certifi[ée]e?s?|certificado\w*|certificaci[óo]n|certificat[oi]|certificazion\w*|gecertificeerd|certificaat|certifica[çc][ãa]o)\b",
    )
    .unwrap()
});

/// Every issuer token, longest spelling first — `\b` does not decide an
/// alternation, and "amazon" would otherwise win against "amazon web services".
static CERT_ISSUER_RE: LazyLock<Regex> = LazyLock::new(|| {
    let mut names: Vec<&str> = CERT_ISSUERS.iter().map(|(name, _)| *name).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    Regex::new(&format!(r"(?i)\b({})\b", names.join("|"))).unwrap()
});

/// One certification a document names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CertClaim {
    /// Every key this claim may be recognised by — an acronym contributes both
    /// its own spelling and its issuer's key, so the two passes cannot end up
    /// in different namespaces again. A claim is unsupported only when NONE of
    /// them is in the source.
    pub(super) keys: Vec<String>,
    /// The span the document wrote — the evidence a finding quotes.
    pub(super) raw: String,
    pub(super) section: Option<String>,
}

/// True when `acronym` appears in `line` as an UPPERCASE, word-bounded token.
///
/// Case-sensitive on the claims side on purpose: the lowercase readings of
/// these tokens are ordinary words and abbreviations in running prose, and an
/// acronym written in caps is the shape a résumé actually names a certification
/// in.
fn contains_upper_acronym(line: &str, acronym: &str) -> bool {
    line.match_indices(acronym).any(|(i, m)| {
        let before = line[..i].chars().next_back();
        let after = line[i + m.len()..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        boundary(before) && boundary(after)
    })
}

/// True when nothing but whitespace and [`CERT_ADJACENCY_PUNCTUATION`] separates
/// the two spans `[a_end, b_start)` — i.e. the issuer and the certification
/// word are neighbouring TOKENS, in either order.
fn spans_are_adjacent(line: &str, first_end: usize, second_start: usize) -> bool {
    if first_end > second_start {
        return true; // overlapping: "Zertifizierter" IS the certification word
    }
    line[first_end..second_start]
        .chars()
        .all(|c| c.is_whitespace() || CERT_ADJACENCY_PUNCTUATION.contains(&c))
}

/// The certifications one line names, as `(keys, evidence span)`.
///
/// Two passes, because certifications are written two ways and only one of them
/// carries a word: a bare acronym (`PMP`), and an issuer beside a certification
/// word in either order (`AWS Certified …`, `Certified Kubernetes
/// Administrator`, `Zertifizierter Scrum Master`).
///
/// ## The two sides read differently, and that is the fix
///
/// "Certified" is a PAST-TENSE VERB at least as often as it is an adjective on
/// a résumé. Accepting any issuer token within [`CERT_ISSUER_WINDOW_CHARS`] of
/// it turned "Certified the release on AWS each Thursday" and "Migrated 40
/// services to Docker and certified each image against CIS" into invented
/// credentials — four of four realistic bullets fired, and the user was told to
/// delete a real achievement.
///
/// So [`Side::Claims`] requires the two to be ADJACENT tokens, which is what
/// "AWS Certified" is and what "certified … on AWS" is not, while
/// [`Side::Source`] keeps the wide window. `Source ⊇ Claims` holds by
/// construction, and it holds in the direction that matters: a source whose
/// prose merely mentions certifying something on AWS now SPARES a generated AWS
/// certification, which is a missed check rather than a false accusation.
fn cert_claims_on_line(line: &str, side: Side) -> Vec<(Vec<String>, String)> {
    let mut out = Vec::new();
    for (acronym, key, expansion) in CERT_ACRONYMS {
        let found = match side {
            Side::Claims => contains_upper_acronym(line, acronym),
            // Any casing, plus the long form: a source writing "cissp" in a
            // skills line, or spelling the certification out in full, still
            // supports the claim.
            Side::Source => {
                let lower = line.to_lowercase();
                contains_phrase(&lower, &acronym.to_lowercase())
                    || (!expansion.is_empty() && contains_phrase(&lower, expansion))
            }
        };
        if found {
            out.push((
                vec![acronym.to_lowercase(), (*key).to_string()],
                (*acronym).to_string(),
            ));
        }
    }
    for word in CERT_WORD_RE.find_iter(line) {
        for issuer in CERT_ISSUER_RE.captures_iter(line) {
            let Some(name) = issuer.get(1) else { continue };
            let (first_end, second_start) = if word.end() <= name.start() {
                (word.end(), name.start())
            } else {
                (name.end(), word.start())
            };
            let accepted = match side {
                Side::Claims => spans_are_adjacent(line, first_end, second_start),
                Side::Source => second_start.saturating_sub(first_end) <= CERT_ISSUER_WINDOW_CHARS,
            };
            if !accepted {
                continue;
            }
            let key = CERT_ISSUERS
                .iter()
                .find(|(spelling, _)| spelling.eq_ignore_ascii_case(name.as_str()))
                .map(|(_, key)| (*key).to_string())
                .unwrap_or_else(|| name.as_str().to_lowercase());
            let trimmed = line.trim();
            let raw = if trimmed.chars().count() <= CERT_EVIDENCE_LINE_CHARS {
                trimmed.to_string()
            } else {
                let start = word.start().min(name.start());
                let end = word.end().max(name.end());
                line[start..end].trim().to_string()
            };
            out.push((vec![key], raw));
        }
    }
    out
}

/// Every certification the generated document names.
pub(super) fn cert_claims(sections: &[Section]) -> Vec<CertClaim> {
    let mut out = Vec::new();
    for section in sections {
        for line in &section.lines {
            for (keys, raw) in cert_claims_on_line(&line.text, Side::Claims) {
                out.push(CertClaim {
                    keys,
                    raw,
                    section: section.heading.clone(),
                });
            }
        }
    }
    out
}

/// Every certification key the SOURCE names — the lenient side.
pub(super) fn source_cert_keys(source: &str) -> HashSet<String> {
    source
        .lines()
        .flat_map(|line| cert_claims_on_line(line, Side::Source))
        .flat_map(|(keys, _)| keys)
        .collect()
}

/// Certifications the generated document names that the source never does.
pub(super) fn unsupported_certs(generated_sections: &[Section], source: &str) -> Vec<CertClaim> {
    let known = source_cert_keys(source);
    let mut seen = HashSet::new();
    cert_claims(generated_sections)
        .into_iter()
        .filter(|claim| !claim.keys.iter().any(|key| known.contains(key)))
        .filter(|claim| {
            claim
                .keys
                .first()
                .is_some_and(|key| seen.insert(key.clone()))
        })
        .collect()
}

// ── A2c: education institutions ─────────────────────────────────────────────

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
fn names_a_degree(text: &str) -> bool {
    let lower = text.to_lowercase();
    DEGREE_TOKENS.iter().any(|d| contains_phrase(&lower, d))
}

/// True when `text` names an institution anywhere.
pub(super) fn names_an_institution(text: &str) -> bool {
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
pub(super) fn institutions(sections: &[Section]) -> Vec<(String, Option<String>)> {
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
pub(super) fn unsupported_institutions(
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
    institutions(generated_sections)
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
