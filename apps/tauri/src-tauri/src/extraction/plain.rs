use crate::extraction::types::{ExtractionError, ExtractedResume, SourceFormat};

pub fn extract(bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|e| ExtractionError::EncodingError(e.to_string()))?;

    let confidence = crate::extraction::confidence::score(&text, SourceFormat::PlainText);

    Ok(ExtractedResume {
        text,
        links: vec![],
        confidence,
        warnings: vec![],
        source_format: SourceFormat::PlainText,
    })
}
