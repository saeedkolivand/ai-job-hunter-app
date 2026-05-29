//! Canonical document model — the single structured representation a resume or
//! cover letter is rendered from. Backends (PDF / DOCX / TXT) translate this
//! model; they do not re-parse text.
//!
//! Introduced in Phase 1 (foundations) with no consumers yet: the adapter that
//! builds a `DocumentModel` from parsed text and the backends that render it
//! land in later phases.

use crate::export::types::DocumentType;

use super::rich::RichText;
use super::version::SCHEMA_VERSION;

/// Canonical, language-agnostic section identity. Display headings and locale
/// ordering are resolved elsewhere (theme / locale). `Custom` preserves any
/// heading we don't recognize so content is never dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionId {
    Summary,
    Experience,
    Education,
    Skills,
    Projects,
    Certifications,
    Languages,
    Awards,
    Publications,
    Volunteer,
    Interests,
    References,
    Custom(String),
}

impl SectionId {
    /// Classify a section heading into a canonical id. English baseline; locale
    /// profiles extend this with localized headings in a later phase. Unknown
    /// headings become [`SectionId::Custom`] so nothing is ever dropped.
    pub fn from_header(heading: &str) -> Self {
        let h = heading.trim().to_lowercase();
        let has = |needle: &str| h.contains(needle);

        if has("summary") || has("profile") || has("objective") || has("about") {
            SectionId::Summary
        } else if has("experience") || has("employment") || has("work history") {
            SectionId::Experience
        } else if has("education") || has("academic") {
            SectionId::Education
        } else if has("skill") || has("competenc") || has("technolog") {
            SectionId::Skills
        } else if has("project") {
            SectionId::Projects
        } else if has("certification") || has("certificate") || has("license") {
            SectionId::Certifications
        } else if has("language") {
            SectionId::Languages
        } else if has("award") || has("honor") || has("achievement") {
            SectionId::Awards
        } else if has("publication") || has("paper") {
            SectionId::Publications
        } else if has("volunteer") {
            SectionId::Volunteer
        } else if has("interest") || has("hobb") {
            SectionId::Interests
        } else if has("reference") {
            SectionId::References
        } else {
            SectionId::Custom(heading.trim().to_string())
        }
    }
}

/// Where a section is placed in a two-column layout. Which sections default to
/// the sidebar is a theme decision (`theme::placement_for`); this is the slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Main,
    Sidebar,
}

/// A structured entry (work role, degree, project): a title line plus optional
/// subtitle / right-aligned date and its own bullets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryBlock {
    pub title: RichText,
    pub subtitle: Option<RichText>,
    pub date: Option<String>,
    pub bullets: Vec<RichText>,
}

/// A unit of section content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// Free paragraph (summary, cover-letter body, a grouped skills line).
    Paragraph(RichText),
    /// A single bullet point not attached to an entry.
    Bullet(RichText),
    /// A structured entry with its own bullets.
    Entry(EntryBlock),
}

/// One titled section of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub id: SectionId,
    /// Heading text as it should display (canonical or as-written).
    pub heading: String,
    pub blocks: Vec<Block>,
}

/// Document header: name, optional title line, and the contact line as rich runs
/// (so emails / links are first-class, not regex-recovered at render time).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeaderBlock {
    pub name: String,
    pub title: Option<String>,
    pub contact: RichText,
}

/// The canonical resume / cover-letter model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentModel {
    /// Internal schema version; lets the model evolve without breaking the
    /// external IPC contract (`ExportRequest`). See [`super::version`].
    pub schema_version: u32,
    pub doc_type: DocumentType,
    pub header: HeaderBlock,
    pub sections: Vec<Section>,
}

impl DocumentModel {
    /// Create an empty model of the given type, stamped with the current schema version.
    pub fn new(doc_type: DocumentType) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            doc_type,
            header: HeaderBlock::default(),
            sections: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_header_classifies_common_sections() {
        assert_eq!(
            SectionId::from_header("Professional Summary"),
            SectionId::Summary
        );
        assert_eq!(
            SectionId::from_header("WORK EXPERIENCE"),
            SectionId::Experience
        );
        assert_eq!(SectionId::from_header("Education"), SectionId::Education);
        assert_eq!(
            SectionId::from_header("Technical Skills"),
            SectionId::Skills
        );
        assert_eq!(
            SectionId::from_header("Certifications"),
            SectionId::Certifications
        );
        assert_eq!(SectionId::from_header("Languages"), SectionId::Languages);
    }

    #[test]
    fn from_header_preserves_unknown_headings_as_custom() {
        assert_eq!(
            SectionId::from_header("  Speaking Engagements  "),
            SectionId::Custom("Speaking Engagements".to_string())
        );
    }

    #[test]
    fn new_stamps_current_schema_version() {
        let m = DocumentModel::new(DocumentType::Resume);
        assert_eq!(m.schema_version, SCHEMA_VERSION);
        assert!(m.sections.is_empty());
        assert_eq!(m.header, HeaderBlock::default());
    }
}
