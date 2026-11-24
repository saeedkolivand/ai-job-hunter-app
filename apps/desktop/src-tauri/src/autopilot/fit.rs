//! Structured fit stored beside an AI note (#1446, slices 2 and 3): the category, the unmet and
//! to-verify must-haves, and the version stamp that says which résumé + description the note was
//! written from. All deterministic; none of it comes from the keyword score.

use serde::{Deserialize, Serialize};

use super::FoundJob;

/// How the candidate stands against the ad's requirements. First matching rule wins:
/// any unmet must-have is `ExplicitGap`; otherwise no usable résumé, no recognisable
/// requirements or a must-have the résumé cannot confirm is `Unknown`; otherwise an unmet
/// preference is `Stretch`; otherwise `SupportedFit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FitCategory {
    SupportedFit,
    Stretch,
    ExplicitGap,
    Unknown,
}

/// The wire shape of a note's structured fit, shared by the best-match rows (flattened).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteFitView {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mandatory_gaps: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub to_verify: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit_category: Option<FitCategory>,
    /// The résumé or description changed since the note was written (or it predates the stamp).
    #[serde(skip_serializing_if = "is_false")]
    pub notes_stale: bool,
}

pub(super) fn is_false(b: &bool) -> bool {
    !*b
}

/// A note with its fit, copied together so they can never come from different jobs.
#[derive(Debug, Clone, Default)]
pub struct NoteSnapshot {
    pub notes: Option<String>,
    pub fit: NoteFitView,
}

impl NoteSnapshot {
    pub fn of(job: &FoundJob) -> Option<Self> {
        let notes = Some(job.assistant_notes.clone()?);
        Some(Self {
            notes,
            fit: NoteFitView {
                mandatory_gaps: job.mandatory_gaps.clone(),
                to_verify: job.to_verify.clone(),
                fit_category: job.fit_category,
                notes_stale: job.notes_stale,
            },
        })
    }
}

/// Hex chars kept from each digest — a stamp, not an identity key.
const STAMP_HEX: usize = 12;

fn short_hash(text: &str) -> String {
    crate::documents::sha256_hex(text.trim())[..STAMP_HEX].to_string()
}

/// Version stamp of the inputs a note is written from: `<résumé hash>.<description hash>`.
pub fn notes_basis(resume: &str, description: Option<&str>) -> String {
    format!(
        "{}.{}",
        short_hash(resume),
        short_hash(description.unwrap_or(""))
    )
}

/// Label-level staleness: a note whose stamp differs from the current inputs, or that has no
/// stamp at all (written before stamping existed).
pub fn notes_stale(job: &FoundJob, resume: &str) -> bool {
    job.assistant_notes.is_some()
        && job.notes_basis.as_deref()
            != Some(notes_basis(resume, job.description.as_deref()).as_str())
}

/// Whether the notes loop should regenerate a prior job's note. Stricter than [`notes_stale`]:
/// an unstamped legacy note is only labelled, never mass-regenerated on upgrade.
pub fn needs_regeneration(prior: &FoundJob, resume: &str, description: Option<&str>) -> bool {
    prior.assistant_notes.is_some()
        && prior
            .notes_basis
            .as_deref()
            .is_some_and(|b| b != notes_basis(resume, description))
}

#[cfg(test)]
mod tests;
