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
    apply_stemmer, detect_locale_tag, display_forms, keywords, keywords_normalized,
    keywords_normalized_list, languages_align, make_stemmer,
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
    /// Keyword → how many times the POSTING states it. See
    /// [`posting_weights`].
    weights: HashMap<String, usize>,
    /// The posting's own detected language tag — picks the [`function_words`]
    /// list the present/absent split is filtered with.
    lang: &'static str,
}

/// How often the posting states each of its own keywords, keyed exactly like
/// [`JobVocabulary::keywords`] — the relevance signal the skills split is
/// ordered by.
///
/// `keywords` is a `HashSet`, so term frequency is discarded by the time the
/// split runs; this recovers it from the posting text. Term frequency rather
/// than first-occurrence position because a requirements list repeats what the
/// role is actually about, while position mostly reflects where the boilerplate
/// ends — and it costs one extra walk of a text this function already tokenizes.
///
/// Both halves come from the kernel rather than being transcribed:
/// `keywords_normalized_list` is its own duplicate-preserving form (same
/// tokenizer, synonym collapse and filter as the set, so a count can never
/// disagree with membership), and the fold onto stems goes through
/// `apply_stemmer`, so the `SHORT_TECH_TERMS` bypass that keeps "aws" from
/// stemming to "aw" is applied once, where it is defined. Stemming runs per
/// DISTINCT token, the same order of work `display_forms` already does.
fn posting_weights(job_text: &str, stemmer: &Stemmer, aligned: bool) -> HashMap<String, usize> {
    let mut normalized: HashMap<String, usize> = HashMap::new();
    for token in keywords_normalized_list(job_text) {
        *normalized.entry(token).or_default() += 1;
    }
    if !aligned {
        return normalized; // The unaligned vocabulary is unstemmed too.
    }
    let mut out: HashMap<String, usize> = HashMap::new();
    for (token, n) in normalized {
        let stem = apply_stemmer(HashSet::from([token]), stemmer)
            .into_iter()
            .next()
            .unwrap_or_default();
        *out.entry(stem).or_default() += n;
    }
    out
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
        let weights = posting_weights(job_text, &stemmer, aligned);
        Self {
            aligned,
            stemmer,
            keywords,
            display,
            weights,
            lang,
        }
    }

    /// How often the posting states `token`; `0` for anything it never said.
    fn weight(&self, token: &str) -> usize {
        self.weights.get(token).copied().unwrap_or(0)
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
    "berufserfahrung",
    "arbeitserfahrung",
    // "Beruflicher Werdegang" is as standard a German experience heading as
    // "Berufserfahrung", and classifying it `Other` discarded every bullet
    // under it. A substring, like the compounds above: "Ausbildungswerdegang"
    // reaching Experience first is a far smaller error than losing the section.
    "werdegang",
    "expérience",
    "experiencia",
    "esperienza",
    "werkervaring",
    "experiência",
];

/// The bare German word for "experience", matched with [`contains_word`].
///
/// Word-bounded rather than added to the substring list above, and the
/// plural is listed for the same reason `formations` is: the rule is exact at
/// BOTH ends. As a substring `erfahrung` would subsume `berufserfahrung` and
/// `arbeitserfahrung` — and every other `…erfahrung` compound with it,
/// including ones that name no work history ("Nutzererfahrung" on a designer's
/// skills heading). Bounded, it adds only the spellings that were actually
/// missing: "Erfahrung", "Erfahrungen", "Berufliche Erfahrung".
const EXPERIENCE_HEADINGS_WORD_BOUNDED: &[&str] = &["erfahrung", "erfahrungen"];

/// Experience stems that ALSO open a summary heading, and therefore lose to
/// [`SUMMARY_HEADINGS`] on a heading that carries both.
///
/// `career` is the whole list. It names a work history on its own ("Career",
/// "Career History") and names a *summary* just as often ("Career Summary",
/// "Career Objective", "Career Profile") — and because the experience test runs
/// first, every one of those classified as Experience. That is not a cosmetic
/// mislabel: prose under an Experience heading reaches [`extract_evidence`]'s
/// role arm, so a summary paragraph became a work bullet under a role the
/// candidate never had.
///
/// Scoped to the ambiguous stem rather than reordering the whole classifier:
/// every other entry in [`EXPERIENCE_HEADINGS`] names a work history in any
/// company, so letting `profil` outrank `berufserfahrung` would trade this
/// false positive for a new one ("Berufserfahrung / Kurzprofil" style compound
/// headings).
const AMBIGUOUS_EXPERIENCE_HEADINGS: &[&str] = &["career"];
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
/// exceptions are [`EXPERIENCE_HEADINGS_WORD_BOUNDED`] and
/// [`EDUCATION_HEADINGS_WORD_BOUNDED`], whose stems are substrings of ordinary
/// words and are therefore matched with [`contains_word`].
///
/// The one place the "experience first" order is NOT applied is
/// [`AMBIGUOUS_EXPERIENCE_HEADINGS`] — a stem that opens a summary heading as
/// readily as an experience one yields to a summary word on the same heading.
pub fn classify_section(heading: &str) -> SectionKind {
    let lower = heading.to_lowercase();
    let has = |set: &[&str]| set.iter().any(|k| lower.contains(k));
    let has_word = |set: &[&str]| set.iter().any(|k| contains_word(&lower, k));
    let summary = has(SUMMARY_HEADINGS);
    if has(EXPERIENCE_HEADINGS)
        || has_word(EXPERIENCE_HEADINGS_WORD_BOUNDED)
        || (!summary && has(AMBIGUOUS_EXPERIENCE_HEADINGS))
    {
        SectionKind::Experience
    } else if has(EDUCATION_HEADINGS) || has_word(EDUCATION_HEADINGS_WORD_BOUNDED) {
        SectionKind::Education
    } else if has(PROJECT_HEADINGS) {
        SectionKind::Projects
    } else if has(SKILLS_HEADINGS) {
        SectionKind::Skills
    } else if summary {
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

/// Whether `text` **ends** with a corporate legal form.
///
/// Positional on purpose. A legal form is a SUFFIX — "Nordwind Systeme GmbH",
/// "Acme Inc", "Globex Ltd" — while [`LEGAL_FORMS`] necessarily also holds
/// `group`, `company` and `holding`, which are ordinary English title words.
/// Testing every token made the head of "Group Product Manager, Acme Payments"
/// look like a company name, so [`split_two_space_label`]'s company-first rule
/// fired and recorded the employer as "Group Product Manager" — discarding both
/// the real company and the title.
///
/// The words stay in [`LEGAL_FORMS`] because [`identity_tokens`] needs them for
/// a different question ("does this token name a SPECIFIC employer?", where
/// `group` is worthless wherever it sits). Only this split heuristic needs the
/// position.
fn ends_with_legal_form(text: &str) -> bool {
    word_tokens(text)
        .last()
        .is_some_and(|t| LEGAL_FORMS.contains(&t.as_str()))
}

/// Whether `s` is made of NOTHING but [`GEOGRAPHY_TOKENS`] — a location column
/// ("Berlin", "Munich, Germany"), never an employer.
///
/// Shared by both entry-splitting arms in [`split_entry`] on purpose. The
/// two-space arm was hardened against reading the location as the company; the
/// pipe/middot arm was not, so "Acme Corp | Berlin | 2021 – Present" recorded
/// the CITY as the employer and the employer as the job title — the same defect
/// class, one heuristic away. A company whose whole name is a listed
/// geography word ("Berlin") is dropped rather than kept: the same tradeoff
/// [`split_two_space_label`] already accepts, and inventing nothing beats
/// naming a city as an employer.
fn is_location_only(s: &str) -> bool {
    let tokens = word_tokens(s);
    !tokens.is_empty()
        && tokens
            .iter()
            .all(|t| GEOGRAPHY_TOKENS.contains(&t.as_str()))
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
/// * failing that, a TRAILING legal form in the HEAD and none at the end of the
///   tail (`Nordwind Systeme GmbH, Ingolstadt`) — an unlisted city cannot be
///   told from a company by shape, so that is the only evidence left. Trailing,
///   not anywhere: see [`ends_with_legal_form`].
fn split_two_space_label(label: &str) -> (String, String) {
    let mut label = label.trim();
    while let Some((head, tail)) = label.rsplit_once(',') {
        if !is_location_only(tail) {
            break;
        }
        label = head.trim();
    }
    match label.rsplit_once(',') {
        Some((head, tail)) if ends_with_legal_form(head) && !ends_with_legal_form(tail) => {
            (head.trim().to_string(), String::new())
        }
        Some((title, company)) => (company.trim().to_string(), title.trim().to_string()),
        None => (label.to_string(), String::new()),
    }
}

/// How many words a comma-less entry label may run to and still read as an
/// employer's NAME. Four covers "Nordwind Systeme Digital Solutions"; past that
/// a label is a sentence far more often than a company.
const MAX_COMPANY_LABEL_WORDS: usize = 4;

/// Whether `label` reads as a NAME rather than as a sentence: at most
/// [`MAX_COMPANY_LABEL_WORDS`] words, none of which STARTS lowercase.
///
/// Capitalization is the discriminator because it is the one signal that
/// separates the two shapes in both curated languages without a vocabulary.
/// English writes an employer in title case ("Acme Payments") and a sentence
/// with lowercase function words ("Led **the** platform rewrite"); German
/// capitalizes its nouns but never its articles, prepositions or verbs
/// ("Beförderung **zum** Staff Engineer"), so a prose fragment in either
/// language almost always carries a lowercase word. Digits and symbols are
/// neutral, so "3M Deutschland" and "Johnson & Johnson" both pass.
///
/// The two errors it can still make, both stated rather than hidden:
///
/// * a title-cased JOB TITLE with no company on the line ("Leitende
///   Entwicklerin, Jan 2019 - Dec 2021") is recorded as the employer;
/// * an employer written all-lowercase ("adidas, Jan 2019 - Dec 2021") stays
///   unattributed.
///
/// The second is the safe direction and the first is bounded: this value never
/// reaches a `factual.dropped_role` comparison, which reads only lines the
/// parser recognised as a `JobEntry`.
fn looks_like_a_company_name(label: &str) -> bool {
    let words: Vec<&str> = label.split_whitespace().collect();
    !words.is_empty()
        && words.len() <= MAX_COMPANY_LABEL_WORDS
        && words.iter().all(|w| {
            w.chars()
                .find(|c| c.is_alphanumeric())
                .is_none_or(|c| !c.is_lowercase())
        })
}

/// Salvage `(company, title)` from an entry line the PARSER did not recognise
/// — [`extract_evidence`]'s role-opening arm, where the label was cut out of
/// running text instead of handed over by `export::parser`. `None` means
/// nothing could be resolved, and the caller must open an UNATTRIBUTED role
/// rather than name one.
///
/// ## The comma-less arm, and why it may trust its input again
///
/// [`split_two_space_label`]'s comma-less arm reads the whole label as the
/// company. That is correct where IT is called from — the parser has already
/// peeled the date column off a template-rendered entry, so whatever remains is
/// the employer by construction — and the same construction now holds here:
/// [`trailing_date_column`] only reaches this function after [`is_date_only`]
/// has confirmed a real date COLUMN, so the label is everything in front of one.
/// "Acme Payments, Jan 2019 - Dec 2021" is an ordinary entry line whose label IS
/// the employer, and refusing to name it (R7's behaviour) cost every such entry
/// its attribution while keeping the name as bullet text.
///
/// What the gate does NOT decide is whether the label is a NAME:
/// "Led the platform rewrite, Jan 2019 - Dec 2021" passes it too. That is
/// [`looks_like_a_company_name`]'s job, and it is why this is not simply a call
/// to [`split_two_space_label`] — the promotion note and the sentence must
/// still open an unattributed role, exactly as R7 pinned them.
///
/// Text is never deleted to buy either behaviour: when this returns `None` the
/// caller keeps the label as the new role's first bullet.
fn salvage_entry_label(label: &str) -> Option<(String, String)> {
    if !label.contains(',') {
        let label = label.trim();
        return looks_like_a_company_name(label).then(|| (label.to_string(), String::new()));
    }
    let (company, title) = split_two_space_label(label);
    (!company.is_empty() || !title.is_empty()).then_some((company, title))
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
///    date segment ([`is_date_column_segment`], which needs a year — a bare
///    present-tense marker is an employer's NAME at least as often as a date)
///    is the span, the first remaining segment is the title and the second the
///    company (the order every template in this repo renders). Location-only
///    segments are dropped first ([`is_location_only`]) — an entry line carries
///    a location column at least as often as a title, and positional reading
///    alone recorded "Acme Corp | Berlin | 2021 – Present" as the company
///    "Berlin".
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
            .find(|s| is_date_column_segment(s))
            .map(|s| s.to_string())
            .unwrap_or_default();
        let rest: Vec<&str> = segments
            .iter()
            .copied()
            .filter(|s| !s.is_empty() && !is_date_column_segment(s) && !is_location_only(s))
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

/// A CLOSED span: two years with nothing between them but a span separator and
/// at most one month-shaped token (`2018 – 2021`, `Jan 2018 - Mar 2021`,
/// `2018 to 2021`, `05/2018 – 07/2021`).
///
/// The separator is what makes this a SPAN rather than two numbers that happen
/// to share a line. Same year window as [`years_in`], for the same reason it
/// lives in this module: what counts as a date is decided in one place.
static DATE_SPAN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:19|20)\d{2}\s*(?:[-–—]|\bto\b|\bbis\b|\buntil\b)\s*(?:[\p{L}\d]{1,9}[./]?\s*)?(?:19|20)\d{2}",
    )
    .unwrap()
});

/// Every closed date span in `s`, as `(start_year, end_year, span_text)` — the
/// text is what a finding may quote as evidence.
///
/// `validate::content::consistency` used to take ANY two years on a line as a
/// span and report the pair as swapped when the second was smaller. A bullet
/// that measures this year against an older baseline ("cut incidents to 2024
/// levels from the 2019 baseline") therefore read as an entry whose end date
/// preceded its start. Requiring the separator is what tells a date column
/// apart from prose; a pair with a word between the years is not a span.
pub fn date_spans(s: &str) -> Vec<(u32, u32, &str)> {
    DATE_SPAN_RE
        .find_iter(s)
        .filter_map(|m| match years_in(m.as_str())[..] {
            [start, end] => Some((start, end, m.as_str())),
            // The optional month slot can swallow a third year; a fragment that
            // does not resolve to exactly two is not a span this can judge.
            _ => None,
        })
        .collect()
}

/// Month names and abbreviations, English and German, matched WHOLE against a
/// [`word_tokens`] token.
///
/// Deliberately not a prefix list: `mar` as a prefix also matches "marketing",
/// and this list's whole job is to decide that a fragment carries nothing but a
/// date. Deliberately only the two languages the rest of this pipeline curates
/// — a French or Spanish month simply leaves [`is_date_only`] false, which
/// costs an entry line its attribution (the pre-existing behaviour) rather than
/// mis-reading a sentence as a date column.
const MONTH_TOKENS: &[&str] = &[
    "jan",
    "january",
    "januar",
    "feb",
    "february",
    "februar",
    "mar",
    "march",
    "mär",
    "märz",
    "apr",
    "april",
    "may",
    "mai",
    "jun",
    "june",
    "juni",
    "jul",
    "july",
    "juli",
    "aug",
    "august",
    "sep",
    "sept",
    "september",
    "oct",
    "october",
    "okt",
    "oktober",
    "nov",
    "november",
    "dec",
    "december",
    "dez",
    "dezember",
];

/// True when `s` is a date COLUMN and nothing else: every word in it is a
/// number, a month or a present-tense marker, **and** it carries more date
/// structure than a lone year — a span separator (`2018 – 2021`), an open end
/// (`2021 – Present`, `2021 –`) or a month (`Jan 2022`).
///
/// Both halves are load-bearing, and [`looks_like_date_span`] alone is neither.
/// It is satisfied by any text carrying a year, so the word test was doing all
/// the work — and a single BARE YEAR passes the word test too. "Promoted to
/// Staff Engineer, 2022" is an ordinary line in an experience section, and
/// reading its trailing year as a date column made [`trailing_date_column`]
/// hand the sentence in front of it to the employer salvage. A year on its own
/// is a date a line MENTIONS; a column is a date the line is STRUCTURED by.
///
/// Cost, deliberately paid: an entry line whose whole date column is one bare
/// year ("Acme Corp, 2022") no longer opens a role, so its bullets continue the
/// entry above it. That is the pre-existing behaviour for every unrecognised
/// line, and it invents nothing.
fn is_date_only(s: &str) -> bool {
    let tokens = word_tokens(s);
    let date_words = tokens.iter().all(|t| {
        t.chars().all(|c| c.is_ascii_digit())
            || MONTH_TOKENS.contains(&t.as_str())
            || PRESENT_MARKERS.contains(&t.as_str())
    });
    if !date_words || !looks_like_date_span(s) {
        return false;
    }
    !date_spans(s).is_empty()
        || is_open_ended(s)
        || tokens.iter().any(|t| MONTH_TOKENS.contains(&t.as_str()))
}

/// True when one pipe/middot SEGMENT is the entry's date column: it carries a
/// YEAR.
///
/// Stricter than [`looks_like_date_span`] in exactly one dimension, and that is
/// the point. `looks_like_date_span` is also satisfied by a bare present-tense
/// marker with no year anywhere ([`is_open_ended`] fires on the word alone), and
/// [`PRESENT_MARKERS`] is a list of ordinary words that real employers are named
/// after — Current (current.com) and Current Health are both real, and "Aktuell"
/// opens plenty of German company names. Such a segment was selected as the date
/// column AND filtered out of the label segments, so the job TITLE was recorded
/// as the employer and the employer as the date span.
///
/// Deliberately NOT [`is_date_only`], which the review suggested: it is both too
/// loose and too tight here. Too loose because a bare "Current" satisfies it
/// (every word is a present marker, and `is_open_ended` supplies the structure),
/// which is the failing case itself; too tight because it rejects a lone year,
/// and this arm has always read `Senior Engineer | Acme Corp | 2022` as an entry
/// with a one-year column. Its word test would additionally reject spellings the
/// PARSER accepts and hands us — "2018 to 2021", "2021 bis Heute",
/// "Jan 2018 through Mar 2021" all carry a word that is neither month, number
/// nor marker — turning a fixed false employer into a lost date column.
///
/// The residual is the pre-existing one, unchanged: a label that happens to
/// carry a year ("2020 Ventures") still reads as the date column. Telling that
/// apart needs the word test this rejects.
fn is_date_column_segment(s: &str) -> bool {
    !years_in(s).is_empty()
}

/// Split `text` into `(label, dates)` when it ends in a `, <dates>` column —
/// the shape of an entry line the parser did not recognise as a `JobEntry`
/// ("Acme Payments, Berlin, 2018 - 2021").
///
/// `None` for anything else, including a line that merely mentions a year: the
/// tail must be [`is_date_only`], or an ordinary sentence ending in
/// "…, delivered in 2019" would read as an employer plus a date column.
fn trailing_date_column(text: &str) -> Option<(&str, &str)> {
    let (label, dates) = text.rsplit_once(',')?;
    let label = label.trim();
    (!label.is_empty() && is_date_only(dates)).then_some((label, dates.trim()))
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
///
/// **An empty list means "no filter", not "nothing to filter"** — see
/// [`has_curated_function_words`], which is what a caller must ask before
/// drawing a conclusion from a count.
pub(crate) fn function_words(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => FUNCTION_WORDS_DE,
        _ => &[],
    }
}

/// Whether `lang`'s function words are actually known to this crate.
///
/// English counts: the kernel's own `STOPWORDS` is its curation, which is why
/// [`function_words`] returns an empty slice for `en` without that meaning
/// "unfiltered". Every other language returns an empty slice because nobody has
/// written the list yet — the two cases are indistinguishable at the call site,
/// and that is the whole point of this helper.
///
/// The rule it exists to enforce, the same one the rest of this module lives by:
/// **never accuse without evidence.** A ratio or a ceiling computed over
/// unfiltered French, Spanish, Italian, Dutch or Portuguese prose counts `pour`,
/// `para`, `nella`, `worden` and `para` as keywords, so ordinary writing reads as
/// stuffing. A caller whose conclusion depends on function words having been
/// removed must go quiet here rather than report a number it cannot stand
/// behind. Adding a language to [`function_words`] is what re-enables it —
/// deliberately one edit, in one place.
pub(crate) fn has_curated_function_words(lang: &str) -> bool {
    matches!(lang, "en" | "de")
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
    // Bullets under a heading no list recognises, held back for the last-resort
    // rescue after the loop — see there for when they are used.
    let mut unclassified_bullets: Vec<String> = Vec::new();

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
                SectionKind::Other => unclassified_bullets.push(line.text.clone()),
                // Skills and Summary bullets are classified and belong where
                // they are: a summary line is a claim about the candidate, not
                // an achievement to draw on.
                _ => {}
            },
            // A content line under Experience the parser recognised as none of
            // the above — the same "never discard the section" rule as the
            // Projects arm below, and the other half of the orphan-bullet fix
            // in [`attach_to_role`]: when the entry line itself did not parse,
            // dropping it too would lose the employer's name as well as the
            // bullets under it.
            //
            // `Contact` belongs here for exactly the reason it belongs on the
            // Education and Projects arms — and it is not an edge case, it is
            // the SHAPE this arm was written for. "Acme Payments, Berlin,
            // 2018 - 2021" is contact-shaped: `PHONE_RE` reads the date span as
            // a phone number. Without it the fix above rescued the bullets and
            // still lost the employer they belong to, which is the worse half of
            // the original bug (evidence with no attribution).
            //
            // A line that ends in a date COLUMN opens its own role, exactly as
            // a recognised `JobEntry` would. Appending it to `roles.last()` —
            // which is all [`attach_to_role`] can do — made the previous
            // employer absorb this one's header line AND every bullet under it,
            // so a second employer's work was credited to the first.
            //
            // Both gates are conservative on purpose, because the input is
            // ordinary text and the output is an employer's name:
            // [`trailing_date_column`] takes a date column, not a mentioned
            // year, and [`salvage_entry_label`] resolves an employer or returns
            // nothing. Unresolved, the role still opens (the column says an
            // entry started) but stays UNATTRIBUTED, and the label is kept as
            // its first bullet so refusing to name an employer never deletes
            // the line — the same shape the R5-F6 rescue already gives an
            // entry line it cannot attribute.
            LineKind::Text | LineKind::Contact
                if section == SectionKind::Experience && !line.text.trim().is_empty() =>
            {
                match trailing_date_column(&line.text) {
                    Some((label, dates)) => {
                        let (company, title) = salvage_entry_label(label).unwrap_or_default();
                        let unattributed = company.is_empty() && title.is_empty();
                        set.roles.push(EvidenceRole {
                            company,
                            title,
                            dates: dates.to_string(),
                            bullets: Vec::new(),
                        });
                        if unattributed {
                            attach_to_role(&mut set.roles, &vocab, label);
                        }
                    }
                    None => attach_to_role(&mut set.roles, &vocab, &line.text),
                }
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

    // Last resort: a document with NO recognised experience section falls back
    // to the bullets under its unclassified headings.
    //
    // A heading this module cannot name is not evidence that the candidate has
    // none — `classify_section` is a fixed list of stems in seven languages, and
    // every gap in it (this round's "Beruflicher Werdegang", the next one's
    // whatever) silently emptied the set the generation prompt is allowed to
    // draw on. Gated on `roles.is_empty()` because a heading LIST is a better
    // signal than a fallback whenever there is one: a résumé with a real
    // experience section keeps its hobbies and interests out of its work
    // history. The bucket is unattributed — no employer is ever guessed — and
    // the cost when the unknown heading was really "PUBLICATIONS" is that a
    // prompt sees the candidate's own publication list as evidence, which is
    // strictly better than seeing nothing at all.
    if set.roles.is_empty() {
        for text in unclassified_bullets {
            attach_to_role(&mut set.roles, &vocab, &text);
        }
    }

    let resume_tokens = vocab.tokens(source_resume);
    // Filter BEFORE the split, on the readable form, so a function word cannot
    // land on either side: "unsere" reported as a missing SKILL is worse than
    // useless, it makes the honest gap list look broken.
    let stop = function_words(vocab.lang);
    let skill_like = |token: &&String| !stop.contains(&vocab.readable(token).as_str());
    // …but that filter only EXISTS for a curated language, and an empty slice
    // cannot say so — see [`has_curated_function_words`].
    let curated = has_curated_function_words(vocab.lang);
    // Ordered by how often the POSTING states the term, alphabetically within a
    // tie. Both lists are truncated by their consumers
    // (`agent::tools_quality::compact_evidence_set` takes the first N and
    // reports only a dropped COUNT), so a purely alphabetical order silently
    // handed a prompt the alphabetical PREFIX of the gap list — "ansible" kept,
    // "terraform" cut, and nothing downstream able to tell. Relevance-first
    // makes a truncated list the top-N by construction, and the consumer needs
    // no change.
    //
    // Determinism is unchanged, which is what the alphabetical sort was for:
    // the tiebreak is a total order, because [`display_forms`] maps each stem to
    // a token that stems back to it, so two distinct keywords cannot share one
    // display form.
    //
    // **The relevance key is switched off for an uncurated language**, which is
    // where it measures the opposite of what it claims: with no filter, the
    // terms a posting repeats most are its FILLERS ("pour" ×4, "avec" ×3), so
    // frequency sorted them to the top of the truncated GAP LIST a generation
    // prompt works from — round 8's ordering made `fr`/`es`/`it`/`nl`/`pt`
    // worse, not better. The honest degrade is to make no relevance claim at
    // all and fall back to the deterministic tiebreak.
    //
    // Emptying the lists instead was rejected on the consumer's terms: nothing
    // in `EvidenceSet` can say "unmeasured", so an empty `skills_absent` reads
    // as "no gaps" — a positive claim this module cannot support — and `lang`
    // is DETECTED, so a terse posting `whatlang` misreads would silently delete
    // an ordinary English gap list. Demoting degrades; deleting lies.
    //
    // *Residual, measured rather than assumed:* the fillers are still IN the
    // list and still consume slots, so a real requirement past the consumer's
    // cap can stay cut in BOTH orders. This stops the list ASSERTING that
    // fillers are the priorities; it does not clean the list. A bullet's `hits`
    // are unfiltered too. Only a curated `function_words` list fixes either —
    // one edit, which re-enables the relevance order with it.
    let by_relevance = |tokens: Vec<&String>| -> Vec<String> {
        let mut scored: Vec<(usize, String)> = tokens
            .into_iter()
            .map(|token| {
                let weight = if curated { vocab.weight(token) } else { 0 };
                (weight, vocab.readable(token))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, display)| display).collect()
    };
    set.skills_present = by_relevance(
        vocab
            .keywords
            .intersection(&resume_tokens)
            .filter(skill_like)
            .collect(),
    );
    set.skills_absent = by_relevance(
        vocab
            .keywords
            .difference(&resume_tokens)
            .filter(skill_like)
            .collect(),
    );

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
mod test;
