use std::collections::HashSet;
use std::sync::LazyLock;

use lopdf::{Dictionary, Document, Object};
use regex::Regex;
use tracing::warn;

use crate::extraction::types::{ExtractionError, ExtractedResume, Link, SourceFormat};

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

/// Extract hyperlink annotations from a PDF.
///
/// PDFs store links as `/Annot` dicts with `/Subtype /Link` and an `/A`
/// action dict containing `/URI`. These are entirely separate from the text
/// content layer — `pdf-extract` never sees them.
///
/// Structured parsing via lopdf is attempted first. When it yields nothing —
/// because lopdf could not parse the file, `/Annots` did not resolve, or the
/// `/URI` was stored in a way we could not follow — we fall back to a raw-byte
/// scan that recovers `/URI (…)` actions directly. This keeps contact links
/// (LinkedIn/GitHub/Website) flowing into the generated cover-letter header even
/// for PDFs lopdf cannot fully model.
fn extract_links(bytes: &[u8]) -> Vec<Link> {
    let mut links = match Document::load_mem(bytes) {
        Ok(doc) => extract_links_structured(&doc),
        Err(e) => {
            warn!("lopdf could not parse PDF for link extraction: {e}");
            Vec::new()
        }
    };

    if links.is_empty() {
        links = scan_raw_uris(bytes);
    }

    dedup_links(links)
}

/// Walk every page's `/Link` annotations and resolve their target URIs.
fn extract_links_structured(doc: &Document) -> Vec<Link> {
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
            let Some(url) = resolve_uri(doc, annot_dict) else {
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

/// Raw-byte fallback: scan for `/URI (…)` action strings and keep http(s) URLs.
///
/// Used only when structured parsing finds no links. The anchor text is the URL
/// itself; downstream label resolution (`url_label` / `urlToFriendlyLabel`)
/// derives the friendly name (e.g. "LinkedIn"). Note: this cannot reach URIs
/// stored inside a compressed object stream — it covers the common uncompressed
/// case where lopdf's structured traversal still fails.
fn scan_raw_uris(bytes: &[u8]) -> Vec<Link> {
    static URI_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"/URI\s*\(([^)]*)\)").unwrap());

    let text = String::from_utf8_lossy(bytes);
    let mut links = Vec::new();
    for cap in URI_RE.captures_iter(&text) {
        let url = unescape_pdf_string(&cap[1]);
        if url.starts_with("http://") || url.starts_with("https://") {
            links.push(Link {
                anchor_text: url.clone(),
                url,
            });
        }
    }
    links
}

/// Unescape a PDF literal string body: `\(` → `(`, `\)` → `)`, `\\` → `\`, etc.
fn unescape_pdf_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Dedup links by URL, preserving first-seen order.
fn dedup_links(links: Vec<Link>) -> Vec<Link> {
    let mut seen = HashSet::new();
    links
        .into_iter()
        .filter(|l| seen.insert(l.url.clone()))
        .collect()
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

    // /URI is normally a direct string literal, but some producers store it as
    // an indirect object — follow the reference before reading the string.
    // as_str returns Result<&[u8]>.
    let uri_obj = action_dict.get(b"URI").ok()?;
    let uri_bytes = match uri_obj {
        Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| o.as_str().ok())?,
        other => other.as_str().ok()?,
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_scan_recovers_uri_actions() {
        let bytes = b"<< /Subtype /Link /A << /S /URI /URI (https://github.com/jane) >> >>";
        let links = scan_raw_uris(bytes);
        assert!(
            links.iter().any(|l| l.url == "https://github.com/jane"),
            "{links:?}"
        );
    }

    #[test]
    fn raw_scan_handles_whitespace_and_dedups() {
        let bytes =
            b"/URI  (https://linkedin.com/in/jane) ... /URI(https://linkedin.com/in/jane)";
        let links = dedup_links(scan_raw_uris(bytes));
        assert_eq!(links.len(), 1, "{links:?}");
        assert_eq!(links[0].url, "https://linkedin.com/in/jane");
    }

    #[test]
    fn raw_scan_ignores_non_http_uris() {
        let bytes = b"/URI (mailto:jane@example.com) /URI (file:///etc/passwd)";
        assert!(scan_raw_uris(bytes).is_empty());
    }

    #[test]
    fn unescape_handles_escaped_parens() {
        assert_eq!(unescape_pdf_string(r"a\(b\)c"), "a(b)c");
        assert_eq!(unescape_pdf_string("plain"), "plain");
    }

    #[test]
    fn dedup_keeps_first_seen() {
        let links = vec![
            Link {
                anchor_text: "First".into(),
                url: "https://x.test".into(),
            },
            Link {
                anchor_text: "Second".into(),
                url: "https://x.test".into(),
            },
        ];
        let out = dedup_links(links);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].anchor_text, "First");
    }
}
