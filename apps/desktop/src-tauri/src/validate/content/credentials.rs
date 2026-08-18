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
//!   years and against the span its employment dates cover. A number survives
//!   translation and paraphrase intact, so this one is checkable in any
//!   language pair — which matters now that a German résumé is routinely
//!   generated from an English source.
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

/// Number words a tenure is spelled with, in the two languages this pipeline
/// curates lexicons for.
///
/// Digits are handled by the regex; this table exists because the ONE truthful
/// shape both clean fixtures use is spelled out ("eight years", "acht Jahre"),
/// and a check that cannot read the truthful form would compare a generated
/// claim against a source it thinks says nothing.
const SPELLED_NUMBERS: &[(&str, u32)] = &[
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
    Regex::new(&format!(
        r"(?i)\b(\d{{1,2}}|{})(?:\s*\+\s*|\s+)({})\b",
        words.join("|"),
        YEAR_WORDS
    ))
    .unwrap()
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

/// Every tenure CLAIM the generated document makes.
///
/// A span is a claim when an [`EXPERIENCE_CONTEXT`] word sits within
/// [`CLAIM_CONTEXT_CHARS`] of it, or when it sits in the summary — the section
/// whose entire job is to state what the candidate has done, and where this
/// class of inflation actually lands.
pub(super) fn years_claims(sections: &[Section]) -> Vec<YearsClaim> {
    let mut out = Vec::new();
    for section in sections {
        let is_summary = section.kind == SectionKind::Summary;
        for line in &section.lines {
            for (years, raw, start, end) in years_spans(&line.text) {
                let window =
                    context_window(&line.text, start, end, CLAIM_CONTEXT_CHARS).to_lowercase();
                let has_context = EXPERIENCE_CONTEXT
                    .iter()
                    .any(|word| contains_phrase(&window, word));
                if has_context || is_summary {
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

/// The span the source's own employment dates cover, earliest start to latest
/// end, with an open-ended entry ending at `reference`.
///
/// The CAREER span, not the sum of the roles: a gap between two jobs is not
/// something to accuse anybody over, and the career reading is the larger of
/// the two. `None` when no source entry carries a year — nothing to compare
/// against, so the check goes quiet — and equally `None` when an entry is
/// open-ended and [`reference_year`] had no trustworthy today to close it with:
/// one unbounded end makes the whole career span unknown, and "unknown" must
/// never be read as "short".
pub(super) fn career_span_years(
    source_sections: &[Section],
    reference: Option<u32>,
) -> Option<u32> {
    let mut earliest: Option<u32> = None;
    let mut latest: Option<u32> = None;
    for (_, dates) in super::factual::entries(source_sections) {
        let years = years_in(&dates);
        let Some(&start) = years.first() else {
            continue;
        };
        let end = if is_open_ended(&dates) {
            reference?
        } else {
            years.iter().copied().max().unwrap_or(start)
        };
        earliest = Some(earliest.map_or(start, |e| e.min(start)));
        latest = Some(latest.map_or(end, |l| l.max(end)));
    }
    Some(latest?.saturating_sub(earliest?))
}

/// The largest tenure the source supports: whatever it states, or the span its
/// dates cover plus [`CAREER_SPAN_SLACK_YEARS`], whichever is larger.
///
/// `None` when the source supports NO reading at all (states no tenure and
/// carries no dated entry) — the comparison is then unmakeable and the check
/// must stay silent rather than treat "unknown" as "zero".
pub(super) fn supported_years(
    source: &str,
    source_sections: &[Section],
    reference: Option<u32>,
) -> Option<u32> {
    let stated = stated_years(source);
    let span = career_span_years(source_sections, reference).map(|s| s + CAREER_SPAN_SLACK_YEARS);
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
    source_sections: &[Section],
    reference: Option<u32>,
) -> Vec<(YearsClaim, u32)> {
    let Some(supported) = supported_years(source, source_sections, reference) else {
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

// ── A2b: certifications ─────────────────────────────────────────────────────

/// How far apart an issuer and a certification word may sit and still name one
/// certification. A line is the outer bound either way — two adjacent entries
/// in a Certifications section must not fuse into one claim.
pub(super) const CERT_ISSUER_WINDOW_CHARS: usize = 60;

/// A line at or under this length is quoted WHOLE as the evidence for a
/// certification finding: an entry in a Certifications section is already the
/// exact span the user has to look at, and "AWS Certified" alone loses the half
/// of the name that says which certification.
pub(super) const CERT_EVIDENCE_LINE_CHARS: usize = 120;

/// Certification acronyms distinctive enough that their ABSENCE from the source
/// is evidence.
///
/// Curated, and short on purpose. Every entry is a token that means one thing
/// on a résumé in any language — which is exactly why it is checkable and why
/// the list may not be grown by pattern-matching on capitalisation. Two
/// deliberate exclusions worth naming: `CSM` (also "customer success manager")
/// and `CPA` (also "cost per acquisition" on a marketing résumé). Both are real
/// certifications; neither is UNAMBIGUOUS, and an ambiguous entry here is a
/// false Critical waiting for the first marketing CV.
const CERT_ACRONYMS: &[&str] = &[
    "PMP", "CAPM", "PRINCE2", "TOGAF", "ITIL", "CISSP", "CISM", "CISA", "CCSP", "CEH", "OSCP",
    "CCNA", "CCNP", "CCIE", "CKA", "CKAD", "CKS", "RHCE", "RHCSA", "CFA", "FRM", "PSM", "PSPO",
];

/// Certification issuers, each mapped to the KEY a claim is compared on.
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
    /// What the two sides are compared on: an issuer key or a lowercased
    /// acronym. Translation-invariant by construction.
    pub(super) key: String,
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

/// The certifications one line names.
///
/// Two passes, because certifications are written two ways and only one of them
/// carries a word: a bare acronym (`PMP`), and an issuer next to a
/// certification word in either order (`AWS Certified …`, `Certified Kubernetes
/// Administrator`, `Zertifizierter Scrum Master`). A certification word with NO
/// issuer near it is skipped — "certified the release" is not a credential, and
/// guessing at one is how this check would start accusing people.
fn cert_claims_on_line(line: &str, upper_acronyms_only: bool) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for acronym in CERT_ACRONYMS {
        let found = if upper_acronyms_only {
            contains_upper_acronym(line, acronym)
        } else {
            contains_phrase(&line.to_lowercase(), &acronym.to_lowercase())
        };
        if found {
            out.push((acronym.to_lowercase(), (*acronym).to_string()));
        }
    }
    for word in CERT_WORD_RE.find_iter(line) {
        for issuer in CERT_ISSUER_RE.captures_iter(line) {
            let Some(name) = issuer.get(1) else { continue };
            let gap = if word.end() <= name.start() {
                name.start() - word.end()
            } else if name.end() <= word.start() {
                word.start() - name.end()
            } else {
                0 // the two spans overlap — "Zertifizierter" IS the issuer word
            };
            if gap > CERT_ISSUER_WINDOW_CHARS {
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
            out.push((key, raw));
        }
    }
    out
}

/// Every certification the generated document names.
pub(super) fn cert_claims(sections: &[Section]) -> Vec<CertClaim> {
    let mut out = Vec::new();
    for section in sections {
        for line in &section.lines {
            for (key, raw) in cert_claims_on_line(&line.text, true) {
                out.push(CertClaim {
                    key,
                    raw,
                    section: section.heading.clone(),
                });
            }
        }
    }
    out
}

/// Every certification key the SOURCE names — the lenient side.
///
/// Acronyms are matched case-INSENSITIVELY here, unlike on the claims side: a
/// source that writes "cissp" in a skills line still supports the claim, and a
/// statement in the candidate's own document may only ever spare.
pub(super) fn source_cert_keys(source: &str) -> HashSet<String> {
    source
        .lines()
        .flat_map(|line| cert_claims_on_line(line, false))
        .map(|(key, _)| key)
        .collect()
}

/// Certifications the generated document names that the source never does.
pub(super) fn unsupported_certs(generated_sections: &[Section], source: &str) -> Vec<CertClaim> {
    let known = source_cert_keys(source);
    let mut seen = HashSet::new();
    cert_claims(generated_sections)
        .into_iter()
        .filter(|claim| !known.contains(&claim.key))
        .filter(|claim| seen.insert(claim.key.clone()))
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
/// The double guard is deliberate. `classify_section` does not recognise every
/// heading a real CV writes, so a source whose education sits under an
/// unrecognised heading would look sectionless — the marker scan over the WHOLE
/// source text is what stops that from becoming an accusation.
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
        || names_an_institution(source);
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
    let mut issues: Vec<ContentIssue> = inflated_years_claims(
        &ctx.generated_sections,
        source,
        &ctx.source_sections,
        reference,
    )
    .into_iter()
    .map(|(claim, supported)| {
        issue(
            FACTUAL_INFLATED_EXPERIENCE,
            claim.section.as_deref(),
            format!(
                "\"{}\" claims more experience than your source résumé supports — it states, \
                 and its dates cover, at most {supported} years. Correct it to a figure your \
                 own document backs.",
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
