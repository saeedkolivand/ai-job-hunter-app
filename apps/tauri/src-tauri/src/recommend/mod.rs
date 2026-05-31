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

    let ats_suggested = matches!(field, Field::Conservative)
        || haystack.contains("applicant tracking")
        || haystack.contains(" ats")
        || haystack.starts_with("ats");

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

fn detect_field(h: &str) -> Field {
    let has = |kws: &[&str]| kws.iter().any(|k| h.contains(k));

    if has(&[
        "professor",
        "researcher",
        "phd",
        "postdoc",
        "post-doc",
        "lecturer",
        "academic",
        "dissertation",
        "research scientist",
    ]) {
        Field::Academia
    } else if has(&[
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
        "tax ",
    ]) {
        Field::Conservative
    } else if has(&[
        "designer",
        "ux",
        "ui ",
        "ux/ui",
        "creative",
        "art director",
        "brand",
        "graphic",
        "illustrator",
        "motion design",
        "product design",
    ]) {
        Field::Design
    } else if has(&[
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
    ]) {
        Field::Software
    } else {
        Field::General
    }
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
                    TemplateId::RefinedExecutive,
                    "Senior design leadership → Refined Executive.",
                )
            } else {
                (
                    TemplateId::TwoColumn,
                    "Design role → Two-Column visual template.",
                )
            }
        }
        Field::Software => {
            if senior_exec {
                (
                    TemplateId::RefinedExecutive,
                    "Senior engineering leadership → Refined Executive.",
                )
            } else if is_systems_role(h) {
                (
                    TemplateId::MonoTechnical,
                    "Systems / low-level role → Mono Technical.",
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
                    TemplateId::RefinedExecutive,
                    "Senior leadership → Refined Executive.",
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
