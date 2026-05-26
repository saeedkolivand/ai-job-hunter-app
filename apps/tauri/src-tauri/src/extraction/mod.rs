pub mod cleanup;
pub mod confidence;
pub mod docx;
pub mod image;
pub mod pdf;
pub mod plain;
pub mod types;

#[cfg(feature = "ocr")]
pub mod pdf_ocr;

use std::path::Path;

use tracing::{instrument, warn};

use types::{ExtractionError, ExtractedResume, SourceFormat};

const MAX_BYTES: usize = 10 * 1024 * 1024; // 10 MB

/// Tauri command — entry point for the frontend.
///
/// Returns `Ok(ExtractedResume)` on success or a user-facing error string on
/// failure. Internal details are logged server-side via `tracing`; only the
/// `Display` form of `ExtractionError` reaches the frontend.
#[tauri::command]
#[instrument(skip_all, fields(path))]
pub async fn extract_resume(path: String) -> Result<ExtractedResume, String> {
    tracing::Span::current().record("path", &path.as_str());

    let bytes = std::fs::read(&path).map_err(|e| {
        let err = ExtractionError::IoError(e.to_string());
        warn!(%err, "failed to read file");
        err.to_string()
    })?;

    route(&path, &bytes).map_err(|e| {
        warn!(error = %e, "extraction failed");
        e.to_string()
    })
}

/// Pure (non-async) router — easier to unit-test without a Tauri runtime.
pub fn route(path: &str, bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    if bytes.len() > MAX_BYTES {
        return Err(ExtractionError::FileTooLarge { size: bytes.len() });
    }

    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "pdf" => route_pdf(bytes),
        "docx" => docx::extract(bytes),
        "txt" | "md" | "markdown" => plain::extract(bytes),
        "png" | "jpg" | "jpeg" | "webp" => route_image(bytes),
        "doc" => Err(ExtractionError::LegacyDoc),
        other => Err(ExtractionError::UnsupportedFormat {
            ext: other.to_string(),
        }),
    }
}

fn route_pdf(bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    let result = pdf::extract(bytes)?;

    // Fall back to OCR when direct extraction yields too little text.
    let word_count = result.text.split_whitespace().count();
    if word_count >= 30 && result.text.len() >= 200 {
        return Ok(result);
    }

    warn!(
        word_count,
        chars = result.text.len(),
        "PDF direct extraction yielded sparse text — attempting OCR fallback"
    );

    #[cfg(feature = "ocr")]
    {
        return pdf_ocr::extract(bytes);
    }

    #[cfg(not(feature = "ocr"))]
    {
        if word_count == 0 {
            return Err(ExtractionError::ScannedPdfWithoutOcr);
        }
        // Sparse but non-empty — return what we have with a warning.
        let mut out = result;
        out.warnings.push(
            "Extracted text is sparse. The PDF may contain scanned pages. \
             Enable OCR support for better results."
                .to_string(),
        );
        out.source_format = SourceFormat::PdfScanned;
        return Ok(out);
    }
}

fn route_image(bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    #[cfg(feature = "ocr")]
    {
        return image::extract(bytes);
    }

    #[cfg(not(feature = "ocr"))]
    {
        let _ = bytes;
        Err(ExtractionError::OcrError(
            "OCR is not enabled in this build. Rebuild with --features ocr to process images."
                .to_string(),
        ))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Confidence, ExtractionError, SourceFormat};
    use std::io::Write as IoWrite;
    use zip::write::{ExtendedFileOptions, FileOptions};
    use zip::ZipWriter;

    // ── Fixture builders ──────────────────────────────────────────────────────

    /// Build a minimal DOCX buffer from paragraphs + an optional hyperlink.
    fn build_docx(paragraphs: &[&str], link: Option<(&str, &str, &str)>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: FileOptions<ExtendedFileOptions> = FileOptions::default();

            zip.start_file("[Content_Types].xml", opts.clone()).unwrap();
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
            )
            .unwrap();

            zip.start_file("_rels/.rels", opts.clone()).unwrap();
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
            )
            .unwrap();

            zip.start_file("word/_rels/document.xml.rels", opts.clone()).unwrap();
            let rels_xml = if let Some((r_id, url, _)) = link {
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="{r_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{url}" TargetMode="External"/>
</Relationships>"#
                )
            } else {
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#
                    .to_string()
            };
            zip.write_all(rels_xml.as_bytes()).unwrap();

            zip.start_file("word/document.xml", opts.clone()).unwrap();
            let mut doc = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<w:body>"#,
            );
            for para in paragraphs {
                doc.push_str(&format!("<w:p><w:r><w:t>{para}</w:t></w:r></w:p>"));
            }
            if let Some((r_id, _, anchor)) = link {
                doc.push_str(&format!(
                    r#"<w:p><w:hyperlink r:id="{r_id}"><w:r><w:t>{anchor}</w:t></w:r></w:hyperlink></w:p>"#
                ));
            }
            doc.push_str("</w:body></w:document>");
            zip.write_all(doc.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    /// Build a valid text-layer PDF using lopdf so xref offsets are correct.
    fn build_text_pdf(text: &str) -> Vec<u8> {
        use lopdf::{Document, Object, Stream, dictionary};

        let content_bytes = if text.is_empty() {
            b"".to_vec()
        } else {
            // Simple content stream: just mark the text as a string literal in a
            // text block. pdf-extract reads BT/ET blocks to recover text.
            let escaped = text
                .replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)");
            format!("BT /F1 12 Tf 50 750 Td ({escaped}) Tj ET").into_bytes()
        };

        let mut doc = Document::with_version("1.4");

        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let content_id = doc.add_object(Stream::new(dictionary! {}, content_bytes));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => dictionary! {
                "Font" => dictionary! { "F1" => font_id }
            },
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1i64,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut out = Vec::new();
        doc.save_to(&mut out).expect("lopdf save failed");
        out
    }

    // ── Plain text ────────────────────────────────────────────────────────────

    #[test]
    fn plain_txt_fixture_extracts() {
        let fixture = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/resume.txt"
        ));
        let result = route("resume.txt", fixture).expect("plain txt failed");
        assert_eq!(result.source_format, SourceFormat::PlainText);
        assert!(result.text.contains("jane.doe@example.com"), "email missing");
        assert!(result.text.contains("Acme Corp"), "experience missing");
        assert!(result.text.contains("Rust"), "skills missing");
        assert!(
            matches!(result.confidence, Confidence::High | Confidence::Medium),
            "unexpected confidence: {:?}",
            result.confidence
        );
    }

    #[test]
    fn plain_md_extracts() {
        let md = b"# Jane Doe\njane@example.com\n\n## Experience\nEngineer at Acme 2020-2024\n\n## Skills\nRust Go";
        let result = route("resume.md", md).expect("md failed");
        assert_eq!(result.source_format, SourceFormat::PlainText);
        assert!(result.text.contains("jane@example.com"));
    }

    // ── DOCX ──────────────────────────────────────────────────────────────────

    #[test]
    fn docx_extracts_paragraphs() {
        let bytes = build_docx(
            &[
                "Jane Doe",
                "jane.doe@example.com",
                "Experience",
                "Senior Engineer at Acme Corp 2020-2025",
                "Skills",
                "Rust Python TypeScript",
            ],
            None,
        );
        let result = route("resume.docx", &bytes).expect("docx failed");
        assert_eq!(result.source_format, SourceFormat::Docx);
        assert!(result.text.contains("Jane Doe"), "name missing");
        assert!(result.text.contains("jane.doe@example.com"), "email missing");
        assert!(result.text.contains("Acme Corp"), "experience missing");
    }

    #[test]
    fn docx_resolves_hyperlink() {
        let bytes = build_docx(
            &["Jane Doe", "Senior Engineer"],
            Some(("rId1", "https://linkedin.com/in/janedoe", "LinkedIn Profile")),
        );
        let result = route("resume.docx", &bytes).expect("docx hyperlink failed");
        assert_eq!(result.source_format, SourceFormat::Docx);

        let link_found = result
            .links
            .iter()
            .any(|l| l.url.contains("linkedin.com") && l.anchor_text == "LinkedIn Profile");
        assert!(link_found, "link not found: {:?}", result.links);

        assert!(
            result
                .text
                .contains("[LinkedIn Profile](https://linkedin.com/in/janedoe)"),
            "inline markdown missing:\n{}",
            result.text
        );
    }

    // ── PDF ───────────────────────────────────────────────────────────────────

    #[test]
    fn pdf_extracts_text() {
        // Content must exceed 30 words / 200 chars to pass the sparse-text threshold.
        let content = "Jane Doe jane.doe@example.com Senior Software Engineer Amsterdam \
            Experience Senior Engineer Acme Corp 2020 to 2025 led migration reduced latency \
            Education BSc Computer Science University of Amsterdam 2017 \
            Skills Rust Go Python TypeScript PostgreSQL Docker Kubernetes \
            Languages English fluent Dutch intermediate";
        let bytes = build_text_pdf(content);
        let result = route("resume.pdf", &bytes).expect("pdf failed");
        assert_eq!(result.source_format, SourceFormat::PdfText);
        assert!(result.text.contains("Jane Doe"), "name missing");
        assert!(result.text.contains("jane.doe@example.com"), "email missing");
    }

    #[test]
    fn pdf_empty_layer_without_ocr_errors_or_warns() {
        let bytes = build_text_pdf("");
        match route("resume.pdf", &bytes) {
            Err(ExtractionError::ScannedPdfWithoutOcr) => {}
            Ok(r) => {
                assert!(
                    !r.warnings.is_empty() || matches!(r.confidence, Confidence::Low),
                    "empty pdf should warn or score Low"
                );
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    // ── Guards ────────────────────────────────────────────────────────────────

    #[test]
    fn rejects_file_over_10mb() {
        let big = vec![0u8; 11 * 1024 * 1024];
        assert!(matches!(
            route("big.pdf", &big),
            Err(ExtractionError::FileTooLarge { .. })
        ));
    }

    #[test]
    fn rejects_legacy_doc() {
        assert!(matches!(
            route("old.doc", b"garbage"),
            Err(ExtractionError::LegacyDoc)
        ));
    }

    #[test]
    fn rejects_unknown_extension() {
        assert!(matches!(
            route("resume.pages", b"garbage"),
            Err(ExtractionError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    #[cfg(not(feature = "ocr"))]
    fn image_without_ocr_returns_ocr_error() {
        assert!(matches!(
            route("photo.png", b"\x89PNG fake"),
            Err(ExtractionError::OcrError(_))
        ));
    }
}
