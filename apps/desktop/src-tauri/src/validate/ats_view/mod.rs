//! "What an ATS sees": a strict, rigid-parser reading (Workday/Taleo style) of
//! the text re-extracted from an exported PDF/DOCX.
//!
//! Built from the text the round-trip gate (`validate::run_validators`)
//! already extracts — no second extraction pass on export. Pure and advisory:
//! it never blocks or fails an export; every judgement is a per-field
//! [`AtsStatus`] the UI/agent can show.
//!
//! **Reading-order heuristic** (deliberately conservative): the order is
//! `suspicious` ([`AtsReason::Interleaved`]) only when body content demonstrably
//! comes before the header block — the first non-empty line is a section
//! heading, or the candidate name / the email line sits AFTER the first
//! standard heading. That is exactly what a two-column header whose columns
//! interleave extracts as; a template that reads name, contact, then sections
//! is never flagged. The name check needs the source name (known on export);
//! for a bare file it reduces to the first two conditions.

use serde::{Deserialize, Serialize};

mod contact;
mod dates;
mod file;
mod roles;
mod text;

/// Largest exported file the file command reads (same cap as résumé import).
const MAX_FILE_BYTES: u64 = crate::extraction::MAX_BYTES as u64;

/// Most extracted text the view reads (a résumé is a few KB).
const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
/// Most section / role entries returned; a real résumé has a handful of each.
const MAX_SECTIONS: usize = 64;
const MAX_ROLES: usize = 64;

/// Whether the parser could read a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AtsStatus {
    Found,
    Missing,
    Suspicious,
}

/// Why a field is `missing` or `suspicious` (machine value, no prose).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AtsReason {
    NotFound,
    Unparseable,
    DatesInverted,
    Wrapped,
    NonstandardHeading,
    Interleaved,
}

/// One extracted field: status, the value the parser read, and the reason when not `found`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtsField {
    pub status: AtsStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<AtsReason>,
}

impl AtsField {
    pub(super) fn found(value: impl Into<String>) -> Self {
        Self {
            status: AtsStatus::Found,
            value: Some(value.into()),
            reason: None,
        }
    }
    pub(super) fn missing() -> Self {
        Self {
            status: AtsStatus::Missing,
            value: None,
            reason: Some(AtsReason::NotFound),
        }
    }
    pub(super) fn suspicious(value: Option<String>, reason: AtsReason) -> Self {
        Self {
            status: AtsStatus::Suspicious,
            value,
            reason: Some(reason),
        }
    }
}

/// The contact fields a parser reads from the header block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtsContact {
    pub name: AtsField,
    pub email: AtsField,
    pub phone: AtsField,
    pub location: AtsField,
}

/// Section bucket of a heading, language-independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AtsSectionKind {
    Summary,
    Experience,
    Education,
    Projects,
    Skills,
    Other,
}

/// A section heading as the parser sees it. A core kind (experience, education,
/// skills) that no heading maps to is reported once with `status: missing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtsSection {
    pub kind: AtsSectionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    pub status: AtsStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<AtsReason>,
}

/// One work-experience entry. `start`/`end` values are `YYYY-MM`, `YYYY`, or
/// `present` (end only); an unparseable date keeps the text as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtsRole {
    pub title: AtsField,
    pub company: AtsField,
    pub start: AtsField,
    pub end: AtsField,
}

/// Field counts over the whole view, so every consumer reports "N of M" identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtsSummary {
    pub found: u32,
    pub suspicious: u32,
    pub missing: u32,
    pub total: u32,
}

/// The strict parse view of one exported résumé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtsView {
    pub contact: AtsContact,
    pub sections: Vec<AtsSection>,
    pub roles: Vec<AtsRole>,
    /// `found` = single-column order; `suspicious` + `interleaved` = body before header.
    pub reading_order: AtsField,
    pub summary: AtsSummary,
}

/// What the caller already knows about the document (export knows the source).
#[derive(Debug, Default, Clone, Copy)]
pub struct AtsHints<'a> {
    pub name: Option<&'a str>,
}

const CORE_KINDS: [AtsSectionKind; 3] = [
    AtsSectionKind::Experience,
    AtsSectionKind::Education,
    AtsSectionKind::Skills,
];

fn reading_order(
    lines: &[String],
    first_standard: Option<usize>,
    name_idx: Option<usize>,
    email_idx: Option<usize>,
) -> AtsField {
    // No standard heading means there is no order to confirm: no false reassurance.
    let Some(h) = first_standard else {
        return AtsField::missing();
    };
    let after = |i: Option<usize>| i.is_some_and(|i| i > h);
    if h == 0 || after(name_idx) || after(email_idx) {
        let first = lines.first().cloned();
        return AtsField::suspicious(first, AtsReason::Interleaved);
    }
    AtsField {
        status: AtsStatus::Found,
        value: None,
        reason: None,
    }
}

fn summarize(view: &AtsView) -> AtsSummary {
    let c = &view.contact;
    let mut fields: Vec<&AtsField> = vec![&c.name, &c.email, &c.phone, &c.location];
    fields.push(&view.reading_order);
    for r in &view.roles {
        fields.extend([&r.title, &r.company, &r.start, &r.end]);
    }
    let mut s = AtsSummary {
        found: 0,
        suspicious: 0,
        missing: 0,
        total: 0,
    };
    let statuses = fields
        .iter()
        .map(|f| f.status)
        .chain(view.sections.iter().map(|x| x.status));
    for st in statuses {
        s.total += 1;
        match st {
            AtsStatus::Found => s.found += 1,
            AtsStatus::Suspicious => s.suspicious += 1,
            AtsStatus::Missing => s.missing += 1,
        }
    }
    s
}

/// Build the view from already-extracted text. Pure; never fails.
pub fn ats_view_from_text(extracted: &str, hints: AtsHints<'_>) -> AtsView {
    // Bounded input: the text may come from a hostile export request or file.
    let cut = (0..=MAX_TEXT_BYTES.min(extracted.len()))
        .rev()
        .find(|&i| extracted.is_char_boundary(i))
        .unwrap_or(0);
    let lines = text::split_fused_headings(text::split_lines(&extracted[..cut]));
    let first_known = text::first_standard_heading(&lines);
    let (name, name_idx) = text::find_name(&lines, hints.name, first_known);
    let headings = text::find_headings(&lines, name_idx);
    let first_standard = headings.iter().find(|h| h.standard).map(|h| h.idx);

    let (email, email_idx) = contact::find_email(&lines);
    let zone = contact::contact_zone(&lines, first_standard, email_idx, name_idx);
    let contact = AtsContact {
        name,
        email,
        phone: contact::find_phone(&zone),
        location: contact::find_location(&zone),
    };

    // Cap the DETECTED headings, then append the Missing core rows, so the cap can
    // never drop a Missing row.
    let mut sections: Vec<AtsSection> = headings
        .iter()
        .take(MAX_SECTIONS)
        .map(|h| AtsSection {
            kind: h.kind,
            heading: Some(h.text.clone()),
            status: if h.standard {
                AtsStatus::Found
            } else {
                AtsStatus::Suspicious
            },
            reason: (!h.standard).then_some(AtsReason::NonstandardHeading),
        })
        .collect();
    for kind in CORE_KINDS {
        if !headings.iter().any(|h| h.kind == kind && h.standard) {
            sections.push(AtsSection {
                kind,
                heading: None,
                status: AtsStatus::Missing,
                reason: Some(AtsReason::NotFound),
            });
        }
    }

    let mut view = AtsView {
        contact,
        sections,
        roles: {
            let mut roles = roles::extract_roles(&lines, &headings);
            roles.truncate(MAX_ROLES);
            roles
        },
        reading_order: reading_order(&lines, first_standard, name_idx, email_idx),
        summary: AtsSummary {
            found: 0,
            suspicious: 0,
            missing: 0,
            total: 0,
        },
    };
    view.summary = summarize(&view);
    view
}

pub use file::ats_view_for_file;

#[cfg(test)]
mod tests;
