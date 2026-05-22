use serde::{Deserialize, Serialize};

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Docx,
    Pdf,
    Txt,
}

/// Template ID for styling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplateId {
    Classic,
    Modern,
    Executive,
}

impl Default for TemplateId {
    fn default() -> Self {
        Self::Modern
    }
}

/// Document type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentType {
    Resume,
    CoverLetter,
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
}

/// Export result (binary data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub filename: String,
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
pub struct ParsedLine {
    pub kind: LineKind,
    pub raw: String,
    pub text: String,
    pub segments: Vec<TextSegment>,
    pub right_text: Option<String>, // For job entries (date)
}

/// Parsed document structure
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub lines: Vec<ParsedLine>,
    pub has_name: bool,
    pub has_contact: bool,
    pub section_count: usize,
}
