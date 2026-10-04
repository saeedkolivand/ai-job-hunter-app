//! A2a — years of experience. See the parent module for why this one is the
//! only member of the family that compares a value across languages.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::SectionKind;

use super::super::{contains_phrase, Section};
use super::{names_a_role, word_tokens};

mod numbers;
mod span;

#[cfg(test)]
pub use self::numbers::spelled_number_words;
use self::numbers::{years_spans, SPELLED_NUMBERS, UNREADABLE_TENURE_RE};
#[cfg(test)]
pub use self::span::SPAN_TAIL_CHARS;
pub use self::span::{career_span_years, reference_year};

// ── The claim, and what the source supports ────────────────────────────────

/// Slack, in years, added to a career span computed from YEAR NUMBERS ONLY.
///
/// A résumé's date column carries years, not months: `2018 - 2021` is anything
/// from 24 months (Dec 2018 → Jan 2021) to 47 (Jan 2018 → Dec 2021). The
/// difference of the two year numbers is therefore a LOWER bound on the true
/// span, and one year is the exact amount by which it can understate. Rounding
/// the other way — accusing a candidate whose eight years are really 7.6 —
/// is the failure this family cannot afford.
pub const CAREER_SPAN_SLACK_YEARS: u32 = 1;

/// How far either side of a `<number> <year-word>` span an experience-context
/// word may sit and still make the span a TENURE CLAIM.
///
/// Wide enough for the shapes real documents write ("8+ years of experience",
/// "acht Jahre Erfahrung", "Erfahrung: acht Jahre", "ten years of professional
/// experience"), narrow enough that an unrelated sentence later in the same
/// bullet cannot supply the context.
pub const CLAIM_CONTEXT_CHARS: usize = 40;

/// The largest tenure this reads as a claim about a person.
///
/// Above it the number is about something else — a system, a company, a
/// dataset — and a check that accuses someone of overstating a 150-year tenure
/// is reporting a parse failure as a fabrication.
pub const MAX_PLAUSIBLE_TENURE_YEARS: u32 = 60;

/// A tenure stated in decades. Neither [`YEARS_RE`] nor
/// [`UNREADABLE_TENURE_RE`] sees these — there is no year-word to anchor on —
/// so "over a decade of experience" read as a source that states NOTHING, and
/// four truthful documents earned a Critical for restating their own tenure.
///
/// Read as unknown rather than mapped to ten: "over a decade" is anywhere from
/// ten years to nineteen, and picking a number would be inventing evidence on
/// the side of the comparison that must never invent any.
static DECADE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(decades?|jahrzehnt(?:en|e)?|d[ée]cennies?|d[ée]cadas?|decenios?|decenni[oi]|decenni(?:um|a))\b",
    )
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

/// Words that may stand between the start of a clause and a tenure span.
///
/// "Backend engineer with 8 years building distributed services" is a tenure
/// claim; "retired 12 years of accumulated schema drift" is an achievement, and
/// the only lexical difference between them is what sits immediately before the
/// number. A lead-in, or nothing at all.
const TENURE_LEAD_INS: &[&str] = &["with", "mit", "avec", "con", "com", "met"];

/// Tokens a tenure span may be FOLLOWED by in a summary — the preposition or
/// participle that turns a bare count into a span of working life.
///
/// The second half of the same discrimination: "30 year OLD mainframe" and "40
/// year LEGACY batch" are not tenures, and neither word is here.
const TENURE_FOLLOWERS: &[&str] = &[
    "of",
    "in",
    "on",
    "across",
    "at",
    "within",
    "building",
    "shipping",
    "leading",
    "running",
    "working",
    "managing",
    "delivering",
    "designing",
    "developing",
    "spanning",
    "supporting",
    "im",
    "bei",
    "als",
    "an",
    "de",
    "du",
    "dans",
    "en",
    "d",
    "nel",
    "su",
    "na",
    "no",
    "em",
    "aan",
    "op",
];

/// How many tokens of the SUBJECT are read, looking for the person the tenure
/// belongs to.
///
/// Two, because a job title is routinely two words and the head noun comes
/// last: "Backend engineer", "Ingénieure backend", "Senior Software Engineer",
/// "Product Designer". One token would drop the French and Dutch orderings; a
/// whole-clause scan would re-admit "The engineer rebuilt a platform with 15
/// years of debt", which is the register this rule exists to reject.
pub const TENURE_SUBJECT_TOKENS: usize = 2;

/// True when the span `[start, end)` reads as a claim about the candidate's own
/// working life rather than about a system, a contract or a migration.
///
/// ## Two admissions, and both are needed
///
/// The first is an [`EXPERIENCE_CONTEXT`] word within [`CLAIM_CONTEXT_CHARS`] —
/// "8+ years of experience", "acht Jahre Erfahrung", "15 años de experiencia".
///
/// The second exists because this repo's own truthful fixtures do not use that
/// shape. `en_generated_clean.txt` writes "Eight years of backend work, most of
/// it on payment systems" and `tests/corpus/synthetic_swe.txt` writes "Backend
/// engineer with 8 years building distributed services": no experience word
/// anywhere near either, and an inflated résumé writes those same sentences
/// with a bigger number. An earlier version admitted ANYTHING in a summary to
/// cover them, which made "replaced a 30 year old mainframe" a tenure claim;
/// removing that wholesale stopped six of twenty-one truthful fixtures being
/// read at all.
///
/// So the summary admission is kept and made shape-aware, on the SUBJECT and
/// on the follower.
///
/// ## The subject is where it separates, and that was measured
///
/// A first cut asked only that the span open a clause. That reads a sentence
/// about a SYSTEM as a claim about a person, fifteen times out of fifteen:
/// "Rebuilt a platform with 15 years of accumulated technical debt", "Joined a
/// team with 12 years of shipping history", "Inherited a codebase with 20 years
/// in production" — and the same shape in every language, built from these very
/// lead-in and follower lists. The message then tells the user to correct a
/// true sentence about a platform.
///
/// The critic printed the clause head for every false positive and every
/// truthful line and got total separation on ONE position: the false ones end
/// in `platform | team | codebase | stack | ledger | service | mainframe`, the
/// truthful ones in `engineer | developer | designer | Ingenieurin`, or open
/// the line. So the subject must be a PERSON ([`super::ROLE_NOUNS`]) or absent.
///
/// The follower list is deliberately NOT where this is fixed: `of|in|on|at`
/// follow any counted noun phrase whatever, and no vocabulary in that position
/// separates a platform's years from a person's.
fn is_tenure_context(line: &str, start: usize, end: usize, allow_summary_shape: bool) -> bool {
    let window = context_window(line, start, end, CLAIM_CONTEXT_CHARS).to_lowercase();
    if EXPERIENCE_CONTEXT
        .iter()
        .any(|word| contains_phrase(&window, word))
    {
        return true;
    }
    if !allow_summary_shape {
        return false;
    }
    // The whole head, not the clause: "Backend engineer, eight years across …"
    // puts the person one comma back, and a clause-scoped head would see it as
    // empty and admit anything.
    let mut head = word_tokens(&line[..start]);
    // A lead-in is not the subject, it introduces it.
    if head
        .last()
        .is_some_and(|last| TENURE_LEAD_INS.contains(&last.as_str()))
    {
        head.pop();
    }
    let subject: Vec<String> = head
        .iter()
        .rev()
        .take(TENURE_SUBJECT_TOKENS)
        .cloned()
        .collect();
    let opens_clause = subject.is_empty() || names_a_role(&subject);
    let followed = word_tokens(&line[end..])
        .first()
        .is_some_and(|next| TENURE_FOLLOWERS.contains(&next.as_str()));
    opens_clause && followed
}

/// One `<number> years` span a document states, with where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearsClaim {
    /// The number, whether the document wrote it in digits or in words. This is
    /// the only field a cross-language comparison may look at.
    pub years: u32,
    /// The exact span the document wrote — the evidence a finding quotes.
    pub raw: String,
    /// The heading of the section the span sits in, for a section-scoped
    /// finding.
    pub section: Option<String>,
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

/// The largest tenure the SOURCE states anywhere, in any shape.
///
/// Deliberately context-free, the same leniency `factual::all_numbers` gives
/// the truth side of the metric comparison: a statement in the candidate's own
/// document may only ever SPARE a claim, so reading too much of it is safe
/// while reading too little manufactures accusations.
pub fn stated_years(source: &str) -> Option<u32> {
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
/// TWO admission rules, both in [`is_tenure_context`] and documented there: an
/// experience word near the span, or — in a summary only — the shape of a
/// tenure sentence, which is a PERSON as its subject and a preposition or
/// participle after it.
///
/// Neither admits "replaced a 30 year old mainframe", "retired 12 years of
/// accumulated schema drift" or "rebuilt a platform with 15 years of technical
/// debt". All three are achievements a summary really contains, all three once
/// fired, and each time the user was told to correct a true sentence.
pub fn years_claims(sections: &[Section]) -> Vec<YearsClaim> {
    let mut out = Vec::new();
    for section in sections {
        let summary = section.kind == SectionKind::Summary;
        for line in &section.lines {
            for (years, raw, start, end) in years_spans(&line.text) {
                if is_tenure_context(&line.text, start, end, summary) {
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

/// A TENURE in the SOURCE that this file cannot put a number on: a year-word
/// with a quantifier [`SPELLED_NUMBERS`] cannot read ("several years", a number
/// word in a language the table misses), or a decade.
///
/// Such a source states a tenure of UNKNOWN size, and unknown is not zero. The
/// whole check goes quiet rather than compare a claim against evidence it
/// failed to read: the sparing side going blind is what manufactures an
/// accusation, and a word list can always be missing a word.
///
/// **Scoped by the same admission rule the claims side uses**, and that scoping
/// is the difference between a guard and an OFF SWITCH. Unscoped, this matched
/// any letter-word before any year-word anywhere in the document — so
/// "Cut cloud spend by 1.2M USD per year", "Reported year over year growth" and
/// "Ran the fiscal year close" each silenced `factual.inflated_experience` for
/// the entire résumé. `$X per year` is close to the most common quantified
/// impact phrasing there is, and a precision-only calibration cannot tell that
/// apart from a fix: both score zero false positives.
///
/// The summary shape is allowed on EVERY line here, not just summary ones,
/// because this side may only ever SPARE — which also keeps the admission rule
/// a superset of the claims side's, by construction.
fn states_an_unreadable_tenure(source: &str) -> bool {
    source.lines().any(|line| {
        UNREADABLE_TENURE_RE.captures_iter(line).any(|c| {
            let Some(word) = c.get(1) else { return false };
            let Some(whole) = c.get(0) else { return false };
            let unreadable = !SPELLED_NUMBERS
                .iter()
                .any(|(w, _)| *w == word.as_str().to_lowercase());
            unreadable && is_tenure_context(line, whole.start(), whole.end(), true)
        }) || DECADE_RE
            .find_iter(line)
            .any(|m| is_tenure_context(line, m.start(), m.end(), true))
    })
}

/// The largest tenure the source supports: whatever it states, or the span its
/// dates cover plus [`CAREER_SPAN_SLACK_YEARS`], whichever is larger.
///
/// `None` when the source supports NO reading at all — it states no tenure and
/// carries no dated entry, or it states one this file cannot READ
/// ([`states_an_unreadable_tenure`]). The comparison is then unmakeable and the
/// check must stay silent rather than treat "unknown" as "zero".
pub fn supported_years(source: &str, reference: Option<u32>) -> Option<u32> {
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
pub fn inflated_years_claims(
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
