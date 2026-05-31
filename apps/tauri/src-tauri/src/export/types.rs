use serde::{Deserialize, Serialize};

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Docx,
    Pdf,
    Txt,
}

/// Template ID for styling.
/// Serde uses kebab-case so "editorial-serif", "swiss-minimal", etc. round-trip
/// correctly. Single-word IDs (classic, modern, executive) are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateId {
    #[default]
    Classic,
    Modern,
    Executive,
    EditorialSerif,
    SwissMinimal,
    TwoColumn,
    MonoTechnical,
    RefinedExecutive,
    Academic,
}

/// Document type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentType {
    Resume,
    CoverLetter,
}

/// Font family selector — shared between template config and PDF renderer.
/// Defined here (types) to avoid a circular dependency between templates ↔ pdf_renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamily {
    Calibri,
    Inter,
    SourceSerif4,
    Manrope,
    JetBrainsMono,
    PlayfairDisplay,
}

/// Metadata for generation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationMeta {
    pub candidate_name: Option<String>,
    pub job_title: Option<String>,
    pub company_name: Option<String>,
    pub target_language: Option<String>,
}

/// Export request from frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub text: String,
    pub format: ExportFormat,
    pub document_type: DocumentType,
    pub template_id: TemplateId,
    pub meta: Option<GenerationMeta>,
    /// Linearize two-column layouts for ATS parsers.
    /// Defaults to false so existing frontends that omit it keep working.
    #[serde(default)]
    pub ats_mode: bool,
    /// Target market id (`us`, `uk`, `de`, …) resolved by
    /// [`crate::locale::LocaleProfile::get`]; drives the page size (US → Letter,
    /// the rest → A4). Optional so frontends that omit it keep the A4 default.
    #[serde(default)]
    pub locale: Option<String>,
    /// User contact profile — the single source of truth for the header contact
    /// line (name fields → `[Label](url)`), localized per language. When present
    /// it overrides whatever links the generated text carried, so a personal
    /// LinkedIn / Website can never be displaced by a company-link. Optional so
    /// older frontends keep the text-derived header.
    #[serde(default)]
    pub contact: Option<crate::contact_profile::ContactProfile>,
}

impl ExportRequest {
    /// Page geometry for this request's locale (international A4 by default).
    pub fn page_geometry(&self) -> crate::locale::PageGeometry {
        crate::locale::LocaleProfile::get(self.locale.as_deref().unwrap_or("en")).page_geometry()
    }

    /// Language the document is written in (drives the localized header location).
    /// Falls back to the request locale, then `en`.
    pub fn target_lang(&self) -> String {
        self.meta
            .as_ref()
            .and_then(|m| m.target_language.as_deref())
            .filter(|s| !s.is_empty())
            .or(self.locale.as_deref())
            .unwrap_or("en")
            .to_string()
    }
}

/// Export result (binary data)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub filename: String,
    /// Pre-export validation report (PDF/DOCX). `None` for TXT, which has no
    /// layout to validate. Optional on the wire so older frontends keep working.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<crate::validate::ExportReport>,
}

/// Line kind in parsed document
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Name,
    Contact,
    SectionHeader,
    JobEntry,
    JobTitle,
    Bullet,
    Text,
    Blank,
}

/// Text segment with optional bold formatting
#[derive(Debug, Clone)]
pub struct TextSegment {
    pub text: String,
    pub bold: bool,
}

/// Parsed line with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParsedLine {
    pub kind: LineKind,
    pub raw: String,
    pub text: String,
    pub segments: Vec<TextSegment>,
    pub right_text: Option<String>, // For job entries (date)
}

/// Parsed document structure
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParsedDocument {
    pub lines: Vec<ParsedLine>,
    pub has_name: bool,
    pub has_contact: bool,
    pub section_count: usize,
}
