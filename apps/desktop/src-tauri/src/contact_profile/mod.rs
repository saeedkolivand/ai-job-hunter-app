//! Contact profile — the single source of truth for the document header.
//!
//! Resumes and cover letters used to build their header contact line from links
//! scavenged out of the uploaded résumé by a domain heuristic + document
//! position, which swapped a personal LinkedIn for a company page and a personal
//! site for an employer URL (the URL-swap symptom). The header is now assembled
//! from **named fields** held here — never by index, never from the company-link
//! pool — and the same builder feeds the résumé, cover letter, and DOCX, localized
//! per language.
//!
//! Persistence mirrors [`crate::job_preferences`]: a single-row SQLite settings
//! table. Seeding from an imported résumé uses [`classify_contact_links`], which
//! picks the personal profile/site by name, rejects company / job-board pages, and
//! keeps every other personal link as a labelled extra; the import adds email /
//! phone / location from the deterministic structuring pass. The result is
//! *merged* into the stored profile via [`ContactProfile::fill_empty_from`] —
//! filling only empty fields so a sparse profile is completed while every value
//! the user edited is preserved. It is a *suggestion* the user can edit, never
//! silently trusted.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod classify;
mod conflicts;
mod header;
mod store;

pub use self::classify::{classify_contact_links, linkedin_from_text};
pub(crate) use self::classify::{is_job_board, is_personal_xing};
pub use self::conflicts::detect_contact_conflicts;
pub use self::store::ContactProfileStore;

// ── Types ───────────────────────────────────────────────────────────────────

/// A free-text value with optional per-language overrides (e.g. a location that
/// reads "Netherlands" in English documents and "Niederlande" in German ones).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedText {
    /// Value used when no language-specific override matches.
    pub default: String,
    /// ISO-639-1 (`de`, `en`, …) → localized value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_lang: BTreeMap<String, String>,
}

impl LocalizedText {
    /// Resolve for `lang` (its primary subtag), falling back to [`Self::default`].
    pub fn resolve(&self, lang: &str) -> &str {
        let primary = lang.split(['-', '_']).next().unwrap_or(lang).to_lowercase();
        self.by_lang
            .get(&primary)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.default)
    }

    fn is_empty(&self) -> bool {
        self.default.trim().is_empty() && self.by_lang.values().all(|v| v.trim().is_empty())
    }
}

/// One additional labelled link beyond the named platform fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactLink {
    pub label: String,
    pub url: String,
}

/// A single identity field where the imported résumé's value CONFLICTS with the
/// value already saved in the contact profile (both non-empty, normalized values
/// differ). The import never blocks on these — it still silently fills empty
/// fields — but the renderer surfaces them so the user can resolve each one. The
/// values reported are the ORIGINAL (un-normalized) strings so the UI shows them
/// faithfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactFieldConflict {
    /// Stable key: `email`, `phone`, `linkedin`, `github`, `website`, or
    /// `location`.
    pub field: String,
    /// The value currently saved in the profile (un-normalized).
    pub current: String,
    /// The value extracted from the imported résumé (un-normalized).
    pub suggested: String,
}

/// The header contact fields, by name. Every field is optional so a partial
/// profile still produces a valid (shorter) header. The order the header renders
/// in is fixed by [`Self::header_markdown`], not by field discovery order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LocalizedText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linkedin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_links: Vec<ContactLink>,
    /// Optional candidate photo as a `data:image/<mime>;base64,<payload>` URI
    /// produced by the photo-upload control.  Stored as-is in the JSON column;
    /// `resolve_photo` validates, sanitises, dimension-caps, and re-encodes it
    /// to PNG before embedding.  File paths are never accepted here — this field
    /// is local-only and is never sent over the network.
    /// `None` → no photo; the templates fall back gracefully.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
}

impl ContactProfile {
    /// True when there is nothing to render from (caller should fall back to the
    /// text-derived header).
    pub fn is_effectively_empty(&self) -> bool {
        let location_empty = self
            .location
            .as_ref()
            .map(LocalizedText::is_empty)
            .unwrap_or(true);
        self.email.is_none()
            && self.phone.is_none()
            && self.linkedin.is_none()
            && self.github.is_none()
            && self.website.is_none()
            && self.extra_links.is_empty()
            && location_empty
    }

    /// Fill only the **empty/None** fields of `self` from `other`, never
    /// overwriting a value the user already set, and merge in any of `other`'s
    /// extra links that `self` does not already have (by URL). This lets an
    /// import complete a sparse profile (e.g. add the résumé's email / phone /
    /// location / Dribbble) while preserving every field the user edited.
    pub fn fill_empty_from(&mut self, other: &ContactProfile) {
        fn fill(slot: &mut Option<String>, src: &Option<String>) {
            if non_empty(slot).is_none() {
                if let Some(v) = non_empty(src) {
                    *slot = Some(v.to_string());
                }
            }
        }
        fill(&mut self.full_name, &other.full_name);
        fill(&mut self.email, &other.email);
        fill(&mut self.phone, &other.phone);
        fill(&mut self.linkedin, &other.linkedin);
        fill(&mut self.github, &other.github);
        fill(&mut self.website, &other.website);

        if self
            .location
            .as_ref()
            .map(LocalizedText::is_empty)
            .unwrap_or(true)
        {
            if let Some(loc) = &other.location {
                if !loc.is_empty() {
                    self.location = Some(loc.clone());
                }
            }
        }

        for link in &other.extra_links {
            let url = link.url.trim();
            if url.is_empty() || self.extra_links.iter().any(|e| e.url.trim() == url) {
                continue;
            }
            self.extra_links.push(link.clone());
        }
    }
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests;
