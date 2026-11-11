//! Skill/tool term recognition for the entry-grounding checks.
//!
//! Everything compares through the keyword kernel (`documents::keywords`): the
//! same tokenizer, synonym table and stemming decision the match score uses,
//! never a raw substring. Two folds sit in front of it so a spelling variant
//! can't read as a different tool: `React.js`/`Node.js`/`Next.js` lose their
//! `.js`, and `CI-CD`/`CI / CD` collapse to the kernel's `ci/cd`.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::SectionKind;
use crate::documents::evidence::{GEOGRAPHY_TOKENS, LEGAL_FORMS};
use crate::documents::keywords::{keywords_normalized_for_lang, SHORT_TECH_TERMS, SYNONYMS};
use crate::validate::content::{Analysis, Section};

static JS_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(node|react|vue|next|nuxt|express|angular)\.?js\b").unwrap()
});
static CICD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bci\s*[/&-]\s*cd\b").unwrap());

/// Acronyms that are not tools. All-caps shape alone would read them as one.
const NON_TOOL_ACRONYMS: &[&str] = &[
    "CEO", "CTO", "CFO", "COO", "CIO", "VP", "HR", "PR", "IT", "USA", "EU", "UK", "US", "MBA",
    "BSC", "MSC", "PHD", "GPA", "KPI", "OKR", "B2B", "B2C", "FAQ", "NDA", "GMBH", "EMEA", "APAC",
    "DACH",
];

/// Kernel vocabulary words that are also ordinary words: only a capitalised
/// spelling counts as the tool.
const AMBIGUOUS_VOCAB: &[&str] = &["go", "r"];

/// Generic practice words a Skills section lists ("Testing · Security") that
/// ordinary bullets use in other forms. Never promoted to a tool claim by a
/// Skills declaration; everything else keeps the exact-surface rule, so
/// `Terraform`, `Docker` and `Redis` stay Critical.
const PRACTICE_WORDS: &[&str] = &[
    "testing",
    "security",
    "monitoring",
    "design",
    "agile",
    "scrum",
    "kanban",
    "leadership",
    "mentoring",
    "communication",
    "documentation",
    "automation",
    "management",
    "architecture",
    "analytics",
    "collaboration",
    "testen",
    "sicherheit",
    "überwachung",
    "führung",
    "kommunikation",
    "dokumentation",
    "automatisierung",
    "architektur",
    "analyse",
    "teamarbeit",
    "zusammenarbeit",
];

/// Languages that capitalise every noun, where mid-sentence capitalisation says
/// nothing about being a proper name.
const NOUN_CAPITALIZING_LANGS: &[&str] = &["de", "lb"];

const NON_TERM_CAPITALS: &[&str] = &[
    "january",
    "february",
    "march",
    "april",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

/// Canonical forms in the kernel's synonym table, so `python`/`kubernetes`
/// count as vocabulary whatever their casing.
static CANON: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| SYNONYMS.iter().map(|(_, canon)| *canon).collect());

/// The kernel's keys for `text` under this report's stemming decision, plus the
/// unstemmed normalized form: either one matching is a match.
pub(super) fn keys(ctx: &Analysis, text: &str) -> HashSet<String> {
    let clean = preclean(text);
    let mut keys = ctx.document.tokens(&clean);
    keys.extend(keywords_normalized_for_lang(&clean, &ctx.lang));
    keys
}

/// One form per token (the stemmed one when stemming applies): the set overlap
/// is counted on, so a word shared by both sides counts once.
pub(super) fn stems(ctx: &Analysis, text: &str) -> HashSet<String> {
    ctx.document.tokens(&preclean(text))
}

// TODO(#1498): the kernel tokenizer splits `Node.js` at the dot, so spelling
// variants are folded here instead. Fixing it in `documents/keywords` would move
// ATS scores and cached keyword sets, so it is deliberately not done in this PR.
fn preclean(text: &str) -> String {
    let text = JS_SUFFIX.replace_all(text, "$1");
    CICD.replace_all(&text, "ci/cd").into_owned()
}

/// A skill or tool the generated text names.
#[derive(Clone)]
pub(super) struct Term {
    /// The span exactly as written.
    pub word: String,
    pub keys: HashSet<String>,
    /// Tool-shaped or declared: safe to raise a Critical on. A merely
    /// capitalised word is `false` and can only ever warn.
    pub strong: bool,
}

impl Term {
    pub fn supported_by(&self, set: &HashSet<String>) -> bool {
        self.keys.iter().any(|k| set.contains(k))
    }
}

fn has_alpha(s: &str) -> bool {
    s.chars().any(char::is_alphabetic)
}

/// Mixed case after the first letter: `ESLint`, `WebSocket`, `iOS`.
fn has_inner_capital(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next();
    let rest: Vec<char> = chars.collect();
    rest.iter().any(|c| c.is_uppercase()) && word.chars().any(|c| c.is_lowercase())
}

fn all_caps(word: &str) -> bool {
    word.chars().filter(|c| c.is_alphabetic()).count() >= 2
        && !word.chars().any(|c| c.is_lowercase())
}

fn inner_dot(word: &str) -> bool {
    word.split('.').filter(|p| !p.is_empty()).count().gt(&1)
        && word.split('.').all(|p| p.chars().count() >= 2)
}

fn shape_is_tool(word: &str) -> bool {
    if word.starts_with(|c: char| c.is_ascii_digit()) {
        return false; // "480ms", "40k": a quantity, not a tool
    }
    let digit_and_alpha = word.chars().any(|c| c.is_ascii_digit()) && has_alpha(word);
    let plus_hash = word.contains(['+', '#']) && has_alpha(word);
    let acronym = all_caps(word)
        && !PRACTICE_WORDS.contains(&word.to_lowercase().as_str())
        && !NON_TOOL_ACRONYMS.contains(&word.to_uppercase().as_str())
        && !LEGAL_FORMS.contains(&word.to_lowercase().as_str());
    digit_and_alpha || plus_hash || inner_dot(word) || has_inner_capital(word) || acronym
}

fn is_vocab(word: &str, keys: &HashSet<String>) -> bool {
    keys.iter().any(|k| {
        (SHORT_TECH_TERMS.contains(&k.as_str()) || CANON.contains(k.as_str()))
            && (!AMBIGUOUS_VOCAB.contains(&k.as_str()) || word.starts_with(char::is_uppercase))
    })
}

fn is_capitalized(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(char::is_uppercase)
        && word.chars().count() >= 3
        && !chars.any(|c| c.is_uppercase() || c.is_ascii_digit())
}

fn starts_sentence(previous: Option<&str>) -> bool {
    previous.is_none_or(|p| {
        p.ends_with(['.', '!', '?', ':', ';']) || p.chars().all(|c| !c.is_alphanumeric())
    })
}

/// The skill/tool terms in one line of prose.
///
/// A word counts when it is kernel vocabulary, tool-shaped (`ESLint`, `AWS`,
/// `Node.js`, `C++`), a skill the SOURCE declares in its own Skills section
/// (`declared`), or, outside noun-capitalising languages, a capitalised word in
/// mid-sentence. Only the last kind is `strong == false`.
pub(super) fn terms_in(ctx: &Analysis, line: &str, declared: &HashSet<String>) -> Vec<Term> {
    let capitals_mean_names = !NOUN_CAPITALIZING_LANGS.contains(&ctx.lang.as_str());
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut previous: Option<&str> = None;
    for chunk in line.split_whitespace() {
        let at_start = starts_sentence(previous);
        previous = Some(chunk);
        for piece in chunk.split(|c: char| !(c.is_alphanumeric() || "+#./".contains(c))) {
            let word = piece.trim_matches(['.', '/']);
            if word.is_empty() || !seen.insert(word.to_lowercase()) {
                continue;
            }
            // `CI/CD` is one kernel token; `REST/WebSocket` is not (the kernel
            // rejects slash tokens), so it falls back to its halves.
            let parts: Vec<&str> = if word.contains('/') && keys(ctx, word).is_empty() {
                word.split('/').filter(|p| !p.is_empty()).collect()
            } else {
                vec![word]
            };
            for part in parts {
                let keys = keys(ctx, part);
                if keys.is_empty() {
                    continue;
                }
                let lower = part.to_lowercase();
                let proper = capitals_mean_names && !at_start && is_capitalized(part);
                // A declared Skills item makes a word a tool when the output
                // spells it exactly as the source's Skills section does
                // ("Terraform" in any position or language). "Mentored" is not
                // the declared "Mentoring", so a soft skill's verb stays quiet.
                let strong = shape_is_tool(part)
                    || is_vocab(part, &keys)
                    || (declared.contains(&lower) && !PRACTICE_WORDS.contains(&lower.as_str()));
                let weak = proper
                    && !LEGAL_FORMS.contains(&lower.as_str())
                    && !GEOGRAPHY_TOKENS.contains(&lower.as_str())
                    && !NON_TERM_CAPITALS.contains(&lower.as_str());
                if strong || weak {
                    out.push(Term {
                        word: part.to_string(),
                        keys,
                        strong,
                    });
                }
            }
        }
    }
    out
}

/// Single-word items of a Skills section (`Rust · Python · Docker`), as terms.
/// Structure alone makes them skills, whatever their casing or language.
pub(super) fn skill_items(ctx: &Analysis, section: &Section) -> Vec<Term> {
    let mut out = Vec::new();
    for line in &section.lines {
        let text = match line.text.split_once(':') {
            Some((label, rest)) if label.chars().count() <= 25 => rest,
            _ => line.text.as_str(),
        };
        for item in text.split(['·', ',', ';', '|', '•']) {
            let word = item.trim();
            if word.is_empty() || word.split_whitespace().count() != 1 {
                continue;
            }
            let keys = keys(ctx, word);
            if !keys.is_empty() {
                out.push(Term {
                    word: word.to_string(),
                    keys,
                    strong: true,
                });
            }
        }
    }
    out
}

/// The single-word Skills items the source declares, lowercased as written.
pub(super) fn declared_skills(ctx: &Analysis, sections: &[Section]) -> HashSet<String> {
    sections
        .iter()
        .filter(|s| s.kind == SectionKind::Skills)
        .flat_map(|s| skill_items(ctx, s))
        .map(|t| t.word.to_lowercase())
        .collect()
}
