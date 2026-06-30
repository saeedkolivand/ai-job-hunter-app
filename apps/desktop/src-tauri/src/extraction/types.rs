use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub anchor_text: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceFormat {
    PdfText,
    PdfScanned,
    Docx,
    Image,
    PlainText,
    Html,
    Rtf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedResume {
    /// Markdown text with hyperlinks inlined as [anchor](url).
    pub text: String,
    /// All discovered hyperlinks, also returned separately for easy consumption.
    pub links: Vec<Link>,
    pub confidence: Confidence,
    pub warnings: Vec<String>,
    pub source_format: SourceFormat,
}

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("File too large ({size} bytes). Maximum allowed size is 10 MB.")]
    FileTooLarge { size: usize },

    #[error("Unsupported file type: .{ext}")]
    UnsupportedFormat { ext: String },

    #[error("Legacy .doc format is not supported. Please save the file as .docx or PDF.")]
    LegacyDoc,

    #[error(
        "PDF appears to be scanned (no extractable text). Please upload a text-based PDF or DOCX."
    )]
    ScannedPdfWithoutOcr,

    #[error("PDF extraction failed: {0}")]
    PdfError(String),

    #[error("DOCX extraction failed: {0}")]
    DocxError(String),

    #[error("Image OCR failed: {0}")]
    OcrError(String),

    #[error("File could not be read as UTF-8: {0}")]
    EncodingError(String),

    #[error("I/O error: {0}")]
    IoError(String),
}
