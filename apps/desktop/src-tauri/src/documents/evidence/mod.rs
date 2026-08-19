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

use rust_stemmers::Stemmer;
use serde::Serialize;

use crate::documents::keywords::{
    apply_stemmer, detect_locale_tag, display_forms, keywords, keywords_normalized,
    keywords_normalized_list, languages_align, make_stemmer,
};
use crate::export::parser::parse_resume;
use crate::export::types::LineKind;
use crate::observability::Span;

mod entry;

use self::entry::salvage_entry_label;

/// The entry/label/date shape family, re-exported so every existing
/// `documents::evidence::…` path keeps resolving after the split.
pub use self::entry::{
    contains_word, date_spans, identity_tokens, is_open_ended, looks_like_date_span, split_entry,
    trailing_date_column, years_in, GEOGRAPHY_TOKENS, LEGAL_FORMS, PRESENT_MARKERS,
};

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

/// Heading fragments that name a WORK HISTORY unconditionally, matched as a
/// substring of the lowercased heading so "Berufserfahrung" and "Beruflicher
/// Werdegang" both land correctly.
/// en/de/fr/es/it/nl/pt — the languages `make_stemmer` already supports.
///
/// Every entry here says *work* ("employment", "Beruf…", "Arbeit…",
/// "…professionnelle", `werdegang` = career path, `werkervaring` = work
/// experience), which is what makes them unconditional: no other section word
/// on the same heading can outrank them. The BARE word for "experience" is not
/// here — it is ambiguous, see [`AMBIGUOUS_EXPERIENCE_HEADINGS`].
const EXPERIENCE_HEADINGS: &[&str] = &[
    "employment",
    "work experience",
    "professional experience",
    "berufserfahrung",
    "berufliche erfahrung",
    "arbeitserfahrung",
    // "Beruflicher Werdegang" is as standard a German experience heading as
    // "Berufserfahrung", and classifying it `Other` discarded every bullet
    // under it. A substring, like the compounds above: "Ausbildungswerdegang"
    // reaching Experience first is a far smaller error than losing the section.
    "werdegang",
    "expérience professionnelle",
    // Work-qualified, no second reading, no substring risk — same profile as
    // the other terms this change teaches.
    "parcours professionnel",
    "experiencia profesional",
    "esperienza professionale",
    // Italian plurals, WORK-QUALIFIED and unconditional — deliberately not the
    // bare `esperienze`, which a review proved re-creates this very commit's
    // bug one language over: "Esperienze di formazione" is an EDUCATION
    // heading, and the ambiguous set yields only to summary/skills, never to
    // education, so a bare stem there resolves Experience and files a degree
    // as a job. Qualified spellings also stay out of the section-DELETING
    // hole the ambiguous list's own doc warns about: "Esperienze
    // professionali e competenze" reaches Experience here, where the bare
    // stem would have lost it to Skills.
    "esperienze professionali",
    "esperienze lavorative",
    // Sweep finds: German "Praxiserfahrung" is not caught by the
    // word-bounded `erfahrung` (it is a compound, not a separate word), and
    // Dutch "Werkervaring" already is. Both name a work history outright.
    "praxiserfahrung",
    "werkervaring",
    "experiência profissional",
];

/// Headings that contain an EXPERIENCE stem but unambiguously name an
/// EDUCATION section, checked BEFORE the experience test so the substring
/// cannot win.
///
/// Found by sweeping every term in the TS `SECTION_LEXICON` through this
/// classifier: "Akademischer Werdegang" — a standard German heading for an
/// academic record — contains `werdegang` and so classified as Experience.
/// That is the expensive direction, not a cosmetic mislabel: prose under an
/// Experience heading reaches [`extract_evidence`]'s role arm, so degree
/// entries became work bullets under roles the candidate never held.
///
/// A closed list of exact phrases rather than a rule, deliberately. The
/// general fix — letting [`AMBIGUOUS_EXPERIENCE_HEADINGS`] yield to education
/// the way it yields to summary and skills — would overturn a documented
/// decision (see that const: Education and Projects are excluded from the
/// yield set on purpose, because "Project Experience" really is a work
/// history in a consultant's CV). Every phrase here has an explicit education
/// qualifier and so has no such second reading, which is what lets membership
/// stand in for a rule. The cost is that membership must be kept up: a review
/// found four more already in the wild after the first three were written, so
/// every entry is asserted individually below rather than sampled.
///
/// This also supersedes one line of [`EXPERIENCE_HEADINGS`]'s `werdegang`
/// note, which accepted "Ausbildungswerdegang" landing on Experience as
/// cheaper than losing the section. It now lands on Education, which is
/// better than either.
const EDUCATION_OVERRIDES_EXPERIENCE: &[&str] = &[
    "akademischer werdegang",
    // German declines and compounds `werdegang` freely, so an exact-phrase
    // list is only as good as its membership. A review found the next four
    // already in the wild — "Wissenschaftlicher Werdegang" heads the academic
    // CV this feature is for — each of them filing a degree as a job on main
    // and on the first draft of this fix alike.
    "akademischen werdegang",
    "wissenschaftlicher werdegang",
    "wissenschaftlichen werdegang",
    "schulischer werdegang",
    "bildungswerdegang",
    "akademische laufbahn",
    "ausbildungswerdegang",
];

/// The bare German word for "experience", matched with [`contains_word`].
///
/// Word-bounded rather than a substring, and the plural is listed for the same
/// reason `formations` is: the rule is exact at BOTH ends. As a substring
/// `erfahrung` would subsume `berufserfahrung` and `arbeitserfahrung` — and
/// every other `…erfahrung` compound with it, including ones that name no work
/// history ("Nutzererfahrung" on a designer's skills heading). Bounded, it adds
/// only the spellings that were actually missing: "Erfahrung", "Erfahrungen",
/// "Berufliche Erfahrung".
///
/// Ambiguous in exactly the way [`AMBIGUOUS_EXPERIENCE_HEADINGS`] is, and read
/// under the same rule — it is a separate const only because it needs the
/// word-boundary matcher.
const EXPERIENCE_HEADINGS_WORD_BOUNDED: &[&str] = &["erfahrung", "erfahrungen"];

/// Experience stems that ALSO open a SUMMARY or a SKILLS heading, and therefore
/// lose to [`SUMMARY_HEADINGS`]/[`SKILLS_HEADINGS`] on a heading that carries
/// both.
///
/// `career` names a work history on its own ("Career", "Career History") and
/// names a *summary* just as often ("Career Summary", "Career Objective",
/// "Career Profile"). The bare word for "experience" is ambiguous the same way
/// against SKILLS: "Skills and Experience", "Technical Skills & Experience" and
/// "Kenntnisse und Erfahrungen" head a skills MATRIX in a real résumé, not a
/// list of employers. Because the experience test runs first, every one of
/// those classified as Experience — and that is not a cosmetic mislabel: prose
/// under an Experience heading reaches [`extract_evidence`]'s role arm, so a
/// summary paragraph or a skills line became a work bullet under a role the
/// candidate never had.
///
/// **The rule is scoped to these stems, not applied to the whole classifier,
/// because the two mistakes do not cost the same.** Skills filed as experience
/// invents a role — noisy, recoverable, visible. Experience filed as skills
/// DELETES the section: nothing in [`extract_evidence`] reads a Skills section
/// (no role arm, no bullet arm, and the last-resort rescue covers `Other`
/// only), so a work history under a skills-word heading would reach the
/// generation prompt as an empty evidence set. Keeping every work-qualified
/// spelling in [`EXPERIENCE_HEADINGS`] means "Berufserfahrung und Kenntnisse"
/// and "Work Experience and Skills" cannot fall into that hole, while the
/// ambiguous stems take the cheaper error.
///
/// Only SUMMARY and SKILLS are in the yield set. Education and Projects are
/// deliberately out: "Project Experience" is a work history in a consultant's
/// CV as often as it is a projects section, and neither reading has been
/// observed to cost anything yet.
const AMBIGUOUS_EXPERIENCE_HEADINGS: &[&str] = &[
    "career",
    "experience",
    "expérience",
    "experiencia",
    "esperienza",
    "experiência",
];
const EDUCATION_HEADINGS: &[&str] = &[
    "education",
    "academic",
    "ausbildung",
    "bildung",
    // Sweep find. German "Studium" is the ordinary word for an education
    // section and matched nothing. Safe as a substring: "Auslandsstudium",
    // "Selbststudium" and "Studium Generale" are all education headings.
    "studium",
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
    // Italian "Istruzione" — the very heading `resume_conventions("it")`
    // teaches the model to write, and it matched nothing. WORD-BOUNDED for
    // the same reason `formation` below is: `distruzione` contains
    // `istruzione` exactly as `information` contains `formation`.
    "istruzione",
    "istruzioni",
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
    // Portuguese "Competências" — the `ê` blocks `competenc` exactly as the
    // `z` blocked it for Italian above. It is the pt Skills header the résumé
    // prompt itself emits, so without this a Portuguese résumé generated by
    // this app classifies its own skills section as `Other`.
    "competênc",
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
/// [`AMBIGUOUS_EXPERIENCE_HEADINGS`] (plus the word-bounded German twins) — a
/// stem that opens a summary or a skills heading as readily as an experience
/// one yields to a summary/skills word on the same heading. See that const for
/// why the yield is scoped to those stems rather than reordering the classifier.
pub fn classify_section(heading: &str) -> SectionKind {
    let lower = heading.to_lowercase();
    let has = |set: &[&str]| set.iter().any(|k| lower.contains(k));
    let has_word = |set: &[&str]| set.iter().any(|k| contains_word(&lower, k));
    let summary = has(SUMMARY_HEADINGS);
    let skills = has(SKILLS_HEADINGS);
    let ambiguous_experience =
        has(AMBIGUOUS_EXPERIENCE_HEADINGS) || has_word(EXPERIENCE_HEADINGS_WORD_BOUNDED);
    // Before the experience test, not after: these carry an experience
    // SUBSTRING, so anything downstream of that test is unreachable for them.
    if has(EDUCATION_OVERRIDES_EXPERIENCE) {
        SectionKind::Education
    } else if has(EXPERIENCE_HEADINGS) || (ambiguous_experience && !summary && !skills) {
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
///
/// **Not the same primitive as `validate::content::language`'s
/// `FUNCTION_WORDS_DE`/`function_words_for`, despite the identical name and
/// overlapping content — do not "unify" them.** That module asks a language-
/// IDENTITY question (does this text carry positive evidence of being
/// written in a specific OTHER language, so a confident-but-wrong `whatlang`
/// read is not this crate's only witness) and answers it for seven curated
/// languages; this one asks a SKILL-CLAIM question (which tokens on a skills
/// line are filler rather than a claimed skill) and, as the doc above
/// explains, is deliberately curated for German only. A word missing here is
/// safe (a display-only gap list stays merely noisy); a word wrongly
/// PRESENT there is not (it manufactures a language accusation) — the two
/// lists answer to different correctness bars for that reason, and merging
/// them would let a change tuned for one silently regress the other.
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
///
/// `false` for `"es"` here does NOT mean this crate has no Spanish function
/// words anywhere — `validate::content::language::function_words_for("es")`
/// returns 61 of them, curated for a DIFFERENT question (see
/// [`FUNCTION_WORDS_DE`]'s doc for why the two are deliberately separate
/// primitives, not the same list read from two places).
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
    // tie. Both lists are sized for a future truncating consumer — none reads
    // them today (`pipeline::resume::stages::strategy` calls `extract_evidence`
    // for `.roles` only); the now-deleted `agent::tools_quality::
    // compact_evidence_set` took the first N and reported only a dropped COUNT,
    // and is why a purely alphabetical order would silently hand a truncating
    // consumer the alphabetical PREFIX of the gap list — "ansible" kept,
    // "terraform" cut, and nothing downstream able to tell. Relevance-first
    // makes a truncated list the top-N by construction, so a future truncating
    // consumer needs no change.
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

#[cfg(test)]
mod lexicon_parity {
    use super::{classify_section, SectionKind};

    /// Every heading term the renderer's `SECTION_LEXICON` knows, swept
    /// through this classifier.
    ///
    /// The two sides answer the same question from different data — the TS
    /// lexicon drives `detectSections`, this drives `extract_evidence` — and
    /// nothing compared them until a sweep found German "Akademischer
    /// Werdegang" classifying as **Experience**, because it contains
    /// `werdegang`. That is the expensive direction: prose under an Experience
    /// heading reaches `extract_evidence`'s role arm, so degree entries became
    /// work bullets under roles the candidate never held.
    ///
    /// What this pins, in order of what each costs:
    ///
    /// 1. **No term may land in another section's bucket.** This catches the
    ///    class of bug above and has zero exceptions.
    /// 2. **The known misses are enumerated, not tolerated in bulk.** A term
    ///    falling to `Other` is a coverage gap, not a misfile — the
    ///    section-specific checks simply do not run. Listing all 40 turns a
    ///    silent gap into a reviewed inventory: adding a lexicon term without
    ///    teaching this classifier fails here, and FIXING one fails here too,
    ///    so the list can only shrink deliberately.
    ///
    /// The list is dominated by Summary and Skills synonyms in fr/es/it/nl/pt
    /// (`objectif`, `conoscenze`, `samenvatting`). Closing them is additive
    /// but not free: every entry in the heading consts is a SUBSTRING test
    /// against real headings, and `expertise`, `portfolio` and `studi` have
    /// second readings (Italian "studio" contains `studi`), so they need the
    /// word-bounded treatment rather than a bare push. Left as a bounded,
    /// visible task rather than a silent one.
    const KNOWN_MISSES: &[&str] = &[
        "à propos",
        "abilità",
        "aptitudes",
        "competenties",
        "conhecimentos",
        "conocimientos",
        "conoscenze",
        "diplômes",
        "doelstelling",
        "educação",
        "educación",
        "éducation",
        "educazione",
        "ervaring",
        "escolaridade",
        // Back on the list after a review proved the bare stem cannot be
        // taught safely: only the WORK-QUALIFIED plurals ("esperienze
        // professionali/lavorative") are in `EXPERIENCE_HEADINGS`, so the bare
        // word is an honest miss rather than a misfile.
        "esperienze",
        "estudios",
        "études",
        "expertise",
        "kennis",
        "loopbaan",
        "obiettivo",
        "objectif",
        "objetivo",
        "onderwijs",
        "over mij",
        "portfolio",
        "qualifications",
        "résumé",
        "resumen",
        "resumo",
        "riassunto",
        "samenvatting",
        "savoir-faire",
        "sobre mí",
        "sobre mim",
        "sommario",
        "studi",
        "studie",
        "über mich",
    ];

    /// `SectionKind` has no `Languages` variant, so a language heading has no
    /// bucket of its own. Skills is the closest true answer and matches what
    /// the résumé conventions already rely on — "Sprachkenntnisse" was
    /// rejected as a German producer heading for exactly this reason.
    /// Allowlisted explicitly rather than swept under the wrong-bucket rule,
    /// so adding a `Languages` variant later has to come past this comment.
    const LANGUAGES_CLASSIFY_AS_SKILLS: &[&str] = &["language skills", "sprachkenntnisse"];

    fn expected(section: &str) -> Option<SectionKind> {
        match section {
            "Summary" => Some(SectionKind::Summary),
            "Experience" => Some(SectionKind::Experience),
            "Education" => Some(SectionKind::Education),
            "Skills" => Some(SectionKind::Skills),
            "Projects" => Some(SectionKind::Projects),
            _ => None,
        }
    }

    #[test]
    fn every_renderer_lexicon_term_classifies_consistently() {
        #[derive(serde::Deserialize)]
        struct Case {
            section: String,
            term: String,
        }

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../packages/prompts/src/fixtures/section-lexicon.json");
        let raw = std::fs::read_to_string(&path).expect(
            "read section-lexicon parity fixture              (packages/prompts/src/fixtures/section-lexicon.json)",
        );
        let cases: Vec<Case> =
            serde_json::from_str(&raw).expect("parse section-lexicon parity fixture");
        assert!(!cases.is_empty(), "the lexicon fixture must not be empty");

        let mut unexpected_misses = Vec::new();
        let mut fixed_misses = Vec::new();

        for c in &cases {
            let kind = classify_section(&c.term);
            let known_miss = KNOWN_MISSES.contains(&c.term.as_str());

            match expected(&c.section) {
                Some(want) if kind == want => {
                    if known_miss {
                        fixed_misses.push(c.term.as_str());
                    }
                }
                Some(_) if kind == SectionKind::Other => {
                    if !known_miss {
                        unexpected_misses.push(c.term.as_str());
                    }
                }
                Some(want) => panic!(
                    "{:?} term {:?} classifies as {kind:?}, not {want:?} — it landed in                      ANOTHER section's bucket, which MISFILES its content rather than                      merely skipping it",
                    c.section, c.term
                ),
                None => assert!(
                    kind == SectionKind::Other
                        || LANGUAGES_CLASSIFY_AS_SKILLS.contains(&c.term.as_str()),
                    "{:?} term {:?} has no bucket of its own but classified as {kind:?}",
                    c.section,
                    c.term
                ),
            }
        }

        assert!(
            unexpected_misses.is_empty(),
            "these lexicon terms classify as Other and are not in KNOWN_MISSES — either              teach the classifier or add them there with a reason: {unexpected_misses:?}"
        );
        // Third direction, and the one the first draft left open: an entry
        // that names no lexicon term at all. Without this, KNOWN_MISSES could
        // be padded with anything and still "shrink deliberately" — a review
        // proved it by adding `"zzz-not-a-lexicon-term"` and watching the
        // suite stay green.
        let lexicon: std::collections::HashSet<&str> =
            cases.iter().map(|c| c.term.as_str()).collect();
        let dead: Vec<&str> = KNOWN_MISSES
            .iter()
            .chain(LANGUAGES_CLASSIFY_AS_SKILLS.iter())
            .copied()
            .filter(|t| !lexicon.contains(t))
            .collect();
        assert!(
            dead.is_empty(),
            "these entries name no lexicon term, so they exempt nothing and only              make the list look longer than the real gap: {dead:?}"
        );

        assert!(
            fixed_misses.is_empty(),
            "these terms now classify correctly but are still listed in KNOWN_MISSES —              remove them, so the list can only shrink deliberately: {fixed_misses:?}"
        );
    }

    /// The precedence exception stated as behaviour rather than list
    /// membership: each of these carries an EXPERIENCE substring and must
    /// still reach Education.
    #[test]
    fn education_headings_carrying_an_experience_stem_reach_education() {
        // TWO lists on purpose, because either alone passes for the wrong
        // reason. The loop below drives off the const, so a phrase ADDED
        // without thought is still asserted — but that is self-referential:
        // deleting an entry deletes its own assertion, and a mutation run
        // proved exactly that (removing `wissenschaftlicher werdegang` left
        // this test green). So the membership check comes first and is
        // written out by hand; it is the half that catches a deletion.
        assert_eq!(
            super::EDUCATION_OVERRIDES_EXPERIENCE,
            &[
                "akademischer werdegang",
                "akademischen werdegang",
                "wissenschaftlicher werdegang",
                "wissenschaftlichen werdegang",
                "schulischer werdegang",
                "bildungswerdegang",
                "akademische laufbahn",
                "ausbildungswerdegang",
            ],
            "the override list changed — every entry is load-bearing (each one              was found filing a degree as a job), so a removal must be a              deliberate edit here, not a silent side effect"
        );

        // Case-varied to prove the lowercasing in `classify_section` rather
        // than assuming it.
        for phrase in super::EDUCATION_OVERRIDES_EXPERIENCE {
            for heading in [phrase.to_string(), phrase.to_uppercase()] {
                assert_eq!(
                    classify_section(&heading),
                    SectionKind::Education,
                    "{heading:?} names an academic record; classifying it Experience                      files degree entries as work bullets under roles the candidate                      never held"
                );
            }
        }

        // Control: the same stem WITHOUT an education qualifier is still a work
        // history, so the exception must not have swallowed the rule.
        assert_eq!(
            classify_section("Beruflicher Werdegang"),
            SectionKind::Experience
        );
        assert_eq!(classify_section("Werdegang"), SectionKind::Experience);
    }

    /// The Italian half of the same invariant, and the regression a pre-PR
    /// review caught in the first draft of this change: putting the BARE stem
    /// `esperienze` in [`AMBIGUOUS_EXPERIENCE_HEADINGS`] sent "Esperienze di
    /// formazione" — an education heading — to Experience, because that set
    /// yields to summary and skills but never to education. A degree entry
    /// under it became a fabricated job.
    ///
    /// Only the WORK-QUALIFIED plurals are taught, so this pins both
    /// directions at once: the education headings stay Education, the
    /// qualified plurals reach Experience, and the bare word is an honest
    /// miss rather than a misfile.
    #[test]
    fn italian_experience_plurals_do_not_capture_education_headings() {
        for heading in [
            "Esperienze di formazione",
            "Formazione ed esperienze",
            "Istruzione ed esperienze",
        ] {
            assert_eq!(
                classify_section(heading),
                SectionKind::Education,
                "{heading:?} is an education heading; a bare `esperienze` stem files                  its degrees as work bullets"
            );
        }

        for heading in [
            "Esperienze professionali",
            "Esperienze lavorative",
            // Work-qualified spellings must also stay OUT of the
            // section-deleting Skills hole `AMBIGUOUS_EXPERIENCE_HEADINGS`
            // documents — nothing in `extract_evidence` reads a Skills
            // section, so a work history landing there is erased, not
            // mislabelled.
            "Esperienze professionali e competenze",
        ] {
            assert_eq!(
                classify_section(heading),
                SectionKind::Experience,
                "{heading:?} is work-qualified and must reach Experience"
            );
        }

        // The bare stem is deliberately NOT taught: it cannot be, without
        // re-opening the education capture above. `Other` is the honest
        // answer and `KNOWN_MISSES` records it.
        assert_eq!(classify_section("Esperienze"), SectionKind::Other);
        // Substring control for the word-bounded `istruzione`.
        assert_eq!(classify_section("Distruzione"), SectionKind::Other);
        assert_eq!(classify_section("Istruzione"), SectionKind::Education);
    }
}
