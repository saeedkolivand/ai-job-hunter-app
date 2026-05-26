use crate::extraction::types::{ExtractionError, ExtractedResume};

#[cfg(feature = "ocr")]
pub fn extract(_bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    todo!("image OCR — Deliverable 3")
}
