use crate::extraction::types::ExtractionError;

#[cfg(feature = "ocr")]
pub fn extract(_bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    todo!("scanned PDF OCR — Deliverable 3")
}
