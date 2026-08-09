//! Evidence extraction — the candidate's OWN résumé, structured and scored
//! against one posting.
//!
//! Two entry points over one scorer:
//!
//! * [`rank_bullets`] — the bullet scorer moved verbatim out of
//!   `commands::match_resume::rank_trim_candidates`. Ranks a résumé's bullets
//!   weakest-first for a posting; the trim panel still consumes it through a
//!   `TrimCandidate: From<EvidenceBullet>` shim so its IPC payload is unchanged.
//! * [`extract_evidence`] — the same scoring applied section-by-section, so a
//!   generation prompt (or a validator) can talk about *which role* an
//!   achievement came from instead of a flat list of lines.
//!
//! The point of the module is the honesty spine: the model decides HOW to
//! present the candidate's verified evidence, never WHAT the candidate has
//! done. Everything here is derived from the source résumé — nothing is
//! invented, nothing comes from a model.
//!
//! Deliberately embedding-free: it reuses the `documents::keywords` kernel and
//! the same JD-derived stemmer as `score_one`, so no surface here can disagree
//! with the match score the user sees for the same pair. Zero model calls,
//! works offline.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use rust_stemmers::Stemmer;
use serde::Serialize;

use crate::documents::keywords::{
    detect_locale_tag, display_forms, keywords, keywords_normalized, languages_align, make_stemmer,
};
use crate::export::parser::parse_resume;
use crate::export::types::{LineKind, ParsedLine};
use crate::observability::Span;

/// One résumé bullet, scored by how much of THIS posting's vocabulary it carries.
///
/// `score` is a hit COUNT stored as `f64` (always a non-negative whole number)
/// so a future weighted scorer can refine it without a wire-type change.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceBullet {
    /// Stable, document-order id (`b3`, `r1b0`, `p2`) — lets a prompt or a
    /// validator reference one specific line without quoting it back.
    pub id: String,
    /// The bullet's markdown-stripped text, as the reader sees it.
    pub text: String,
    /// Readable (unstemmed) job keywords this line carries — may be empty.
    pub hits: Vec<String>,
    pub score: f64,
}

/// One employment entry with the bullets that belong to it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRole {
    pub company: String,
    pub title: String,
    /// The entry's date span exactly as the source wrote it (`2021 – Present`).
    pub dates: String,
    pub bullets: Vec<EvidenceBullet>,
}

/// Everything the source résumé can actually vouch for, for one posting.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSet {
    pub roles: Vec<EvidenceRole>,
    /// Posting keywords the source résumé DOES evidence, readable + sorted.
    pub skills_present: Vec<String>,
    /// Posting keywords it does not — the honest gap list, readable + sorted.
    pub skills_absent: Vec<String>,
    pub education: Vec<String>,
    pub projects: Vec<EvidenceBullet>,
}

/// The posting's vocabulary, resolved once with the language discipline every
/// résumé↔posting surface in this codebase shares.
///
/// Symmetric normalization, exactly as `score_one` does it: stem BOTH sides
/// with the JD-derived stemmer when the languages align, leave BOTH
/// normalized-only when they diverge. Stemming one side alone mutates
/// language-neutral tech tokens on that side only and matches neither set.
///
/// The résumé's language is DETECTED rather than read from a stored locale —
/// this path scores generator output that was never persisted, so there is no
/// `DocumentRecord::locale` to consult.
struct JobVocabulary {
    aligned: bool,
    stemmer: Stemmer,
    keywords: HashSet<String>,
    /// Stem → readable, unstemmed display form for every posting keyword.
    display: HashMap<String, String>,
    /// The posting's own detected language tag — picks the [`function_words`]
    /// list the present/absent split is filtered with.
    lang: &'static str,
}

impl JobVocabulary {
    fn new(resume_text: &str, job_text: &str) -> Self {
        let aligned = languages_align(job_text, detect_locale_tag(resume_text));
        let stemmer = make_stemmer(job_text);
        let lang = detect_locale_tag(job_text);
        let keywords = if aligned {
            keywords(job_text, &stemmer)
        } else {
            keywords_normalized(job_text)
        };
        // Display forms are keyed on whatever the JD side produced, so they must
        // be built the same way — a stemmed map would miss every unstemmed hit.
        let display = if aligned {
            display_forms(job_text, &stemmer)
        } else {
            keywords_normalized(job_text)
                .into_iter()
                .map(|t| (t.clone(), t))
                .collect()
        };
        Self {
            aligned,
            stemmer,
            keywords,
            display,
            lang,
        }
    }

    /// The readable form of a keyword — the unstemmed token the posting used,
    /// falling back to the key itself.
    fn readable(&self, token: &str) -> String {
        self.display
            .get(token)
            .cloned()
            .unwrap_or_else(|| token.to_string())
    }

    /// This side's tokens, normalized the same way the posting's were.
    fn tokens(&self, text: &str) -> HashSet<String> {
        if self.aligned {
            keywords(text, &self.stemmer)
        } else {
            keywords_normalized(text)
        }
    }

    /// Score one line: the readable posting keywords it carries.
    fn bullet(&self, id: String, text: &str) -> EvidenceBullet {
        let mut hits: Vec<String> = self
            .tokens(text)
            .intersection(&self.keywords)
            .map(|stem| self.readable(stem))
            .collect();
        hits.sort();
        EvidenceBullet {
            id,
            score: hits.len() as f64,
            hits,
            text: text.to_string(),
        }
    }
}

/// Weakest first. Among lines carrying equally little, the LONGEST goes first:
/// same loss to the reader, most space recovered.
fn sort_weakest_first(bullets: &mut [EvidenceBullet]) {
    bullets.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| b.text.len().cmp(&a.text.len()))
    });
}

/// Rank a résumé's bullets weakest-first for a given posting.
///
/// Only `LineKind::Bullet` lines are candidates — section headers, job entries
/// and the contact block are structural, and cutting them to save space is never
/// the right advice.
///
/// Returns empty when the posting has no extractable keywords, mirroring
/// `keyword_coverage`'s `None`: with nothing to score against, every line would
/// tie at zero and the ranking would be noise dressed up as a recommendation.
///
/// Ids are `b<n>` in the résumé's own bullet order, assigned BEFORE the ranking
/// sort, so `b0` always means "the first bullet in the document".
pub fn rank_bullets(text: &str, job_text: &str) -> Vec<EvidenceBullet> {
    let vocab = JobVocabulary::new(text, job_text);
    if vocab.keywords.is_empty() {
        return Vec::new();
    }
    let mut bullets: Vec<EvidenceBullet> = parse_resume(text)
        .lines
        .iter()
        .filter(|line| matches!(line.kind, LineKind::Bullet))
        .enumerate()
        .map(|(i, line)| vocab.bullet(format!("b{i}"), &line.text))
        .collect();
    sort_weakest_first(&mut bullets);
    bullets
}

/// Which broad kind of section a heading names. Classification only — DETECTING
/// that a line *is* a heading stays `export::parser`'s job; this just buckets the
/// heading text so evidence lands in the right list.
///
/// Public and shared with `validate::content`, which needs the same buckets: two
/// classifiers disagreeing about what "SKILLS" means would let a validator warn
/// about a section the evidence extractor filed somewhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Experience,
    Education,
    Projects,
    Skills,
    Summary,
    Other,
}

/// Heading fragments per kind, matched as a substring of the lowercased heading
/// so "PROFESSIONAL EXPERIENCE" and "Berufserfahrung" both land correctly.
/// en/de/fr/es/it/nl/pt — the languages `make_stemmer` already supports.
const EXPERIENCE_HEADINGS: &[&str] = &[
    "experience",
    "employment",
    "career",
    "berufserfahrung",
    "arbeitserfahrung",
    "expérience",
    "experiencia",
    "esperienza",
    "werkervaring",
    "experiência",
];
const EDUCATION_HEADINGS: &[&str] = &[
    "education",
    "academic",
    "ausbildung",
    "bildung",
    "opleiding",
];

/// Education stems that are also the tail of an ordinary word, matched with
/// [`contains_word`] instead of as a bare substring.
///
/// `formation` hides inside `information` — and `formación`/`formação`/
/// `formazione` inside `información`/`informação`/`informazione` — so the
/// "PERSONAL INFORMATION" heading that opens half the CVs in Europe classified
/// as EDUCATION, filing the candidate's phone number and email as a degree.
/// Both the singular and the plural are listed because the word-boundary rule
/// is exact at BOTH ends: without `formations`, a French "FORMATIONS" heading
/// would stop classifying, and adding it to the substring list above would
/// re-open the collision on `informations`.
///
/// Only these stems are word-bounded. Every other entry stays a substring
/// because German compounds a heading straight into a longer word
/// (`Weiterbildung` → `bildung`, `Berufsausbildung` → `ausbildung`), which a
/// both-ends boundary would break.
const EDUCATION_HEADINGS_WORD_BOUNDED: &[&str] = &[
    "formation",
    "formations",
    "formación",
    "formaciones",
    "formação",
    "formações",
    "formazione",
    "formazioni",
];
const PROJECT_HEADINGS: &[&str] = &["project", "projekt", "projet", "proyecto", "progetti"];
const SKILLS_HEADINGS: &[&str] = &[
    "skill",
    "competenc",
    // Italian "COMPETENZE" — `competenc` (English "competencies") does not
    // cover it, and a missed skills heading silently disables every
    // skills-section check on an Italian résumé.
    "competenz",
    "fähigkeit",
    "kenntnis",
    "kompetenz",
    "compétence",
    "habilidad",
    "vaardigheden",
    "technologies",
    "tech stack",
];
const SUMMARY_HEADINGS: &[&str] = &[
    "summary",
    "profile",
    "objective",
    "about",
    "zusammenfassung",
    "profil",
    "perfil",
    "profilo",
    "profiel",
];

/// Bucket a heading. Substring match on the lowercased heading, checked
/// most-specific-first, so "PROFESSIONAL EXPERIENCE" and "Berufserfahrung" both
/// land on [`SectionKind::Experience`] without a per-locale word list. The
/// exception is [`EDUCATION_HEADINGS_WORD_BOUNDED`], whose stems are substrings
/// of ordinary words and are therefore matched with [`contains_word`].
pub fn classify_section(heading: &str) -> SectionKind {
    let lower = heading.to_lowercase();
    let has = |set: &[&str]| set.iter().any(|k| lower.contains(k));
    let has_word = |set: &[&str]| set.iter().any(|k| contains_word(&lower, k));
    if has(EXPERIENCE_HEADINGS) {
        SectionKind::Experience
    } else if has(EDUCATION_HEADINGS) || has_word(EDUCATION_HEADINGS_WORD_BOUNDED) {
        SectionKind::Education
    } else if has(PROJECT_HEADINGS) {
        SectionKind::Projects
    } else if has(SKILLS_HEADINGS) {
        SectionKind::Skills
    } else if has(SUMMARY_HEADINGS) {
        SectionKind::Summary
    } else {
        SectionKind::Other
    }
}

/// Legal-form suffixes: they name a corporate structure, never an employer.
/// "GmbH" appearing in a document proves nothing about which GmbH.
pub const LEGAL_FORMS: &[&str] = &[
    "gmbh",
    "corp",
    "corporation",
    "limited",
    "incorporated",
    "holding",
    "group",
    "company",
    "inc",
    "ltd",
    "llc",
    "plc",
    "ag",
    "kg",
    "se",
    "bv",
    "nv",
    "sa",
    "srl",
    "spa",
];

/// Country/region/city tokens an employer's legal name — or an entry line's
/// location column — carries. Same argument as [`LEGAL_FORMS`]: "Deutschland"
/// or "Berlin" identifies no particular employer, so neither may be the SOLE
/// reason `validate::content` decides an entry survived, and a comma-tail made
/// of nothing but these is a location rather than a company (see
/// [`split_entry`]).
pub const GEOGRAPHY_TOKENS: &[&str] = &[
    "deutschland",
    "germany",
    "österreich",
    "austria",
    "schweiz",
    "switzerland",
    "europe",
    "europa",
    "emea",
    "apac",
    "dach",
    "international",
    "global",
    "worldwide",
    "berlin",
    "münchen",
    "munich",
    "hamburg",
    "wien",
    "zürich",
];

/// The lowercased alphanumeric runs in `text`, in order.
fn word_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The tokens of `company` that name a SPECIFIC employer: alphanumeric runs of
/// `min_chars`+ characters, with legal forms and geography removed.
///
/// One helper, two questions, deliberately — they differ only in that
/// threshold. `validate::content::factual` asks "could anything in the
/// generated document evidence this employer?" and accepts two characters
/// ("SAP" is a real name); `validate::content::consistency` asks "are these two
/// entries the same employer?" and needs a distinctive four. Keeping a second
/// copy of the exclusion lists beside either question is exactly how one of
/// them silently kept "Berlin" and started matching two unrelated Berlin
/// employers to each other.
pub fn identity_tokens(company: &str, min_chars: usize) -> Vec<String> {
    word_tokens(company)
        .into_iter()
        .filter(|t| t.chars().count() >= min_chars)
        .filter(|t| !LEGAL_FORMS.contains(&t.as_str()))
        .filter(|t| !GEOGRAPHY_TOKENS.contains(&t.as_str()))
        .collect()
}

/// Whether `text` carries a corporate legal form.
fn has_legal_form(text: &str) -> bool {
    word_tokens(text)
        .iter()
        .any(|t| LEGAL_FORMS.contains(&t.as_str()))
}

/// Split the two-space form's label into `(company, title)`.
///
/// This form is the extracted-PDF shape, and its comma tail is a LOCATION far
/// more often than a company — the title usually sits on the line BELOW, which
/// is what [`extract_evidence`]'s `LineKind::JobTitle` arm exists to pick up.
/// Reusing the parenthesized form's title-first "split at the last comma" rule
/// here named the CITY as the employer, and `validate::content` then went
/// looking for that city in the generated document and raised a
/// `factual.dropped_role` Critical when a tailored résumé left it out.
///
/// Only two signals are trusted, both from lists this module already owns, and
/// anything they cannot resolve keeps the title-first reading:
///
/// * a tail made of nothing but [`GEOGRAPHY_TOKENS`] is a location, however
///   many segments it runs to (`Globex Logistics, Munich, Germany`);
/// * failing that, a legal form in the HEAD and none in the tail
///   (`Nordwind Systeme GmbH, Ingolstadt`) — an unlisted city cannot be told
///   from a company by shape, so that is the only evidence left.
fn split_two_space_label(label: &str) -> (String, String) {
    let mut label = label.trim();
    while let Some((head, tail)) = label.rsplit_once(',') {
        let tail_tokens = word_tokens(tail);
        let is_location = !tail_tokens.is_empty()
            && tail_tokens
                .iter()
                .all(|t| GEOGRAPHY_TOKENS.contains(&t.as_str()));
        if !is_location {
            break;
        }
        label = head.trim();
    }
    match label.rsplit_once(',') {
        Some((head, tail)) if has_legal_form(head) && !has_legal_form(tail) => {
            (head.trim().to_string(), String::new())
        }
        Some((title, company)) => (company.trim().to_string(), title.trim().to_string()),
        None => (label.to_string(), String::new()),
    }
}

/// Split an entry line into `(company, title, dates)`.
///
/// HEURISTIC, and deliberately a conservative one — a wrong split must never
/// invent a company the résumé does not name. Three shapes, matching the three
/// `LineKind::JobEntry` forms `export::parser` produces:
///
/// 1. Two-space form (`Acme Corp    2021 – Present`): the parser already split
///    the date into `right_text`, so `text` is the entry label, split by
///    [`split_two_space_label`]'s company-first rule.
/// 2. Pipe/middot form (`Senior Engineer | Acme Corp | 2021 – Present`): the
///    date-shaped segment is the span, the first segment is the title and the
///    second the company (the order every template in this repo renders).
/// 3. Parenthesized form (`Senior Engineer, Acme Corp (Jan 2021 – Mar 2023)`):
///    the parenthesized tail is the span, and the label splits at the LAST
///    comma.
///
/// A label with no separator becomes the company with an empty title — a
/// following `LineKind::JobTitle` line fills that in.
///
/// Public so `validate::content` decides "did this employer survive?" against
/// the same company string the evidence extractor derived. Splitting the label
/// twice, differently, is how a dropped-role check ends up comparing a job
/// TITLE against the output and concluding nothing was lost.
pub fn split_entry(line: &ParsedLine) -> (String, String, String) {
    let label_and_dates = |label: &str, dates: &str| {
        let (title, company) = match label.rsplit_once(',') {
            Some((t, c)) => (t.trim().to_string(), c.trim().to_string()),
            None => (String::new(), label.trim().to_string()),
        };
        (company, title, dates.trim().to_string())
    };

    if let Some(dates) = line.right_text.as_deref() {
        let (company, title) = split_two_space_label(&line.text);
        return (company, title, dates.trim().to_string());
    }

    let separators = ['|', '·', '•'];
    if line.text.contains(separators) {
        let segments: Vec<&str> = line.text.split(separators).map(str::trim).collect();
        let dates = segments
            .iter()
            .find(|s| looks_like_date_span(s))
            .map(|s| s.to_string())
            .unwrap_or_default();
        let rest: Vec<&str> = segments
            .iter()
            .copied()
            .filter(|s| !s.is_empty() && !looks_like_date_span(s))
            .collect();
        return match rest.as_slice() {
            [] => (String::new(), String::new(), dates),
            [only] => (only.to_string(), String::new(), dates),
            [title, company, ..] => (company.to_string(), title.to_string(), dates),
        };
    }

    match line.text.rsplit_once('(') {
        Some((label, tail)) => label_and_dates(label, tail.trim_end_matches(')')),
        None => label_and_dates(&line.text, ""),
    }
}

/// Present-tense markers a date span can end with, in the languages the résumé
/// pipeline supports.
///
/// **Always matched with [`contains_word`], never as a bare substring.** Every
/// entry here hides inside an ordinary word: `present` in "presented", `now` in
/// "knowledge", `current` in "currently", `actual` in "actually". A substring
/// comparison turns each of those into a date context and, downstream, into a
/// false `factual.unsupported_date` Critical on a truthful bullet.
pub const PRESENT_MARKERS: &[&str] = &[
    "present", "current", "now", "ongoing", "heute", "aktuell", "laufend", "actuel", "actual",
    "attuale", "heden", "atual",
];

/// Openers that make a span open-ended without naming a present-tense word:
/// `since 2021`, `seit 2021`, `from 2021`. Matched only when a year follows
/// immediately (optionally through a month), so the ordinary English
/// preposition in "cut costs from 2019 baselines" is not read as a date span.
static OPEN_ENDED_OPENER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:since|seit|from|ab|depuis|desde|dal|vanaf|sinds)\s+(?:\p{L}+\.?\s+)?(?:19|20)\d{2}\b")
        .unwrap()
});

/// True when `needle` (lowercase) occurs in `haystack` (lowercase) at word
/// boundaries on both ends — so `vital` does not fire on `revitalize` and
/// `not just` does not fire inside `cannot justify`.
///
/// One boundary rule for every lexicon-style match in the résumé pipeline:
/// `validate::content` re-exports this as `contains_phrase`, and
/// [`PRESENT_MARKERS`] is compared through it on both surfaces.
pub fn contains_word(haystack_lower: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return false;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    haystack_lower.match_indices(needle_lower).any(|(i, m)| {
        let before = haystack_lower[..i].chars().next_back();
        let after = haystack_lower[i + m.len()..].chars().next();
        before.is_none_or(|c| !is_word(c)) && after.is_none_or(|c| !is_word(c))
    })
}

/// True when `s` names a span with **no end date**.
///
/// Three shapes, because résumés spell "still there" in more than one way and a
/// validator that only knows `Present` treats every other spelling as a closed
/// span:
///
/// 1. a present-tense marker (`2021 – Present`, `2021 – Heute`), word-bounded;
/// 2. an open-ended opener with a year (`since 2021`, `seit 2021`, `from 2021`);
/// 3. a trailing dash with a year in front of it (`2021 –`).
pub fn is_open_ended(s: &str) -> bool {
    let lower = s.to_lowercase();
    if PRESENT_MARKERS.iter().any(|m| contains_word(&lower, m)) {
        return true;
    }
    if years_in(s).is_empty() {
        return false;
    }
    OPEN_ENDED_OPENER_RE.is_match(s) || lower.trim_end().ends_with(['-', '–', '—'])
}

/// True when `s` carries a year (1900–2099) or reads as an open-ended span —
/// the shape a date span has. Shared with the content validators so "what
/// counts as a date" is decided in one place.
pub fn looks_like_date_span(s: &str) -> bool {
    !years_in(s).is_empty() || is_open_ended(s)
}

/// Every 1900–2099 year in `s`, in order, deduplicated by position (not value).
///
/// Bounded to that window on purpose: a 4-digit run outside it is a quantity
/// ("processed 4500 orders"), not a date, and treating it as one is how a
/// fabricated-metric check turns into a false accusation.
pub fn years_in(s: &str) -> Vec<u32> {
    let bytes: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i - start == 4 {
            let year: u32 = bytes[start..i]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            if (1900..=2099).contains(&year) {
                out.push(year);
            }
        }
    }
    out
}

/// Function words and posting filler that are not skills, per language.
///
/// **Deliberately LOCAL to this module — do not move these into
/// `documents::keywords::STOPWORDS`.** That list feeds the scoring kernel and is
/// pinned to the match-score formula version: adding a word to it changes every
/// document's keyword set and silently invalidates every cached match score.
/// `skills_present`/`skills_absent` are a NEW, display-only surface, so the
/// filter belongs here, where it can only affect what the user reads.
///
/// `STOPWORDS` itself is English-only, which is why a German posting fills the
/// gap list with "unsere", "hinter" and "bereits" instead of skills. Entries are
/// the *unstemmed, normalized* forms (what [`display_forms`] yields), because
/// the split happens on stems when the languages align.
const FUNCTION_WORDS_DE: &[&str] = &[
    // Determiners, pronouns and prepositions that survive the ≥4-char filter.
    "aber",
    "auch",
    "beim",
    "dabei",
    "damit",
    "dann",
    "dass",
    "dein",
    "deine",
    "diese",
    "diesem",
    "diesen",
    "dieser",
    "dieses",
    "durch",
    "eine",
    "einem",
    "einen",
    "einer",
    "eines",
    // Four BYTES, so the kernel's `w.len() > 3` filter never drops it however
    // short it reads — and it is the commonest preposition in the language.
    "für",
    "hinter",
    "ihre",
    "ihrem",
    "ihren",
    "ihnen",
    "jede",
    "jeden",
    "mehr",
    "nach",
    "noch",
    "oder",
    "ohne",
    "schon",
    "sehr",
    "selbst",
    "sowie",
    "unser",
    "unsere",
    "unserem",
    "unseren",
    "unter",
    "wenn",
    "zwischen", // Copulas and modals.
    "haben",
    "hast",
    "hatte",
    "kann",
    "können",
    "muss",
    "müssen",
    "sein",
    "seine",
    "sind",
    "sollte",
    "sollten",
    "werden",
    "wird",
    "wurde",
    "wurden", // Posting filler — the German
    // half of the English filler `STOPWORDS` already drops ("experience",
    // "skills", "requirements", …).
    "anforderungen",
    "aufgaben",
    "bereits",
    "bieten",
    "erfahrung",
    "erfahrene",
    "erfahrungen",
    "gerne",
    "gute",
    "guten",
    "idealerweise",
    "kenntnisse",
    "profil",
    "suchen",
    // "Verantwortlich für …" opens half the bullets in a German résumé. It
    // names no skill, and counted as a keyword it read as stuffing.
    "verantwortlich",
    "voraussetzungen",
    "wünschenswert",
];

/// The function-word list for `lang`, empty for languages with no list yet
/// (English is already covered by the kernel's own `STOPWORDS`).
///
/// `pub(crate)` for `validate::content::ats`, whose keyword-density check counts
/// tokens through the same English-only `STOPWORDS` plus a BYTE-length filter —
/// so `durch`, `werden` and `wurde` all counted as keywords and ordinary German
/// prose was accused of stuffing. One list, both surfaces: a word that is not a
/// skill in the gap list is not a stuffed keyword either.
pub(crate) fn function_words(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => FUNCTION_WORDS_DE,
        _ => &[],
    }
}

/// Attach one experience line to the entry above it, opening an UNATTRIBUTED
/// bucket when no entry has parsed yet.
///
/// A résumé whose entry lines the parser does not recognise as `JobEntry` — no
/// pipe form, no two-space date column, no parenthesized span, e.g. a plain
/// "Acme Payments — Senior Backend Engineer" with the dates on the next line —
/// used to lose its ENTIRE experience section here: `checked_sub` on an empty
/// `roles` discarded every bullet silently, and the prompt was then told the
/// candidate had no experience to draw on.
///
/// The bucket carries EMPTY `company`/`title`/`dates` rather than a guessed
/// employer: inventing a company name is the one thing this module may never
/// do, and every consumer either flattens `roles[].bullets` (the agent's
/// evidence tool) or reads the fields it does have. An empty company reads as
/// "unattributed" to per-company logic instead of colliding with a real one.
fn attach_to_role(roles: &mut Vec<EvidenceRole>, vocab: &JobVocabulary, text: &str) {
    if roles.is_empty() {
        roles.push(EvidenceRole {
            company: String::new(),
            title: String::new(),
            dates: String::new(),
            bullets: Vec::new(),
        });
    }
    let role_idx = roles.len() - 1;
    let bullet_idx = roles[role_idx].bullets.len();
    let bullet = vocab.bullet(format!("r{role_idx}b{bullet_idx}"), text);
    roles[role_idx].bullets.push(bullet);
}

/// Structure the source résumé into the evidence a generation prompt is allowed
/// to draw on, scored against `job_text`.
///
/// Section membership drives everything: bullets under an experience heading
/// attach to the entry above them, bullets under a projects heading become
/// `projects`, and content lines under an education heading become `education`.
/// `skills_present`/`skills_absent` are the posting's own keywords split by
/// whether the résumé evidences them — the same split `keyword_coverage`
/// reports, in readable (unstemmed) form.
pub fn extract_evidence(source_resume: &str, job_text: &str) -> EvidenceSet {
    let span = Span::begin("evidence", "op=extract");
    let vocab = JobVocabulary::new(source_resume, job_text);
    let parsed = parse_resume(source_resume);

    let mut set = EvidenceSet::default();
    let mut section = SectionKind::Other;

    for line in &parsed.lines {
        match line.kind {
            LineKind::SectionHeader => section = classify_section(&line.text),
            LineKind::JobEntry if section == SectionKind::Experience => {
                let (company, title, dates) = split_entry(line);
                set.roles.push(EvidenceRole {
                    company,
                    title,
                    dates,
                    bullets: Vec::new(),
                });
            }
            // A short line right after an entry names the role the entry's
            // label left out.
            LineKind::JobTitle if section == SectionKind::Experience => {
                if let Some(role) = set.roles.last_mut() {
                    if role.title.is_empty() {
                        role.title = line.text.clone();
                    }
                }
            }
            LineKind::Bullet => match section {
                SectionKind::Experience => attach_to_role(&mut set.roles, &vocab, &line.text),
                SectionKind::Projects => {
                    let bullet = vocab.bullet(format!("p{}", set.projects.len()), &line.text);
                    set.projects.push(bullet);
                }
                SectionKind::Education => set.education.push(line.text.clone()),
                _ => {}
            },
            // A content line under Experience the parser recognised as none of
            // the above — the same "never discard the section" rule as the
            // Projects arm below, and the other half of the orphan-bullet fix
            // in [`attach_to_role`]: when the entry line itself did not parse,
            // dropping it too would lose the employer's name as well as the
            // bullets under it.
            LineKind::Text
                if section == SectionKind::Experience && !line.text.trim().is_empty() =>
            {
                attach_to_role(&mut set.roles, &vocab, &line.text)
            }
            // `Contact` belongs here: `export::parser` classifies any line
            // carrying a phone-shaped digit run as Contact, and a degree line
            // with a date span ("BSc Computer Science, TU Berlin, 2014 - 2018")
            // satisfies `PHONE_RE`. Without this arm the only education entries
            // that survived were the ones with no dates on them.
            LineKind::Text | LineKind::JobEntry | LineKind::Contact
                if section == SectionKind::Education && !line.text.trim().is_empty() =>
            {
                set.education.push(line.text.clone())
            }
            // `Contact` belongs here for the same reason it belongs on the
            // Education arm above: `export::parser` classifies any line carrying
            // a `github.com`/`portfolio` URL — or two `·` separators — as
            // Contact, which is precisely the owner-locked projects format
            // ("**Ledger CLI** · site · repo", then a "Rust · SQLite" stack
            // line). Without it, the only project lines that survived were the
            // bulleted ones and the prose description, so a prompt built from
            // this set was told the project had no link and no stack.
            LineKind::Text | LineKind::JobEntry | LineKind::Contact
                if section == SectionKind::Projects && !line.text.trim().is_empty() =>
            {
                let bullet = vocab.bullet(format!("p{}", set.projects.len()), &line.text);
                set.projects.push(bullet);
            }
            _ => {}
        }
    }

    let resume_tokens = vocab.tokens(source_resume);
    // Filter BEFORE the split, on the readable form, so a function word cannot
    // land on either side: "unsere" reported as a missing SKILL is worse than
    // useless, it makes the honest gap list look broken.
    let stop = function_words(vocab.lang);
    let skill_like = |token: &&String| !stop.contains(&vocab.readable(token).as_str());
    set.skills_present = vocab
        .keywords
        .intersection(&resume_tokens)
        .filter(skill_like)
        .map(|t| vocab.readable(t))
        .collect();
    set.skills_absent = vocab
        .keywords
        .difference(&resume_tokens)
        .filter(skill_like)
        .map(|t| vocab.readable(t))
        .collect();
    set.skills_present.sort();
    set.skills_absent.sort();

    // Codes and counts only — never résumé or posting text (ADR-027).
    span.end_with(
        &format!(
            "roles={} projects={} skills_present={} skills_absent={}",
            set.roles.len(),
            set.projects.len(),
            set.skills_present.len(),
            set.skills_absent.len()
        ),
        true,
    );
    set
}

#[cfg(test)]
mod test {
    use super::*;

    const RESUME: &str = "\
EXPERIENCE

Senior Engineer, Acme
- Built and shipped Docker containers onto a Kubernetes cluster
- Organised the team offsite and the summer party for forty people
- Ran the weekly standup
";

    /// The whole point: a bullet the posting never mentions must rank BELOW one
    /// full of the posting's vocabulary, so the weakest-first list is cuttable
    /// from the top.
    #[test]
    fn irrelevant_bullets_rank_below_keyword_bearing_ones() {
        let job = "We need a backend engineer with strong Docker and Kubernetes experience \
                   to own our container platform.";
        let ranked = rank_bullets(RESUME, job);

        assert_eq!(ranked.len(), 3, "all three bullets are candidates");
        assert!(
            ranked[0].text.contains("offsite"),
            "the offsite bullet carries none of the posting's vocabulary and must rank first \
             for cutting; got {:?}",
            ranked[0].text
        );
        let docker = ranked
            .iter()
            .position(|c| c.text.contains("Docker"))
            .expect("the Docker bullet must be present");
        assert_eq!(
            docker, 2,
            "the Docker bullet is the strongest — cut it last"
        );
        assert!(ranked[docker].score > 0.0);
        // Hits are surfaced unstemmed — "kubernetes", never the stem "kubernet".
        assert!(
            ranked[docker].hits.iter().any(|h| h == "kubernetes"),
            "hits must be readable display forms, not Snowball stems; got {:?}",
            ranked[docker].hits
        );
    }

    /// Ids are assigned in DOCUMENT order, before the weakest-first sort, so
    /// `b0` still names the first bullet after ranking reorders the list.
    #[test]
    fn ids_follow_document_order_not_rank_order() {
        let job = "Docker and Kubernetes platform engineering.";
        let ranked = rank_bullets(RESUME, job);
        let docker = ranked
            .iter()
            .find(|c| c.text.contains("Docker"))
            .expect("the Docker bullet must be present");
        assert_eq!(
            docker.id, "b0",
            "the Docker bullet is the document's FIRST bullet, so its id is b0 \
             even though it ranks last; got {:?}",
            docker.id
        );
        let ids: HashSet<&str> = ranked.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), 3, "ids must be unique; got {ids:?}");
    }

    /// Ties break on length: two equally worthless bullets, the longer one frees
    /// more space, so it is offered first.
    #[test]
    fn equally_weak_bullets_are_ordered_longest_first() {
        let job = "Docker and Kubernetes platform engineering.";
        let ranked = rank_bullets(RESUME, job);
        let zeroes: Vec<&EvidenceBullet> = ranked.iter().filter(|c| c.score == 0.0).collect();
        assert!(
            zeroes.len() >= 2,
            "expected at least two zero-scoring lines"
        );
        assert!(
            zeroes[0].text.len() >= zeroes[1].text.len(),
            "equal-score lines must be ordered longest-first; got {:?} before {:?}",
            zeroes[0].text,
            zeroes[1].text
        );
    }

    /// `SHORT_TECH_TERMS` bypass stemming (aws → aw would be corruption). The
    /// panel must surface them intact, same as the match score does.
    #[test]
    fn short_tech_terms_survive_intact() {
        let resume = "EXPERIENCE\n\n- Migrated the fleet to AWS and wrote the Go services\n";
        let job = "Hiring an engineer fluent in AWS and Go.";
        let ranked = rank_bullets(resume, job);
        let hits = &ranked[0].hits;
        assert!(hits.iter().any(|h| h == "aws"), "got {hits:?}");
        assert!(hits.iter().any(|h| h == "go"), "got {hits:?}");
    }

    /// The invariant the whole panel rests on: it must never disagree with the
    /// match score for the same pair. A cross-language pair is where that breaks
    /// — `score_one` leaves BOTH sides unstemmed there, so ranking must too.
    /// Before this, ranking always stemmed with the JD stemmer and a German
    /// posting against an English résumé scored a shared tech token on one side
    /// only. Both now route through `languages_align`.
    #[test]
    fn cross_language_pair_ranks_symmetrically_like_score_one() {
        // German JD, English résumé — divergent, so neither side may be stemmed.
        let job = "Wir suchen einen erfahrenen Entwickler mit Kubernetes und Docker \
                   für unsere Container-Plattform in München.";
        let resume = "EXPERIENCE\n\n\
                      - Shipped kubernetes clusters and docker containers to production\n\
                      - Organised the team offsite and the summer party for forty people\n";

        assert!(
            !languages_align(job, detect_locale_tag(resume)),
            "fixture must actually be a divergent pair, or this test proves nothing"
        );

        let ranked = rank_bullets(resume, job);
        let tech = ranked
            .iter()
            .find(|c| c.text.contains("kubernetes"))
            .expect("the tech bullet must be a candidate");
        assert!(
            tech.score > 0.0,
            "shared tech tokens must still match across languages when both sides \
             stay unstemmed; got {:?}",
            tech.hits
        );
        assert!(
            ranked[0].text.contains("offsite"),
            "the JD-irrelevant bullet must still rank first for cutting; got {:?}",
            ranked[0].text
        );
    }

    /// A posting with nothing extractable yields no ranking rather than a
    /// ranking in which everything ties at zero — mirrors `keyword_coverage`
    /// returning `None` instead of 0%.
    #[test]
    fn keywordless_posting_yields_no_suggestions() {
        assert!(rank_bullets(RESUME, "!!! ??? ...").is_empty());
    }

    /// Only bullets are cuttable. Section headers and the name/contact block are
    /// structural — never offer them.
    #[test]
    fn only_bullets_are_candidates() {
        let job = "Docker and Kubernetes platform engineering.";
        let ranked = rank_bullets(RESUME, job);
        assert!(
            !ranked.iter().any(|c| c.text.contains("EXPERIENCE")),
            "section headers must not be offered for cutting"
        );
        assert!(
            !ranked.iter().any(|c| c.text.contains("Senior Engineer")),
            "job entries must not be offered for cutting"
        );
    }

    // ── extract_evidence ────────────────────────────────────────────────────

    const STRUCTURED: &str = "\
Jane Doe
jane@example.com | +49 30 1234567

EXPERIENCE

Senior Engineer | Acme Corp | 2021 - Present
- Shipped Docker containers onto a Kubernetes cluster
- Ran the weekly standup

Backend Developer | Globex | 2018 - 2021
- Built the billing API in Rust

PROJECTS

- Ledger CLI - a Rust tool for double-entry bookkeeping

EDUCATION

BSc Computer Science, TU Berlin
";

    #[test]
    fn evidence_groups_bullets_under_their_role() {
        let set = extract_evidence(STRUCTURED, "Docker Kubernetes Rust backend engineer");
        assert_eq!(set.roles.len(), 2, "two entries; got {:?}", set.roles);
        assert_eq!(set.roles[0].company, "Acme Corp");
        assert_eq!(set.roles[0].title, "Senior Engineer");
        assert_eq!(set.roles[0].dates, "2021 - Present");
        assert_eq!(set.roles[0].bullets.len(), 2);
        assert_eq!(set.roles[1].company, "Globex");
        assert_eq!(set.roles[1].bullets.len(), 1);
        // Ids are role-scoped and stable.
        assert_eq!(set.roles[1].bullets[0].id, "r1b0");
    }

    #[test]
    fn evidence_separates_projects_and_education_from_experience() {
        let set = extract_evidence(STRUCTURED, "Docker Kubernetes Rust backend engineer");
        assert_eq!(set.projects.len(), 1, "got {:?}", set.projects);
        assert!(set.projects[0].text.contains("Ledger CLI"));
        assert_eq!(set.projects[0].id, "p0");
        assert!(
            set.education.iter().any(|e| e.contains("TU Berlin")),
            "education content must land in `education`; got {:?}",
            set.education
        );
        assert!(
            !set.roles
                .iter()
                .any(|r| r.bullets.iter().any(|b| b.text.contains("Ledger"))),
            "a projects bullet must never attach to an experience role"
        );
    }

    /// `skills_present`/`skills_absent` partition the posting's keywords — every
    /// keyword lands on exactly one side, which is what makes the gap list
    /// honest rather than decorative.
    #[test]
    fn skills_present_and_absent_partition_the_posting_vocabulary() {
        let set = extract_evidence(STRUCTURED, "Docker Kubernetes Terraform Rust engineer");
        assert!(
            set.skills_present.iter().any(|s| s == "docker"),
            "docker is evidenced by a bullet; got {:?}",
            set.skills_present
        );
        assert!(
            set.skills_absent.iter().any(|s| s == "terraform"),
            "terraform appears nowhere in the résumé; got {:?}",
            set.skills_absent
        );
        let overlap: Vec<&String> = set
            .skills_present
            .iter()
            .filter(|s| set.skills_absent.contains(s))
            .collect();
        assert!(
            overlap.is_empty(),
            "sides must be disjoint; got {overlap:?}"
        );
        assert!(
            set.skills_present.windows(2).all(|w| w[0] <= w[1]),
            "skills_present must be sorted for deterministic output"
        );
    }

    /// A posting with no extractable keywords still yields the résumé's
    /// STRUCTURE (roles, projects, education) — only the scoring goes quiet.
    /// `rank_bullets` returns nothing there; `extract_evidence` must not, or a
    /// generation prompt would lose the candidate's evidence entirely.
    #[test]
    fn keywordless_posting_still_yields_structure() {
        let set = extract_evidence(STRUCTURED, "!!! ??? ...");
        assert_eq!(
            set.roles.len(),
            2,
            "structure survives a keywordless posting"
        );
        assert!(set.skills_present.is_empty());
        assert!(set.skills_absent.is_empty());
        assert!(
            set.roles[0].bullets.iter().all(|b| b.score == 0.0),
            "with no posting vocabulary every bullet scores zero"
        );
    }

    /// A degree line with a date span on it is the NORMAL shape, and
    /// `export::parser` classifies it `Contact` because `PHONE_RE` matches
    /// "2014 - 2018". Before that arm existed, the only education entries that
    /// reached the evidence set were the ones with no dates — so a prompt built
    /// from this set was told the candidate had an undated degree, or none.
    #[test]
    fn education_entries_with_date_spans_are_kept() {
        let resume = "EXPERIENCE\n\n\
                      Senior Engineer | Acme | 2021 - 2024\n\
                      - Shipped the ledger service\n\n\
                      EDUCATION\n\n\
                      BSc Computer Science, TU Berlin, 2014 - 2018\n\
                      MSc Distributed Systems, TU Berlin, 2018 - 2020\n";
        let set = extract_evidence(resume, "backend engineer computer science");
        assert_eq!(
            set.education.len(),
            2,
            "both degrees carry a date span and both must survive; got {:?}",
            set.education
        );
        assert!(set.education.iter().any(|e| e.contains("BSc")));
        assert!(set.education.iter().any(|e| e.contains("MSc")));
        assert!(
            !set.roles
                .iter()
                .any(|r| r.bullets.iter().any(|b| b.text.contains("BSc"))),
            "an education line must never attach to an experience role"
        );
    }

    /// The kernel's `STOPWORDS` list is English-only, so a German posting filled
    /// the honest gap list with "unsere", "hinter" and "sehr". Those are not
    /// skills, and a gap list full of them reads as broken. Filtered LOCALLY —
    /// `STOPWORDS` feeds the scoring kernel and is formula-version-pinned.
    #[test]
    fn german_function_words_never_reach_the_skills_split() {
        let job = "Wir suchen eine erfahrene Backend-Entwicklerin für unsere \
                   Container-Plattform und die Dienste hinter dem Bezahlvorgang. Du \
                   betreibst Kubernetes unter Last und schreibst Rust. Sehr gute \
                   Kenntnisse in Docker sind eine Anforderung.";
        let resume = "BERUFSERFAHRUNG\n\n\
                      Senior Backend Engineer | Acme | 2021 - Heute\n\
                      - Docker-Container auf einem Kubernetes-Cluster betrieben\n";
        let set = extract_evidence(resume, job);
        let listed: Vec<&String> = set
            .skills_present
            .iter()
            .chain(set.skills_absent.iter())
            .collect();
        for function_word in [
            "unsere", "hinter", "unter", "sehr", "gute", "eine", "suchen", "für",
        ] {
            assert!(
                !listed.iter().any(|s| s.as_str() == function_word),
                "{function_word:?} is a function word, not a skill; got {listed:?}"
            );
        }
        // The real vocabulary is untouched.
        assert!(
            set.skills_present.iter().any(|s| s == "kubernetes"),
            "got {:?}",
            set.skills_present
        );
        assert!(
            set.skills_absent.iter().any(|s| s == "rust"),
            "got {:?}",
            set.skills_absent
        );
    }

    /// The filter must stay OUT of the scored bullet path: `hits` (and therefore
    /// `score`) drives the trim panel's wire payload and its ranking, and the
    /// scoring kernel owns that vocabulary.
    #[test]
    fn the_function_word_filter_does_not_touch_bullet_scores() {
        let job = "Wir suchen eine erfahrene Entwicklerin für unsere Container-Plattform \
                   mit Kubernetes und Docker.";
        let resume = "BERUFSERFAHRUNG\n\n\
                      Senior Engineer | Acme | 2021 - Heute\n\
                      - Unsere Container-Plattform mit Kubernetes betrieben\n";
        let ranked = rank_bullets(resume, job);
        assert!(
            ranked[0].hits.iter().any(|h| h == "unsere"),
            "bullet hits come from the scoring kernel and must be left alone; got {:?}",
            ranked[0].hits
        );
    }

    #[test]
    fn open_ended_spans_are_recognised_in_every_spelling() {
        for open in [
            "2021 - Present",
            "2021 – Heute",
            "seit 2021",
            "since Jan 2021",
            "from 2019",
            "2021 –",
            "2021 -",
        ] {
            assert!(is_open_ended(open), "{open:?} has no end date");
        }
        for closed in [
            "2018 - 2021",
            "Jan 2018 to Mar 2021",
            "presented the roadmap",
            "knowledge sharing",
            "currently",
            "actually shipped it",
            "since the rewrite",
        ] {
            assert!(!is_open_ended(closed), "{closed:?} is not an open span");
        }
    }

    #[test]
    fn contains_word_respects_boundaries() {
        assert!(contains_word("this is vital work", "vital"));
        assert!(!contains_word("we revitalized the pipeline", "vital"));
        assert!(contains_word("not just faster, but cheaper", "not just"));
        assert!(!contains_word("", "vital"));
        assert!(!contains_word("anything", ""));
    }

    /// A projects entry whose links live on a non-bulleted title line is the
    /// owner-locked format's NORMAL shape, and `export::parser` classifies it
    /// `Contact` (a `github.com` URL, or two `·` separators, is all it takes).
    /// The Education arm above already had `LineKind::Contact` added for exactly
    /// this reason; the Projects arm did not, so the line naming the project and
    /// its repository was silently dropped from the evidence set — a generation
    /// prompt was told the candidate's project had no link and no stack.
    #[test]
    fn project_lines_carrying_links_are_kept_as_evidence() {
        let resume = "EXPERIENCE\n\n\
                      Senior Engineer | Acme | 2021 - 2024\n\
                      - Shipped the ledger service\n\n\
                      PROJECTS\n\n\
                      **Ledger CLI** · https://ledger.example.dev · github.com/janedoe/ledger\n\
                      Rust · SQLite · Clap\n\
                      A double-entry bookkeeping tool for freelancers.\n";
        let set = extract_evidence(resume, "rust backend engineer bookkeeping ledger");
        assert!(
            set.projects
                .iter()
                .any(|p| p.text.contains("github.com/janedoe/ledger")),
            "the project's title + link line must reach the evidence set; got {:?}",
            set.projects
        );
        assert!(
            set.projects.iter().any(|p| p.text.contains("SQLite")),
            "the `·`-separated stack line is also Contact-shaped and must survive; got {:?}",
            set.projects
        );
        // Ids stay dense and document-ordered across the newly-kept lines.
        let ids: Vec<&str> = set.projects.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["p0", "p1", "p2"], "got {ids:?}");
        assert!(
            !set.roles
                .iter()
                .any(|r| r.bullets.iter().any(|b| b.text.contains("Ledger CLI"))),
            "a projects line must never attach to an experience role"
        );
    }

    /// `classify_section` matches substrings, and "formation" hides inside
    /// "information" (as do `formación`/`formação`/`formazione` inside
    /// `información`/`informação`/`informazione`). So the "PERSONAL INFORMATION"
    /// heading that heads half the CVs in Europe classified as EDUCATION — and,
    /// with the Contact arm above, filed the candidate's phone number and email
    /// as a degree.
    #[test]
    fn personal_information_headings_are_not_education() {
        for heading in [
            "PERSONAL INFORMATION",
            "Personal Information",
            "Información personal",
            "Informação pessoal",
            "Informazioni personali",
        ] {
            assert_ne!(
                classify_section(heading),
                SectionKind::Education,
                "{heading:?} is a contact block, not education"
            );
        }
        // The real headings these stems exist for still classify.
        for heading in [
            "FORMATION",
            "Formation académique",
            "Formations",
            "Formación académica",
            "Formação acadêmica",
            "Formazione",
            "Ausbildung",
            "Weiterbildung",
            "EDUCATION",
        ] {
            assert_eq!(
                classify_section(heading),
                SectionKind::Education,
                "{heading:?} names an education section"
            );
        }
    }

    /// The same defect end to end: the contact block under a "PERSONAL
    /// INFORMATION" heading was handed to the prompt as the candidate's
    /// education.
    #[test]
    fn a_personal_information_block_is_never_extracted_as_education() {
        let resume = "Jane Doe\n\n\
                      EXPERIENCE\n\n\
                      Senior Engineer | Acme | 2021 - 2024\n\
                      - Shipped the ledger service\n\n\
                      PERSONAL INFORMATION\n\n\
                      jane.doe@example.com | +49 30 1234567\n";
        let set = extract_evidence(resume, "backend engineer ledger");
        assert!(
            set.education.is_empty(),
            "a contact block is not a degree; got {:?}",
            set.education
        );
    }

    #[test]
    fn italian_skills_heading_is_classified() {
        assert_eq!(classify_section("COMPETENZE"), SectionKind::Skills);
        assert_eq!(classify_section("Competenze tecniche"), SectionKind::Skills);
    }

    #[test]
    fn years_in_ignores_non_year_digit_runs() {
        assert_eq!(years_in("2021 - Present"), vec![2021]);
        assert_eq!(years_in("Jan 2018 to Mar 2021"), vec![2018, 2021]);
        assert!(
            years_in("processed 4500 orders").is_empty(),
            "a quantity outside 1900–2099 is not a year"
        );
        assert!(
            years_in("+49 30 1234567").is_empty(),
            "a phone number's digit runs are not years"
        );
    }

    #[test]
    fn split_entry_handles_the_two_space_form() {
        let parsed = parse_resume("EXPERIENCE\n\nAcme Corporation    2021 - Present\n");
        let entry = parsed
            .lines
            .iter()
            .find(|l| matches!(l.kind, LineKind::JobEntry))
            .expect("the two-space form must parse as a JobEntry");
        let (company, title, dates) = split_entry(entry);
        assert_eq!(company, "Acme Corporation");
        assert_eq!(title, "");
        assert_eq!(dates, "2021 - Present");
    }

    /// The two-space form is the extracted-PDF shape, where the comma tail is a
    /// LOCATION far more often than a company. Reusing the parenthesized form's
    /// title-first "split at the last comma" rule named the CITY as the
    /// employer — and `validate::content` then went looking for that city in
    /// the generated document and raised a `factual.dropped_role` Critical when
    /// a tailored résumé (reasonably) left the location out.
    #[test]
    fn two_space_form_names_the_company_not_the_city() {
        let split = |line: &str| {
            let text = format!("EXPERIENCE\n\n{line}\n");
            let parsed = parse_resume(&text);
            let entry = parsed
                .lines
                .iter()
                .find(|l| matches!(l.kind, LineKind::JobEntry))
                .unwrap_or_else(|| panic!("{line:?} must parse as a JobEntry"))
                .clone();
            split_entry(&entry)
        };

        // A known city as the tail.
        let (company, title, dates) = split("Acme GmbH, Berlin    2021 - Present");
        assert_eq!(company, "Acme GmbH");
        assert_eq!(title, "");
        assert_eq!(dates, "2021 - Present");

        // An UNLISTED city: the legal form in the head is the evidence.
        let (company, title, _) = split("Nordwind Systeme GmbH, Ingolstadt    2018 - 2021");
        assert_eq!(company, "Nordwind Systeme GmbH");
        assert_eq!(title, "");

        // City plus country.
        let (company, _, _) = split("Globex Logistics, Munich, Germany    2018 - 2021");
        assert_eq!(company, "Globex Logistics");

        // Nothing says otherwise → the title-first reading is untouched.
        let (company, title, _) = split("Senior Engineer, Acme Corp    2021 - Present");
        assert_eq!(company, "Acme Corp");
        assert_eq!(title, "Senior Engineer");
    }

    /// An experience section whose entry line does not parse as a `JobEntry`
    /// (no pipe form, no two-space date column, no parenthesized span) used to
    /// lose its ENTIRE experience section: `checked_sub` on an empty `roles`
    /// silently discarded every bullet, and the prompt was then told the
    /// candidate had no experience at all.
    #[test]
    fn experience_bullets_survive_an_unparsed_entry_line() {
        let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Acme Payments — Senior Backend Engineer
2021 to Present
- Shipped Docker containers onto a Kubernetes cluster
- Cut checkout latency with a Redis cache in front of the ledger service
";
        let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
        let texts: Vec<&str> = set
            .roles
            .iter()
            .flat_map(|r| r.bullets.iter())
            .map(|b| b.text.as_str())
            .collect();
        assert!(
            texts.iter().any(|t| t.contains("Docker")),
            "an orphan bullet under Experience must still be evidence; got {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains("Redis")),
            "every orphan bullet lands in the same bucket; got {texts:?}"
        );
        // The bucket is UNATTRIBUTED, never a company the résumé never named.
        assert!(
            set.roles
                .iter()
                .all(|r| r.company.is_empty() || !r.bullets.is_empty()),
            "no empty role may be invented; got {:?}",
            set.roles
        );
    }
}
