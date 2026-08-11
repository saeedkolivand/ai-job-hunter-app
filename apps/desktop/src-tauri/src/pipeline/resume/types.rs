//! The stage artifacts of the résumé pipeline — the typed values the model is
//! asked for, and the shapes it is shown.
//!
//! **Every type here is FLAT and every field is `#[serde(default)]`.** Both are
//! load-bearing, for opposite reasons:
//!
//! * *Flat* (no `$defs`, at most ONE level of nesting — an array of plain
//!   objects) because the 2026 measurements on small local models put nested
//!   `$defs` schemas at ~68% non-compliance. Every provider's constrained
//!   decoder handles a flat schema; only some handle a referenced one, and the
//!   ones that don't are exactly the ones this app runs offline.
//! * *Defaulted* because a missing key must degrade to an empty value rather
//!   than failing the whole parse — a model that omits `red_flags` has still
//!   answered the question. The safety net for the OPPOSITE failure (a
//!   truncated response deserializing cleanly into an all-empty struct) is not
//!   here: it is [`Completer::complete_json`](crate::pipeline::Completer::complete_json),
//!   which hard-errors rather than persisting a partial parse, plus each
//!   stage's own emptiness check.
//!
//! Each type also carries a FILLED example ([`JobAnalysis::EXAMPLE`], …) used as
//! the `schema_hint`, and a flat JSON Schema for the providers with native
//! constrained decoding. The example's values are placeholders — the shared
//! `JSON_EXAMPLE_DIRECTIVE` tells the model so.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ipc_contracts::events::{is_pipeline_section_key, PIPELINE_SECTION_EXPERIENCE_PREFIX};

/// What the posting is actually asking for. **Presentation metadata only.**
///
/// This never enters match scoring: `score_one`/`MATCH_FORMULA_VERSION` read
/// the posting text through `documents::keywords`, and a model-derived
/// requirement list feeding a number the user reads as objective is exactly the
/// core-rule violation this pipeline exists to prevent. Pinned by
/// `job_analysis_never_reaches_match_scoring`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JobAnalysis {
    pub role_title: String,
    pub seniority: String,
    /// The language the posting is written in, as a 2-letter code. Advisory:
    /// the run's target language comes from the request, and
    /// `validate::content` decides language questions through
    /// `languages_align`.
    pub language: String,
    pub must_have: Vec<String>,
    pub nice_to_have: Vec<String>,
    pub responsibilities: Vec<String>,
    pub domain_keywords: Vec<String>,
    pub red_flags: Vec<String>,
}

impl JobAnalysis {
    pub const EXAMPLE: &'static str = r#"{
  "roleTitle": "Senior Backend Engineer",
  "seniority": "senior",
  "language": "en",
  "mustHave": ["Go", "Kubernetes", "distributed systems"],
  "niceToHave": ["Terraform", "gRPC"],
  "responsibilities": ["Own the payments service end to end"],
  "domainKeywords": ["payments", "PCI"],
  "redFlags": ["on-call rotation not described"]
}"#;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "roleTitle": { "type": "string" },
                "seniority": { "type": "string" },
                "language": { "type": "string" },
                "mustHave": { "type": "array", "items": { "type": "string" } },
                "niceToHave": { "type": "array", "items": { "type": "string" } },
                "responsibilities": { "type": "array", "items": { "type": "string" } },
                "domainKeywords": { "type": "array", "items": { "type": "string" } },
                "redFlags": { "type": "array", "items": { "type": "string" } },
            },
        })
    }

    /// Whether the model answered at all. A structurally-valid but empty
    /// analysis is a failed stage, not a posting with no requirements.
    pub fn is_empty(&self) -> bool {
        self.role_title.trim().is_empty()
            && self.must_have.is_empty()
            && self.nice_to_have.is_empty()
            && self.responsibilities.is_empty()
    }
}

/// Whether the source résumé evidences one requirement.
///
/// **Decided by Rust, never by the model.** The model ranks and quotes; the
/// status is overwritten from the `documents::keywords` kernel — the same
/// tokenizer, stemmer and `SYNONYMS` the match score uses — so the pipeline's
/// coverage claim and the score the user already sees cannot disagree. A model
/// that calls a missing skill "covered" is the single most damaging thing this
/// artifact could carry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceStatus {
    /// Every token of the requirement is present in the source résumé.
    Covered,
    /// Some tokens are present.
    Partial,
    /// None are.
    #[default]
    Missing,
}

impl EvidenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Covered => "covered",
            Self::Partial => "partial",
            Self::Missing => "missing",
        }
    }
}

/// One requirement, and what the candidate's OWN résumé says about it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EvidenceItem {
    pub requirement: String,
    /// Overwritten by the kernel after parsing — see [`EvidenceStatus`].
    pub status: EvidenceStatus,
    /// A span the model claims to have copied out of the source résumé.
    /// **Dropped (emptied), never repaired, when it is not verbatim** — see
    /// `stages::match_evidence`.
    pub source_quote: String,
    pub source_company: String,
    /// 0–3, the model's own ranking of how strongly the quote supports the
    /// requirement. Clamped on parse; advisory only.
    pub strength: u8,
}

/// The evidence artifact: one flat list, deliberately not a map keyed by
/// requirement (a JSON object with model-chosen KEYS is the shape small models
/// get wrong most often, and it cannot be schema'd flatly).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EvidenceMap {
    pub items: Vec<EvidenceItem>,
}

impl EvidenceMap {
    pub const EXAMPLE: &'static str = r#"{
  "items": [
    {
      "requirement": "Kubernetes",
      "status": "covered",
      "sourceQuote": "Migrated 40 services to Kubernetes, cutting deploy time from 25 to 4 minutes",
      "sourceCompany": "Acme Payments",
      "strength": 3
    }
  ]
}"#;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "requirement": { "type": "string" },
                            "status": { "type": "string", "enum": ["covered", "partial", "missing"] },
                            "sourceQuote": { "type": "string" },
                            "sourceCompany": { "type": "string" },
                            "strength": { "type": "integer", "minimum": 0, "maximum": 3 },
                        },
                    },
                },
            },
        })
    }
}

/// How ONE employment entry should be presented. The identity fields
/// (`company`/`title`/`dates`) are SEEDED verbatim from the parsed source
/// résumé before the model ever sees them, and re-seeded after parsing — the
/// model may reorder and emphasize, never rename an employer or move a date.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompanyPlan {
    pub company: String,
    pub title: String,
    pub dates: String,
    /// The angle this role should be told from, for this posting.
    pub angle: String,
    /// Which of the posting's requirements this role should evidence.
    pub emphasis: Vec<String>,
    /// `true` for the ONE condensed "earlier roles" group that carries every
    /// role past the per-company cap. Never dropped, only condensed.
    pub condensed: bool,
}

/// One labelled skills cluster (`"Languages"`, `"Infrastructure"`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillGroup {
    pub label: String,
    pub skills: Vec<String>,
}

/// The plan the draft is written against.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ResumeStrategy {
    pub headline_angle: String,
    pub summary_focus: Vec<String>,
    pub section_order: Vec<String>,
    pub per_company: Vec<CompanyPlan>,
    pub skills_groups: Vec<SkillGroup>,
}

impl ResumeStrategy {
    pub const EXAMPLE: &'static str = r#"{
  "headlineAngle": "Payments-platform engineer who ships reliability work",
  "summaryFocus": ["distributed systems", "payments domain"],
  "sectionOrder": ["summary", "skills", "experience", "projects", "education"],
  "perCompany": [
    {
      "company": "Acme Payments",
      "title": "Senior Backend Engineer",
      "dates": "2021 - Present",
      "angle": "Lead the reliability story with the migration numbers",
      "emphasis": ["Kubernetes", "Go"],
      "condensed": false
    }
  ],
  "skillsGroups": [{ "label": "Languages", "skills": ["Go", "Rust"] }]
}"#;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "headlineAngle": { "type": "string" },
                "summaryFocus": { "type": "array", "items": { "type": "string" } },
                "sectionOrder": { "type": "array", "items": { "type": "string" } },
                "perCompany": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "company": { "type": "string" },
                            "title": { "type": "string" },
                            "dates": { "type": "string" },
                            "angle": { "type": "string" },
                            "emphasis": { "type": "array", "items": { "type": "string" } },
                            "condensed": { "type": "boolean" },
                        },
                    },
                },
                "skillsGroups": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": { "type": "string" },
                            "skills": { "type": "array", "items": { "type": "string" } },
                        },
                    },
                },
            },
        })
    }
}

/// Which résumé section a section-wise operation addresses.
///
/// **There is no `Header` variant, by design.** The contact header is owned by
/// the editor at export time (ADR-0021), so "regenerate the header" is not a
/// request this pipeline can express — the type system refuses it rather than a
/// runtime branch that someone can later forget. [`SectionKey::from_wire`]
/// therefore errs on `"header"` like any other unknown token, which is
/// test-pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKey {
    Summary,
    Skills,
    /// One employment entry, by its 0-based index in the source résumé.
    Experience(u8),
    Projects,
    Education,
}

impl SectionKey {
    /// Parse the wire form of a `pipeline:stage` `sectionKey`.
    ///
    /// The grammar check is the GENERATED [`is_pipeline_section_key`] — the
    /// Rust twin of the TS guard, emitted by `pnpm gen:ipc` from the same
    /// source — so the boundary this rejects at and the vocabulary the renderer
    /// types against cannot drift. Running it FIRST also means the length bound
    /// and the canonical-decimal rule (`experience:01` is not a second spelling
    /// of `experience:1`) apply before anything else is done with the value.
    pub fn from_wire(value: &str) -> Option<Self> {
        if !is_pipeline_section_key(value) {
            return None;
        }
        match value {
            "summary" => Some(Self::Summary),
            "skills" => Some(Self::Skills),
            "projects" => Some(Self::Projects),
            "education" => Some(Self::Education),
            _ => value
                .strip_prefix(PIPELINE_SECTION_EXPERIENCE_PREFIX)
                .and_then(|index| index.parse::<u8>().ok())
                .map(Self::Experience),
        }
    }

    /// The wire form — round-trips through [`Self::from_wire`].
    pub fn to_wire(self) -> String {
        match self {
            Self::Summary => "summary".to_string(),
            Self::Skills => "skills".to_string(),
            Self::Projects => "projects".to_string(),
            Self::Education => "education".to_string(),
            Self::Experience(index) => format!("{PIPELINE_SECTION_EXPERIENCE_PREFIX}{index}"),
        }
    }
}

/// How much work one generation is allowed to do. Parsed from the wire token
/// against the GENERATED [`crate::ipc_contracts::generation_depths`]
/// vocabulary, so an unknown depth is a validation error rather than a silent
/// fallback to the cheapest (or most expensive) tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationDepth {
    Fast,
    Quality,
    Max,
}

impl GenerationDepth {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "fast" => Some(Self::Fast),
            "quality" => Some(Self::Quality),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Quality => "quality",
            Self::Max => "max",
        }
    }
}
