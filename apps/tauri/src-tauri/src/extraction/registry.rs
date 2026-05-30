//! Extractor registry — the single dispatch table mapping a file extension to an
//! [`Extractor`]. Adding a format is one entry here plus its module; the router
//! ([`super::route`]) carries no per-format `match`.

use std::sync::LazyLock;

use tracing::warn;

use crate::extraction::types::{ExtractedResume, ExtractionError, SourceFormat};

/// A pluggable extractor for one or more file extensions (lowercase, no dot).
pub trait Extractor: Send + Sync {
    fn extensions(&self) -> &'static [&'static str];
    fn extract(&self, bytes: &[u8]) -> Result<ExtractedResume, ExtractionError>;
}

struct PdfExtractor;
impl Extractor for PdfExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["pdf"]
    }
    fn extract(&self, bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
        let result = crate::extraction::pdf::extract(bytes)?;

        let word_count = result.text.split_whitespace().count();
        if word_count >= 30 && result.text.len() >= 200 {
            return Ok(result);
        }

        warn!(word_count, chars = result.text.len(), "PDF direct extraction yielded sparse text");

        // Empty text layer → a scanned PDF; the renderer's OCR fallback handles it.
        if word_count == 0 {
            return Err(ExtractionError::ScannedPdfWithoutOcr);
        }

        let mut out = result;
        out.warnings
            .push("Extracted text is sparse. The PDF may contain scanned pages.".to_string());
        out.source_format = SourceFormat::PdfScanned;
        Ok(out)
    }
}

struct DocxExtractor;
impl Extractor for DocxExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["docx"]
    }
    fn extract(&self, bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
        crate::extraction::docx::extract(bytes)
    }
}

struct PlainExtractor;
impl Extractor for PlainExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["txt", "md", "markdown"]
    }
    fn extract(&self, bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
        crate::extraction::plain::extract(bytes)
    }
}

struct HtmlExtractor;
impl Extractor for HtmlExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["html", "htm"]
    }
    fn extract(&self, bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
        crate::extraction::html::extract(bytes)
    }
}

struct RtfExtractor;
impl Extractor for RtfExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["rtf"]
    }
    fn extract(&self, bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
        crate::extraction::rtf::extract(bytes)
    }
}

/// Images can't be read in the Rust core — OCR runs in the renderer
/// (Tesseract.js). Returning an OCR error signals the frontend to run it.
struct ImageExtractor;
impl Extractor for ImageExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["png", "jpg", "jpeg", "webp"]
    }
    fn extract(&self, _bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
        Err(ExtractionError::OcrError(
            "Image files are read with OCR, which runs in the app.".to_string(),
        ))
    }
}

struct LegacyDocExtractor;
impl Extractor for LegacyDocExtractor {
    fn extensions(&self) -> &'static [&'static str] {
        &["doc"]
    }
    fn extract(&self, _bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
        Err(ExtractionError::LegacyDoc)
    }
}

static REGISTRY: LazyLock<Vec<Box<dyn Extractor>>> = LazyLock::new(|| {
    vec![
        Box::new(PdfExtractor),
        Box::new(DocxExtractor),
        Box::new(PlainExtractor),
        Box::new(HtmlExtractor),
        Box::new(RtfExtractor),
        Box::new(ImageExtractor),
        Box::new(LegacyDocExtractor),
    ]
});

/// The extractor registered for `ext` (lowercase, no leading dot), if any.
pub fn extractor_for(ext: &str) -> Option<&'static dyn Extractor> {
    REGISTRY
        .iter()
        .find(|e| e.extensions().contains(&ext))
        .map(|b| b.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_extension_resolves() {
        for ext in [
            "pdf", "docx", "txt", "md", "markdown", "html", "htm", "rtf", "png", "jpg", "jpeg",
            "webp", "doc",
        ] {
            assert!(extractor_for(ext).is_some(), "no extractor for .{ext}");
        }
    }

    #[test]
    fn unknown_extension_is_unregistered() {
        assert!(extractor_for("pages").is_none());
        assert!(extractor_for("").is_none());
    }

    #[test]
    fn no_extension_is_claimed_twice() {
        let mut seen = std::collections::HashSet::new();
        for e in REGISTRY.iter() {
            for ext in e.extensions() {
                assert!(seen.insert(*ext), "extension .{ext} is registered twice");
            }
        }
    }
}
