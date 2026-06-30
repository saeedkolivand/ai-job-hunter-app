use lopdf::{Dictionary, Document, Object};
use tracing::warn;

use crate::extraction::types::{ExtractedResume, ExtractionError, Link, SourceFormat};

pub fn extract(bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    let text = pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| ExtractionError::PdfError(e.to_string()))?;
    let text = crate::extraction::clean::strip_icon_glyphs(&text);

    let links = extract_links(bytes);
    let text = inline_links(&text, &links);
    let confidence = crate::extraction::confidence::score(&text, SourceFormat::PdfText);

    Ok(ExtractedResume {
        text,
        links,
        confidence,
        warnings: vec![],
        source_format: SourceFormat::PdfText,
    })
}

/// Extract hyperlink annotations using lopdf.
///
/// PDFs store links as `/Annot` dicts with `/Subtype /Link` and an `/A`
/// action dict containing `/URI`. These are entirely separate from the text
/// content layer — `pdf-extract` never sees them.
fn extract_links(bytes: &[u8]) -> Vec<Link> {
    let doc = match Document::load_mem(bytes) {
        Ok(d) => d,
        Err(e) => {
            warn!("lopdf could not parse PDF for link extraction: {e}");
            return vec![];
        }
    };

    let mut links = Vec::new();

    for (_, page_id) in doc.get_pages() {
        // get_page_annotations returns Result<Vec<&Dictionary>>
        let annots = match doc.get_page_annotations(page_id) {
            Ok(a) => a,
            Err(_) => continue,
        };

        for annot_dict in annots {
            if !is_link_annotation(annot_dict) {
                continue;
            }
            let Some(url) = resolve_uri(&doc, annot_dict) else {
                continue;
            };
            let anchor_text = annotation_contents(annot_dict)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| url.clone());
            links.push(Link { anchor_text, url });
        }
    }

    links
}

fn is_link_annotation(dict: &Dictionary) -> bool {
    dict.get(b"Subtype")
        .and_then(|v| v.as_name())
        .map(|s| s == b"Link")
        .unwrap_or(false)
}

fn resolve_uri(doc: &Document, annot_dict: &Dictionary) -> Option<String> {
    let action_obj = annot_dict.get(b"A").ok()?;
    let action_dict: &Dictionary = match action_obj {
        Object::Dictionary(d) => d,
        Object::Reference(id) => doc.get_dictionary(*id).ok()?,
        _ => return None,
    };

    // as_str returns Result<&[u8]>
    let uri_bytes = action_dict.get(b"URI").ok().and_then(|v| v.as_str().ok())?;
    Some(String::from_utf8_lossy(uri_bytes).into_owned())
}

fn annotation_contents(dict: &Dictionary) -> Option<String> {
    let bytes = dict.get(b"Contents").ok().and_then(|v| v.as_str().ok())?;
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// Append extracted links at the end of the text as a markdown reference list.
///
/// PDF text and annotation layers use separate coordinate systems; there is no
/// reliable way to splice a link inline at exactly the right word without
/// pdfium. Appending them as a reference list is accurate and never corrupts
/// surrounding text.
fn inline_links(text: &str, links: &[Link]) -> String {
    if links.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    out.push_str("\n\n---\n");
    for link in links {
        out.push_str(&format!("- [{}]({})\n", link.anchor_text, link.url));
    }
    out
}
