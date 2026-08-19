//! A2a — years of experience. See the parent module for why this one is the
//! only member of the family that compares a value across languages.

use std::collections::HashSet;
use std::sync::LazyLock;

use chrono::Datelike;
use regex::Regex;

use crate::documents::evidence::{is_open_ended, years_in, SectionKind, PRESENT_MARKERS};

use super::super::{contains_phrase, Section};
use super::{names_a_role, word_tokens};

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

/// The distinct spellings [`YEARS_RE`] alternates on, longest first — `\b`
/// alone does not decide an alternation, and `vier` would otherwise win
/// against `vierzehn`.
///
/// Deduplicated by VALUE first, not by adjacency after the length sort:
/// `Vec::dedup` removes only CONSECUTIVE equal elements, and sorting by
/// LENGTH groups equal lengths, not equal strings — a few spellings repeat
/// across language blocks at a different position in the table (`quatorze` is
/// French 14 and repeats in the Portuguese block; `tres` is Spanish 3 and
/// repeats in the Portuguese block), and other same-length words sit between
/// the two copies, so a length-only sort never makes them adjacent.
pub fn spelled_number_words() -> Vec<&'static str> {
    let mut words: Vec<&str> = SPELLED_NUMBERS.iter().map(|(w, _)| *w).collect();
    words.sort_unstable();
    words.dedup();
    words.sort_by_key(|w| std::cmp::Reverse(w.len()));
    words
}

/// `<number> <year-word>`, where the separator is whitespace or a `+`.
///
/// The separator rule is what keeps the hyphenated ADJECTIVE out: "a 20-year-old
/// legacy stack" is not a claim about anybody's tenure, and it is the single
/// most common two-digit `year` collocation in engineering prose. `\b` on the
/// number keeps the check out of a four-digit date ("2014 - 2018" offers no
/// position where one or two digits end on a word boundary).
static YEARS_RE: LazyLock<Regex> = LazyLock::new(|| {
    let words = spelled_number_words();
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
pub fn reference_year(source: &str, generated: &str) -> Option<u32> {
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
pub const SPAN_TAIL_CHARS: usize = 16;

/// `<year> <span separator> <tail>` — the shape a date column has, read off raw
/// TEXT rather than off a parsed entry.
static SPAN_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?:19|20)\d{{2}}\s*(?:[-–—]|\bto\b|\bbis\b|\buntil\b|\bhasta\b|\bau\b|\bà\b|\ba\b|\btot\b|\bfino\b|\baté\b)\s*([^\n]{{0,{SPAN_TAIL_CHARS}}})"
    ))
    .unwrap()
});

/// Openers that make a date column open-ended, matched as BARE TOKENS.
///
/// `documents::evidence::is_open_ended` knows these words but requires a year
/// to follow within one optional word, so `Seit März 2016` is open and
/// `Seit 03/2016` is not — and a numeric month is exactly what a German, French
/// or Spanish date column usually carries. Measured: the current role then read
/// as closed at its own start year and a truthful fifteen years became a
/// Critical. Same vocabulary as that function, deliberately, so the two cannot
/// disagree about which words these are; only the adjacency rule differs.
static SPAN_OPENER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(since|seit|from|ab|depuis|desde|dal|dalla|vanaf|sinds)\b").unwrap()
});

/// A [`PRESENT_MARKERS`] word anywhere on a line that ALREADY CARRIES A YEAR —
/// `2021 Present`, `2021 | Heute`, `2015 · Aktuell`, `2016 (ongoing)`.
///
/// `documents::evidence::is_open_ended` used to answer this and deliberately
/// no longer does: it now demands an explicit span separator (`-`, `to`,
/// `bis`, …) between the year and the marker, because that predicate also
/// runs against whole prose sentences, where "a year, and a marker word a few
/// words later" is indistinguishable from ordinary text ("Reduced actual
/// costs by 20% in 2023"). Here it IS distinguishable, and that is why this
/// arm is local to this file: [`source_is_ongoing`] has already established
/// the line carries a year, and every direction of error it can make is the
/// generous one.
///
/// **Why no separator list of its own.** The first attempt at this arm allowed
/// only WHITESPACE between the year and the marker, which left a pipe, a
/// middot, a comma and a parenthesis reading as CLOSED — the one dangerous
/// answer [`source_is_ongoing`] documents. Measured: a truthful
/// `… | 2015 | Present` history collapsed from an eleven-year span to a
/// zero-year one and raised a false `factual.inflated_experience` Critical.
/// Any enumeration of separators fails the same way one spelling further out,
/// and `export::parser::DATE_RE` — this codebase's own definition of a job
/// entry's date range — already allows 30 ARBITRARY characters there. So this
/// asks only for the marker and lets the caller's year requirement carry the
/// structure.
///
/// Same vocabulary as [`is_open_ended`], deliberately, so the two can never
/// disagree about which words these are; only the adjacency rule differs.
fn names_a_present_marker(line: &str) -> bool {
    let lower = line.to_lowercase();
    PRESENT_MARKERS.iter().any(|m| contains_phrase(&lower, m))
}

/// True when the source shows a role that has NOT ended.
///
/// Asked of raw text, and asked STRUCTURALLY: a date column whose separator is
/// followed by something that is not another year is open, whatever word sits
/// there. That is what makes it work for `2015 - Actualidad`, `2019 -
/// Aujourd'hui` and `2020 - Today`, none of which `PRESENT_MARKERS` carries —
/// and for the next spelling nobody has thought of either. The marker list is
/// still consulted, because `seit 2021` is open with no separator at all, and
/// so is [`names_a_present_marker`], because a marker can sit next to a year
/// behind any separator at all — or none.
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
            || SPAN_OPENER_RE.is_match(line)
            || names_a_present_marker(line)
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
pub fn career_span_years(source: &str, reference: Option<u32>) -> Option<u32> {
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
