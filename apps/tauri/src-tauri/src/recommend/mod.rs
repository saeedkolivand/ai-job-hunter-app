//! Template + locale recommender.
//!
//! Maps the signals the TS metadata step already extracts (job title, candidate
//! seniority, top requirements, resume/job-ad language) to a suggested template,
//! locale, and whether to enable ATS mode — always with a plain-language
//! rationale. Rules-first and deterministic (an AI tiebreak can layer on later);
//! the recommendation is a suggestion the user can always override.

use serde::{Deserialize, Serialize};

use crate::export::types::TemplateId;

/// Inputs to the recommender — mirrors the TS `GenerationMeta` signals.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendSignals {
    pub job_title: Option<String>,
    /// `junior | mid | senior | lead | executive`
    pub candidate_seniority: Option<String>,
    #[serde(default)]
    pub top_requirements: Vec<String>,
    pub resume_language: Option<String>,
    pub job_ad_language: Option<String>,
    /// The job ad's target country/market (`us`, `de`, `gb`, …), when known —
    /// e.g. from the posting's location. Takes priority over language for the
    /// locale, since `en` alone can't tell the US (Letter) from the UK (A4).
    pub target_country: Option<String>,
}

/// A template + locale suggestion with a printed reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub template_id: TemplateId,
    /// Locale/market id resolvable by [`crate::locale::LocaleProfile::get`].
    pub locale: String,
    /// Whether to suggest ATS (single-column) mode.
    pub ats_suggested: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Academia,
    Conservative,
    Design,
    Software,
    General,
}

/// Recommend a template + locale from the metadata signals.
pub fn recommend(signals: &RecommendSignals) -> Recommendation {
    let haystack = haystack(signals);
    let field = detect_field(&haystack);
    let seniority = signals
        .candidate_seniority
        .as_deref()
        .unwrap_or("")
        .to_lowercase();
    let senior_exec = matches!(seniority.as_str(), "executive" | "lead");

    let (template_id, reason) = pick_template(field, senior_exec, &haystack);

    // Word-boundary match instead of the old space-padded `" ats"` /
    // `starts_with("ats")` hacks — those missed "ATS" at the very start without
    // the prefix branch and risked matching "ats" inside other words.
    let ht = tokens(&haystack);
    let ats_suggested = matches!(field, Field::Conservative)
        || contains_phrase(&ht, "applicant tracking")
        || contains_phrase(&ht, "ats");

    let locale = pick_locale(signals);

    let mut rationale = reason.to_string();
    if ats_suggested {
        rationale.push_str(" ATS mode suggested (single column) for reliable parsing.");
    }
    if locale != "en" {
        rationale.push_str(&format!(
            " Locale set to {} from the target market.",
            locale
        ));
    }

    Recommendation {
        template_id,
        locale,
        ats_suggested,
        rationale,
    }
}

fn haystack(signals: &RecommendSignals) -> String {
    let mut s = signals.job_title.clone().unwrap_or_default();
    s.push(' ');
    s.push_str(&signals.top_requirements.join(" "));
    s.to_lowercase()
}

const ACADEMIA_KW: &[&str] = &[
    "professor",
    "researcher",
    "phd",
    "postdoc",
    "post-doc",
    "lecturer",
    "academic",
    "dissertation",
    "research scientist",
];

const CONSERVATIVE_KW: &[&str] = &[
    "lawyer",
    "attorney",
    "legal",
    "paralegal",
    "accountant",
    "auditor",
    "finance",
    "financial",
    "compliance",
    "banker",
    "investment",
    "actuary",
    "government",
    "public sector",
    "policy analyst",
    "tax",
];

const DESIGN_KW: &[&str] = &[
    "designer",
    "ux",
    "ui",
    "ux/ui",
    "creative",
    "art director",
    "brand",
    "graphic",
    "illustrator",
    "motion design",
    "product design",
];

const SOFTWARE_KW: &[&str] = &[
    "engineer",
    "developer",
    "software",
    "programmer",
    "data scientist",
    "data engineer",
    "devops",
    "sre",
    "backend",
    "frontend",
    "full stack",
    "fullstack",
    "machine learning",
    "cloud",
    "architect",
];

/// Split a string into lowercase alphanumeric word tokens (the same boundary
/// model used on both sides of [`contains_phrase`]). `c++`/`ux/ui` split on the
/// non-alphanumeric separators into their word parts.
fn tokens(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word-boundary phrase match: true when the keyword's token sequence appears as
/// a contiguous run inside the haystack's token sequence. Single-word keywords
/// are the length-1 case. This is what replaces the old `h.contains(k)` substring
/// test (which matched "tax" inside "syntax", "ats" inside "stats", etc.) and the
/// space-padded `" ats"` / `"ui "` hacks.
fn contains_phrase(haystack_tokens: &[&str], keyword: &str) -> bool {
    let needle = tokens(keyword);
    if needle.is_empty() {
        return false;
    }
    haystack_tokens
        .windows(needle.len())
        .any(|w| w == needle.as_slice())
}

/// Number of a field's keywords present (word-boundary) in the haystack tokens.
fn field_score(haystack_tokens: &[&str], keywords: &[&str]) -> usize {
    keywords
        .iter()
        .filter(|k| contains_phrase(haystack_tokens, k))
        .count()
}

/// Classify the role by scoring EVERY field's keyword hits and taking the
/// strongest, instead of the old first-match-wins ordering. Scoring all fields
/// means a title like "financial software engineer" (1 Conservative hit vs. 2
/// Software hits) classifies as Software, not Conservative. Ties fall back to the
/// listed priority order (Academia → Conservative → Design → Software); a tie at
/// zero hits is [`Field::General`].
fn detect_field(h: &str) -> Field {
    let ht = tokens(h);
    // Ordered so a tie resolves to the earlier (higher-priority) field via the
    // strict `>` comparison below — the first field with the max score wins.
    let scored = [
        (Field::Academia, field_score(&ht, ACADEMIA_KW)),
        (Field::Conservative, field_score(&ht, CONSERVATIVE_KW)),
        (Field::Design, field_score(&ht, DESIGN_KW)),
        (Field::Software, field_score(&ht, SOFTWARE_KW)),
    ];

    let mut best = Field::General;
    let mut best_score = 0;
    for (field, score) in scored {
        if score > best_score {
            best = field;
            best_score = score;
        }
    }
    best
}

fn pick_template(field: Field, senior_exec: bool, h: &str) -> (TemplateId, &'static str) {
    match field {
        Field::Academia => (
            TemplateId::Academic,
            "Academic / research role → Academic template.",
        ),
        Field::Conservative => (
            TemplateId::Classic,
            "Conservative field (finance / legal / public sector) → Classic single-column.",
        ),
        Field::Design => {
            if senior_exec {
                (
                    TemplateId::Meridian,
                    "Senior design leadership → Meridian header-forward template.",
                )
            } else {
                (
                    TemplateId::Atelier,
                    "Design role → Atelier two-column visual template.",
                )
            }
        }
        Field::Software => {
            if senior_exec {
                (
                    TemplateId::Meridian,
                    "Senior engineering leadership → Meridian header-forward template.",
                )
            } else if is_systems_role(h) {
                (
                    TemplateId::Modern,
                    "Systems / low-level role → Modern clean template.",
                )
            } else {
                (
                    TemplateId::Modern,
                    "Software / engineering role → Modern template.",
                )
            }
        }
        Field::General => {
            if senior_exec {
                (
                    TemplateId::Meridian,
                    "Senior leadership → Meridian header-forward template.",
                )
            } else {
                (TemplateId::Modern, "General role → Modern template.")
            }
        }
    }
}

fn is_systems_role(h: &str) -> bool {
    [
        "embedded",
        "kernel",
        "firmware",
        "compiler",
        "low-level",
        "systems programming",
        "rust",
        "c++",
    ]
    .iter()
    .any(|k| h.contains(k))
}

fn pick_locale(signals: &RecommendSignals) -> String {
    // An explicit target country/market wins — resolved through the locale
    // registry so there's a single source of truth (e.g. `gb` → `uk`).
    if let Some(country) = signals
        .target_country
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return crate::locale::LocaleProfile::get(country).id.to_string();
    }

    // Otherwise derive from the job-ad language (preferred) or resume language.
    let tag = signals
        .job_ad_language
        .as_deref()
        .or(signals.resume_language.as_deref())
        .unwrap_or("en")
        .to_lowercase();
    let mut parts = tag.split(['-', '_']);
    let lang = parts.next().unwrap_or("en");

    // A region subtag resolves directly (`en-US` → us, `en-GB` → uk).
    if let Some(region) = parts.next() {
        let profile = crate::locale::LocaleProfile::get(region);
        if profile.id != "en" {
            return profile.id.to_string();
        }
    }

    match lang {
        "de" => "dach",
        "fr" => "fr",
        "nl" => "nl",
        _ => "en",
    }
    .to_string()
}

#[cfg(test)]
mod tests;
