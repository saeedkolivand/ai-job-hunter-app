//! Structured resume extraction.
//!
//! Turns the flat [`ExtractedResume`] text into a typed [`StructuredResume`]:
//! contact fields (name / email / phone / location / links) and a section
//! inventory, each carrying a [`Confidence`] and the byte span it was found at.
//!
//! This is the **deterministic pre-pass** the plan describes. It runs offline,
//! is fully testable, and never drops content. An AI structuring stage can later
//! refine the body into rich entries; for contact fields the deterministic result
//! wins (a regex-matched email is more trustworthy than an LLM guess). The whole
//! document [`Confidence`] from [`crate::extraction::confidence`] stays as the
//! fast gate; this adds the per-field detail a review UI needs.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::extraction::types::{Confidence, ExtractedResume};
use crate::model::adapter::model_from_resume_text;
use crate::model::document::{DocumentModel, HeaderBlock, SectionId};
use crate::model::rich::tokenize_rich;

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap());

/// Phone numbers: at least 7 digits, optional `+`, spaces, dashes, parens.
static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\+?\d[\d\s().\-]{6,}\d").unwrap()
});

/// A byte span `[start, end)` into the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// One extracted field: its value, how sure we are, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field<T> {
    pub value: T,
    pub confidence: Confidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_span: Option<Span>,
}

impl<T> Field<T> {
    fn new(value: T, confidence: Confidence, source_span: Option<Span>) -> Self {
        Self { value, confidence, source_span }
    }
}

/// A detected section, for the review inventory (not the full block tree).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionSummary {
    /// Heading as written.
    pub heading: String,
    /// Canonical section kind (`experience`, `skills`, `custom`, …).
    pub kind: String,
    pub confidence: Confidence,
}

/// Typed view of a resume with per-field confidence, for the review step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredResume {
    pub name: Field<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Field<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Field<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Field<String>>,
    pub links: Vec<Field<String>>,
    pub sections: Vec<SectionSummary>,
    /// Whole-document confidence (the fast gate).
    pub overall: Confidence,
    /// True when a human should review before generating (low confidence or a
    /// missing critical field). Never blocks — just flags.
    pub review_required: bool,
    /// Plain-language notes about what is missing or uncertain.
    pub warnings: Vec<String>,
}

/// Run the deterministic structuring pre-pass over an extraction result.
pub fn structure(extracted: &ExtractedResume) -> StructuredResume {
    let text = &extracted.text;

    let name = extract_name(text);
    let email = find_span(text, &EMAIL_RE).map(|(v, s)| Field::new(v, Confidence::High, Some(s)));
    let phone = find_span(text, &PHONE_RE)
        .filter(|(v, _)| v.chars().filter(|c| c.is_ascii_digit()).count() >= 7)
        .map(|(v, s)| Field::new(v.trim().to_string(), Confidence::Medium, Some(s)));
    let location = extract_location(text);

    // Links come from real hyperlinks (high confidence); fall back to bare URLs.
    let links = extracted
        .links
        .iter()
        .map(|l| {
            let span = find_literal(text, &l.url);
            Field::new(l.url.clone(), Confidence::High, span)
        })
        .collect::<Vec<_>>();

    let sections = section_inventory(text);

    let mut warnings = Vec::new();
    if name.value.trim().is_empty() {
        warnings.push("No name was detected — add it before generating.".to_string());
    }
    if email.is_none() {
        warnings.push("No email address was detected.".to_string());
    }
    if sections.len() < 2 {
        warnings.push("Few resume sections were detected — the layout may be hard to read.".to_string());
    }

    let review_required = matches!(extracted.confidence, Confidence::Low)
        || name.value.trim().is_empty()
        || email.is_none()
        || sections.len() < 2;

    StructuredResume {
        name,
        email,
        phone,
        location,
        links,
        sections,
        overall: extracted.confidence,
        review_required,
        warnings,
    }
}

/// Build a [`DocumentModel`] from the structured fields, reconciling the header
/// with the deterministic contact fields (which win over the text round-trip).
///
/// The body sections come from the canonical text adapter (which already groups
/// entries / bullets); the header is rebuilt from the typed contact fields so the
/// name and links are the trustworthy regex/hyperlink values, not re-parsed text.
///
/// The structured render path that consumes this (export building the model from
/// `StructuredResume` rather than re-parsing text) lands in a follow-up; the
/// reconcile is implemented and tested here so that wiring is a one-line swap.
#[allow(dead_code)]
pub fn build_model(extracted: &ExtractedResume, structured: &StructuredResume) -> DocumentModel {
    let mut model = model_from_resume_text(&extracted.text);
    model.header = reconcile_header(&model.header, structured);
    model
}

#[allow(dead_code)]
fn reconcile_header(parsed: &HeaderBlock, sr: &StructuredResume) -> HeaderBlock {
    let name = if sr.name.value.trim().is_empty() {
        parsed.name.clone()
    } else {
        sr.name.value.clone()
    };

    // Rebuild the contact line from the typed fields, preserving links as runs.
    let mut parts: Vec<String> = Vec::new();
    if let Some(email) = &sr.email {
        parts.push(email.value.clone());
    }
    if let Some(phone) = &sr.phone {
        parts.push(phone.value.clone());
    }
    if let Some(loc) = &sr.location {
        parts.push(loc.value.clone());
    }
    for link in &sr.links {
        parts.push(link.value.clone());
    }

    let contact = if parts.is_empty() {
        parsed.contact.clone()
    } else {
        tokenize_rich(&parts.join(" · "))
    };

    HeaderBlock { name, title: parsed.title.clone(), contact }
}

// ── Field extractors ──────────────────────────────────────────────────────────

fn extract_name(text: &str) -> Field<String> {
    let model = model_from_resume_text(text);
    let name = model.header.name.trim().to_string();
    if name.is_empty() {
        return Field::new(String::new(), Confidence::Low, None);
    }
    // A real name is short and free of email/digit noise.
    let words = name.split_whitespace().count();
    let looks_like_name =
        words <= 5 && !name.contains('@') && !name.chars().any(|c| c.is_ascii_digit());
    let confidence = if looks_like_name { Confidence::High } else { Confidence::Medium };
    let span = find_literal(text, &name);
    Field::new(name, confidence, span)
}

/// Best-effort location: a short, comma-bearing line near the top that isn't the
/// contact/email line. Low confidence by nature.
fn extract_location(text: &str) -> Option<Field<String>> {
    for line in text.lines().take(6) {
        let l = line.trim();
        if l.len() < 3 || l.len() > 60 || !l.contains(',') {
            continue;
        }
        if l.contains('@') || EMAIL_RE.is_match(l) || PHONE_RE.is_match(l) || l.contains("](") {
            continue;
        }
        // Two short comma-separated tokens look like "City, Country".
        let tokens: Vec<&str> = l.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
        if tokens.len() == 2 && tokens.iter().all(|t| t.split_whitespace().count() <= 3) {
            let span = find_literal(text, l);
            return Some(Field::new(l.to_string(), Confidence::Low, span));
        }
    }
    None
}

fn section_inventory(text: &str) -> Vec<SectionSummary> {
    model_from_resume_text(text)
        .sections
        .iter()
        .filter(|s| !s.heading.trim().is_empty()) // skip the untitled preamble
        .map(|s| {
            let confidence = match s.id {
                SectionId::Custom(_) => Confidence::Medium,
                _ => Confidence::High,
            };
            SectionSummary {
                heading: s.heading.clone(),
                kind: section_kind(&s.id),
                confidence,
            }
        })
        .collect()
}

fn section_kind(id: &SectionId) -> String {
    match id {
        SectionId::Summary => "summary",
        SectionId::Experience => "experience",
        SectionId::Education => "education",
        SectionId::Skills => "skills",
        SectionId::Projects => "projects",
        SectionId::Certifications => "certifications",
        SectionId::Languages => "languages",
        SectionId::Awards => "awards",
        SectionId::Publications => "publications",
        SectionId::Volunteer => "volunteer",
        SectionId::Interests => "interests",
        SectionId::References => "references",
        SectionId::Custom(_) => "custom",
    }
    .to_string()
}

// ── span helpers ──────────────────────────────────────────────────────────────

fn find_span(text: &str, re: &Regex) -> Option<(String, Span)> {
    re.find(text).map(|m| {
        (
            m.as_str().to_string(),
            Span { start: m.start(), end: m.end() },
        )
    })
}

fn find_literal(text: &str, needle: &str) -> Option<Span> {
    if needle.is_empty() {
        return None;
    }
    text.find(needle).map(|start| Span { start, end: start + needle.len() })
}

#[cfg(test)]
mod tests;
