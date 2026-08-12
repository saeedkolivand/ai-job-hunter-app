//! The MAX-depth section artifacts — one typed answer per résumé section, and
//! the SEEDS each section is generated from.
//!
//! Same two rules as [`super::types`], for the same two reasons: every type is
//! FLAT (an object of scalars and arrays, at most an array of plain objects) so
//! a small local model's constrained decoder can hold it, and every field is
//! `#[serde(default)]` so a missing key degrades to an empty value instead of
//! failing the parse. Each also carries a FILLED example used as the
//! `schema_hint`.
//!
//! ## What the model is allowed to author, per section
//!
//! The split between [`SectionSeed`] and the `*Out` types below IS the core
//! rule, expressed as types: the seed carries what the SOURCE résumé says
//! (identity, links, the education lines) and the `*Out` type carries the only
//! thing the model decides — how to present it.
//!
//! | section    | seeded, never model-authored          | the model authors        |
//! | ---------- | ------------------------------------- | ------------------------ |
//! | summary    | —                                     | the paragraph            |
//! | skills     | the groups the strategy grounded      | order + which survive    |
//! | experience | company / title / dates               | the bullets              |
//! | projects   | name / links / stack                  | the description only     |
//! | education  | every line, verbatim from the source  | order + which survive    |
//!
//! There is deliberately no `severity` anywhere in [`JudgeOut`]: the judge is
//! Warning-only, and a field the model cannot fill is a cap nothing downstream
//! has to remember to apply.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::types::{CompanyPlan, SectionKey, SkillGroup};

/// The `usedEvidence` property, spelled once for every section schema.
///
/// Every section answer carries this trailer: which evidence entries the model
/// says ground it. It is filtered against the grounded `EvidenceMap` in
/// `stages::section_gen`, where a citation the map does not back is DROPPED and
/// never repaired — the same philosophy as the verbatim-quote filter one stage
/// up.
fn used_evidence_property() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

/// The professional-summary paragraph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SummaryOut {
    pub summary: String,
    pub used_evidence: Vec<String>,
}

impl SummaryOut {
    pub const EXAMPLE: &'static str = r#"{
  "summary": "Backend engineer with eight years on payment platforms, most recently owning the settlement service that moves EUR 40M a month.",
  "usedEvidence": ["payments domain", "Go"]
}"#;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string" },
                "usedEvidence": used_evidence_property(),
            },
        })
    }
}

/// The skills clusters, re-ordered for THIS posting.
///
/// The groups are seeded from the strategy (which the model already grounded
/// once against the source résumé), and every surviving skill is checked
/// against the source text again in `stages::section_gen` — the model may drop
/// and reorder, never add.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillsOut {
    pub groups: Vec<SkillGroup>,
    pub used_evidence: Vec<String>,
}

impl SkillsOut {
    pub const EXAMPLE: &'static str = r#"{
  "groups": [
    { "label": "Languages", "skills": ["Go", "Rust", "TypeScript"] },
    { "label": "Infrastructure", "skills": ["Kubernetes", "Terraform"] }
  ],
  "usedEvidence": ["Kubernetes"]
}"#;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "groups": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": { "type": "string" },
                            "skills": { "type": "array", "items": { "type": "string" } },
                        },
                    },
                },
                "usedEvidence": used_evidence_property(),
            },
        })
    }
}

/// ONE employment entry's bullets.
///
/// **No identity fields, on purpose.** `company`/`title`/`dates` are seeded
/// from the [`CompanyPlan`] and rendered by `assemble`, so this type gives the
/// model nowhere to rename an employer or move a date — the failure the
/// strategy stage's re-seed exists to undo, made unreachable one stage earlier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExperienceOut {
    pub bullets: Vec<String>,
    pub used_evidence: Vec<String>,
}

impl ExperienceOut {
    pub const EXAMPLE: &'static str = r#"{
  "bullets": [
    "Migrated 40 services to Kubernetes, cutting deploy time from 25 to 4 minutes",
    "Owned the settlement service through a payment-provider migration with no customer-visible downtime"
  ],
  "usedEvidence": ["Kubernetes", "payments domain"]
}"#;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "bullets": { "type": "array", "items": { "type": "string" } },
                "usedEvidence": used_evidence_property(),
            },
        })
    }
}

/// ONE project entry, in the owner-locked source signature.
///
/// `name`, `links` and `stack` are seeded VERBATIM from the parsed source and
/// re-seeded after the answer arrives; the model authors `description` and
/// nothing else, and only when the source supplied one to ground on. A link is
/// how a reviewer verifies the work exists, which is why
/// `factual.altered_project_link` is a Critical — the type makes the Critical
/// unreachable from this path rather than merely detectable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectOut {
    pub name: String,
    pub links: Vec<String>,
    pub stack: Vec<String>,
    pub description: String,
}

/// The projects section.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectsOut {
    pub projects: Vec<ProjectOut>,
    pub used_evidence: Vec<String>,
}

impl ProjectsOut {
    pub const EXAMPLE: &'static str = r#"{
  "projects": [
    {
      "name": "CrossKit",
      "links": ["https://crosskit.dev", "https://github.com/example/crosskit"],
      "stack": ["TypeScript", "Vite"],
      "description": "A framework-agnostic component library used by three internal products."
    }
  ],
  "usedEvidence": ["TypeScript"]
}"#;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "projects": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "links": { "type": "array", "items": { "type": "string" } },
                            "stack": { "type": "array", "items": { "type": "string" } },
                            "description": { "type": "string" },
                        },
                    },
                },
                "usedEvidence": used_evidence_property(),
            },
        })
    }
}

/// The education section, as a selection over the SOURCE's own lines.
///
/// A degree, an institution and a graduation year are facts, not presentation,
/// so nothing here is written: every returned entry must match a line of the
/// source's education section or it is dropped. What the model contributes is
/// the ORDER and which entries earn space on a tailored page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EducationOut {
    pub entries: Vec<String>,
    pub used_evidence: Vec<String>,
}

impl EducationOut {
    pub const EXAMPLE: &'static str = r#"{
  "entries": ["MSc Computer Science, TU Berlin  2014 - 2016"],
  "usedEvidence": []
}"#;

    pub fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "entries": { "type": "array", "items": { "type": "string" } },
                "usedEvidence": used_evidence_property(),
            },
        })
    }
}

/// One remark from the max-depth judge.
///
/// **There is no `severity` field, and that is the whole design.** The judge is
/// Warning-only, and the cheapest way to guarantee a model never emits a
/// Critical is to give it no field to say one in — a filter can be forgotten or
/// inverted, a missing field cannot. `kind` picks the issue CODE from a closed
/// Rust table; an unrecognized kind lands on the generic note code rather than
/// inventing a new one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JudgeItem {
    pub kind: String,
    /// The section heading the remark is about — matched back to the document,
    /// never trusted as a label.
    pub section: String,
    pub note: String,
    /// The span the remark is about, so the reader can look at it.
    pub quote: String,
}

/// The judge's whole answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JudgeOut {
    pub items: Vec<JudgeItem>,
}

impl JudgeOut {
    pub const EXAMPLE: &'static str = r#"{
  "items": [
    {
      "kind": "evidence",
      "section": "Work Experience",
      "note": "This bullet claims ownership without naming what shipped; the source résumé names the settlement service.",
      "quote": "Owned critical platform work"
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
                            "kind": {
                                "type": "string",
                                "enum": ["clarity", "evidence", "tailoring"],
                            },
                            "section": { "type": "string" },
                            "note": { "type": "string" },
                            "quote": { "type": "string" },
                        },
                    },
                },
            },
        })
    }
}

// ── Seeds and results ────────────────────────────────────────────────────────

/// What ONE section is generated FROM — everything resolved from the SOURCE
/// résumé and the strategy before a single provider call is made.
///
/// The variants carry data rather than the section merely naming itself,
/// because that data is exactly what the model is NOT allowed to author: an
/// [`Experience`](Self::Experience) slot hands the renderer its identity line,
/// a [`Projects`](Self::Projects) slot hands it the links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionSeed {
    Summary,
    Skills(Vec<SkillGroup>),
    Experience(Box<CompanyPlan>),
    /// Seeded name/links/stack per project; `description` is empty until the
    /// model fills it, and stays empty for a project the source has no blurb
    /// for (the degradation ladder's lower tiers).
    Projects(Vec<ProjectOut>),
    Education(Vec<String>),
}

impl SectionSeed {
    /// The section KIND's stable name, for a content-free artifact or log line.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Skills(_) => "skills",
            Self::Experience(_) => "experience",
            Self::Projects(_) => "projects",
            Self::Education(_) => "education",
        }
    }
}

/// One section the run will generate: its wire key, the heading `assemble`
/// renders, and its seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionSlot {
    pub key: SectionKey,
    /// The heading line, resolved BEFORE the call — from the localized
    /// conventions for the standard sections, from the source's own heading for
    /// a section only the source can name.
    pub heading: String,
    pub seed: SectionSeed,
}

/// The FINISHED content of one section: the seed and the model's answer,
/// already filtered. Rendered to text by `assemble`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionBody {
    Summary(String),
    Skills(Vec<SkillGroup>),
    /// One employment entry's bullets. The identity line comes from the slot's
    /// [`SectionSeed::Experience`] plan, never from here.
    Bullets(Vec<String>),
    Projects(Vec<ProjectOut>),
    /// Education entries, each a verbatim source line.
    Lines(Vec<String>),
}

/// One finished section, plus the content-free counts the ledger reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionResult {
    pub key: SectionKey,
    pub heading: String,
    pub body: SectionBody,
    /// The citations that SURVIVED the grounded-evidence filter.
    pub used_evidence: Vec<String>,
    /// How many citations the filter dropped — the only trace a drop leaves,
    /// and a count, never the term itself (ADR-027).
    pub dropped_citations: u32,
    /// How many content items (a skill the source does not state, an education
    /// line the source does not have) the grounding filter dropped.
    pub dropped_content: u32,
    /// Whether the section came from the stage cache instead of a provider.
    pub from_cache: bool,
}

impl SectionResult {
    /// Whether the section is worth rendering at all. An empty body is a failed
    /// section, not a short one: rendering a bare heading with nothing under it
    /// produces a document that looks damaged.
    pub fn is_empty(&self) -> bool {
        match &self.body {
            SectionBody::Summary(text) => text.trim().is_empty(),
            SectionBody::Skills(groups) => groups.iter().all(|g| g.skills.is_empty()),
            SectionBody::Bullets(bullets) => bullets.is_empty(),
            SectionBody::Projects(projects) => projects.is_empty(),
            SectionBody::Lines(lines) => lines.is_empty(),
        }
    }
}

/// The filled example every section prompt shows, keyed by the seed it is for.
/// Kept beside the types so an example can never describe a shape the struct no
/// longer has.
pub fn example_for(seed: &SectionSeed) -> &'static str {
    match seed {
        SectionSeed::Summary => SummaryOut::EXAMPLE,
        SectionSeed::Skills(_) => SkillsOut::EXAMPLE,
        SectionSeed::Experience(_) => ExperienceOut::EXAMPLE,
        SectionSeed::Projects(_) => ProjectsOut::EXAMPLE,
        SectionSeed::Education(_) => EducationOut::EXAMPLE,
    }
}

/// The flat JSON Schema for one section, for the providers with native
/// constrained decoding.
pub fn schema_for(seed: &SectionSeed) -> Value {
    match seed {
        SectionSeed::Summary => SummaryOut::schema(),
        SectionSeed::Skills(_) => SkillsOut::schema(),
        SectionSeed::Experience(_) => ExperienceOut::schema(),
        SectionSeed::Projects(_) => ProjectsOut::schema(),
        SectionSeed::Education(_) => EducationOut::schema(),
    }
}

/// Every section example mentions `usedEvidence` — the one field shared by all
/// five answers, and the one a model most often omits. Asserted rather than
/// assumed, because the examples are what a non-constrained provider actually
/// follows.
#[cfg(test)]
pub(crate) fn examples() -> [&'static str; 5] {
    [
        SummaryOut::EXAMPLE,
        SkillsOut::EXAMPLE,
        ExperienceOut::EXAMPLE,
        ProjectsOut::EXAMPLE,
        EducationOut::EXAMPLE,
    ]
}

/// The `usedEvidence` fragment a section example must carry, exposed for the
/// test above rather than re-typed there.
#[cfg(test)]
pub(crate) const USED_EVIDENCE_KEY: &str = "usedEvidence";
