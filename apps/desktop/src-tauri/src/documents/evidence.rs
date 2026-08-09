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
}

impl JobVocabulary {
    fn new(resume_text: &str, job_text: &str) -> Self {
        let aligned = languages_align(job_text, detect_locale_tag(resume_text));
        let stemmer = make_stemmer(job_text);
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
        }
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
            .map(|stem| {
                self.display
                    .get(stem)
                    .cloned()
                    .unwrap_or_else(|| stem.clone())
            })
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionKind {
    Experience,
    Education,
    Projects,
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
    "formation",
    "formación",
    "formazione",
    "opleiding",
    "formação",
];
const PROJECT_HEADINGS: &[&str] = &["project", "projekt", "projet", "proyecto", "progetti"];

fn classify_section(heading: &str) -> SectionKind {
    let lower = heading.to_lowercase();
    let has = |set: &[&str]| set.iter().any(|k| lower.contains(k));
    if has(EXPERIENCE_HEADINGS) {
        SectionKind::Experience
    } else if has(EDUCATION_HEADINGS) {
        SectionKind::Education
    } else if has(PROJECT_HEADINGS) {
        SectionKind::Projects
    } else {
        SectionKind::Other
    }
}

/// Split an entry line into `(company, title, dates)`.
///
/// HEURISTIC, and deliberately a conservative one — a wrong split must never
/// invent a company the résumé does not name. Three shapes, matching the three
/// `LineKind::JobEntry` forms `export::parser` produces:
///
/// 1. Two-space form (`Acme Corp    2021 – Present`): the parser already split
///    the date into `right_text`, so `text` is the entry label.
/// 2. Pipe/middot form (`Senior Engineer | Acme Corp | 2021 – Present`): the
///    date-shaped segment is the span, the first segment is the title and the
///    second the company (the order every template in this repo renders).
/// 3. Parenthesized form (`Senior Engineer, Acme Corp (Jan 2021 – Mar 2023)`):
///    the parenthesized tail is the span, and the label splits at the LAST
///    comma.
///
/// A label with no separator becomes the company with an empty title — a
/// following `LineKind::JobTitle` line fills that in.
fn split_entry(line: &ParsedLine) -> (String, String, String) {
    let label_and_dates = |label: &str, dates: &str| {
        let (title, company) = match label.rsplit_once(',') {
            Some((t, c)) => (t.trim().to_string(), c.trim().to_string()),
            None => (String::new(), label.trim().to_string()),
        };
        (company, title, dates.trim().to_string())
    };

    if let Some(dates) = line.right_text.as_deref() {
        return label_and_dates(&line.text, dates);
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
/// pipeline supports. Lowercased comparison.
pub const PRESENT_MARKERS: &[&str] = &[
    "present", "current", "now", "ongoing", "heute", "aktuell", "laufend", "actuel", "actual",
    "attuale", "heden", "atual",
];

/// True when `s` carries a year (1900–2099) or a present-tense marker — the
/// shape a date span has. Shared with the content validators so "what counts as
/// a date" is decided in one place.
pub fn looks_like_date_span(s: &str) -> bool {
    let lower = s.to_lowercase();
    !years_in(s).is_empty() || PRESENT_MARKERS.iter().any(|m| lower.contains(m))
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
                SectionKind::Experience => {
                    if let Some(role_idx) = set.roles.len().checked_sub(1) {
                        let bullet_idx = set.roles[role_idx].bullets.len();
                        let bullet = vocab.bullet(format!("r{role_idx}b{bullet_idx}"), &line.text);
                        set.roles[role_idx].bullets.push(bullet);
                    }
                }
                SectionKind::Projects => {
                    let bullet = vocab.bullet(format!("p{}", set.projects.len()), &line.text);
                    set.projects.push(bullet);
                }
                SectionKind::Education => set.education.push(line.text.clone()),
                SectionKind::Other => {}
            },
            LineKind::Text | LineKind::JobEntry
                if section == SectionKind::Education && !line.text.trim().is_empty() =>
            {
                set.education.push(line.text.clone())
            }
            LineKind::Text | LineKind::JobEntry
                if section == SectionKind::Projects && !line.text.trim().is_empty() =>
            {
                let bullet = vocab.bullet(format!("p{}", set.projects.len()), &line.text);
                set.projects.push(bullet);
            }
            _ => {}
        }
    }

    let resume_tokens = vocab.tokens(source_resume);
    let readable = |stem: &String| {
        vocab
            .display
            .get(stem)
            .cloned()
            .unwrap_or_else(|| stem.clone())
    };
    set.skills_present = vocab
        .keywords
        .intersection(&resume_tokens)
        .map(readable)
        .collect();
    set.skills_absent = vocab
        .keywords
        .difference(&resume_tokens)
        .map(readable)
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
}
