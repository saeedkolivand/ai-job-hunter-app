use std::collections::HashMap;
use std::io::Read;

use zip::ZipArchive;

use crate::extraction::types::{ExtractedResume, ExtractionError, Link, SourceFormat};

pub fn extract(bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| ExtractionError::DocxError(e.to_string()))?;

    let relationships = read_relationships(&mut archive);
    let document_xml = read_zip_entry(&mut archive, "word/document.xml")?;

    let (text, links) = parse_document(&document_xml, &relationships);
    let text = crate::extraction::clean::strip_icon_glyphs(&text);
    let confidence = crate::extraction::confidence::score(&text, SourceFormat::Docx);

    Ok(ExtractedResume {
        text,
        links,
        confidence,
        warnings: vec![],
        source_format: SourceFormat::Docx,
    })
}

// ── ZIP helpers ───────────────────────────────────────────────────────────────

fn read_zip_entry(
    archive: &mut ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> Result<String, ExtractionError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| ExtractionError::DocxError(format!("missing {name} in DOCX archive")))?;
    let mut buf = String::new();
    entry
        .read_to_string(&mut buf)
        .map_err(|e| ExtractionError::DocxError(e.to_string()))?;
    Ok(buf)
}

/// Parse `word/_rels/document.xml.rels` into a map of `rId → target URL`.
fn read_relationships(archive: &mut ZipArchive<std::io::Cursor<&[u8]>>) -> HashMap<String, String> {
    let xml = match read_zip_entry(archive, "word/_rels/document.xml.rels") {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    parse_relationships(&xml)
}

// ── XML parsers ───────────────────────────────────────────────────────────────

/// Build `rId → target` from the relationships XML.
fn parse_relationships(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    // Each relationship looks like:
    //   <Relationship Id="rId1" Type="...hyperlink" Target="https://..." TargetMode="External"/>
    for segment in xml.split("<Relationship ") {
        let id = attr_value(segment, "Id");
        let target = attr_value(segment, "Target");
        let target_mode = attr_value(segment, "TargetMode");
        if let (Some(id), Some(url)) = (id, target) {
            // Only keep external hyperlinks.
            if target_mode.as_deref() == Some("External")
                && (url.starts_with("http://")
                    || url.starts_with("https://")
                    || url.starts_with("mailto:"))
            {
                map.insert(id, url);
            }
        }
    }
    map
}

/// Walk document.xml and emit text + hyperlinks.
///
/// DOCX structure that matters:
///   <w:p>                 paragraph
///     <w:hyperlink r:id="rId1">   hyperlink wrapping one or more runs
///       <w:r><w:t>text</w:t></w:r>
///     </w:hyperlink>
///     <w:r><w:t>plain run</w:t></w:r>
///   </w:p>
///
/// We do a single left-to-right pass tracking whether we're inside a
/// hyperlink element, collecting run text, then resolving the rId on exit.
fn parse_document(xml: &str, rels: &HashMap<String, String>) -> (String, Vec<Link>) {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut links: Vec<Link> = Vec::new();

    // Split on paragraph boundaries.
    for para_xml in paragraph_slices(xml) {
        let para_text = parse_paragraph(para_xml, rels, &mut links);
        let trimmed = para_text.trim().to_string();
        if !trimmed.is_empty() {
            paragraphs.push(trimmed);
        }
    }

    (paragraphs.join("\n"), links)
}

/// One slice per `<w:p …>` / `<w:p>` paragraph, each running up to the next
/// paragraph start (any leading preamble before the first `<w:p` is dropped).
///
/// Writers disagree on the spelling: MS Word emits `<w:p w:rsidR="…">` while
/// Google Docs / LibreOffice / python-docx emit a bare `<w:p>`, and a document
/// can contain both. Splitting on one spelling and chaining the other emitted
/// every paragraph TWICE for the bare form (the `"<w:p "` split matched nothing
/// and returned the whole document as a single boundary-less element), so scan
/// for the tag instead. The byte after `<w:p` must be `' '` or `'>'` — that is
/// what keeps `<w:pPr>`/`<w:pict>` from being mistaken for a paragraph.
fn paragraph_slices(xml: &str) -> Vec<&str> {
    let bytes = xml.as_bytes();
    let mut starts: Vec<usize> = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = xml[from..].find("<w:p") {
        let at = from + rel;
        let after = at + "<w:p".len();
        if matches!(bytes.get(after), Some(b' ') | Some(b'>')) {
            starts.push(at);
        }
        from = after;
    }
    starts
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = starts.get(i + 1).copied().unwrap_or(xml.len());
            &xml[start..end]
        })
        .collect()
}

fn parse_paragraph(xml: &str, rels: &HashMap<String, String>, links: &mut Vec<Link>) -> String {
    let mut out = String::new();
    let mut remaining = xml;

    while let Some(tag_start) = remaining.find('<') {
        // Emit any raw text before this tag.
        let before = &remaining[..tag_start];
        if !before.is_empty() && !looks_like_xml_noise(before) {
            // Skip — raw inter-tag content in document.xml is not human text.
        }

        let rest = &remaining[tag_start..];

        if rest.starts_with("<w:hyperlink ") {
            // Find the closing tag and handle the whole hyperlink block.
            if let Some(close_pos) = rest.find("</w:hyperlink>") {
                let block = &rest[..close_pos + "</w:hyperlink>".len()];
                let r_id = attr_value(rest.strip_prefix("<w:hyperlink ").unwrap_or(rest), "r:id");
                let anchor = collect_run_text(block);
                if let Some(id) = r_id {
                    if let Some(url) = rels.get(&id) {
                        links.push(Link {
                            anchor_text: anchor.clone(),
                            url: url.clone(),
                        });
                        out.push_str(&format!("[{anchor}]({url})"));
                        remaining = &rest[close_pos + "</w:hyperlink>".len()..];
                        continue;
                    }
                }
                // No resolved URL — emit plain text.
                out.push_str(&anchor);
                remaining = &rest[close_pos + "</w:hyperlink>".len()..];
                continue;
            }
        }

        if rest.starts_with("<w:t") {
            // Collect text content of this <w:t> element.
            if let Some(close) = rest.find("</w:t>") {
                let inner = &rest[..close];
                let text = strip_single_tag(inner);
                out.push_str(text);
                remaining = &rest[close + "</w:t>".len()..];
                continue;
            }
        }

        if rest.starts_with("<w:br") || rest.starts_with("<w:cr") {
            out.push(' ');
        }

        // Skip to end of this tag.
        if let Some(tag_end) = rest.find('>') {
            remaining = &rest[tag_end + 1..];
        } else {
            break;
        }
    }

    out
}

/// Collect all <w:t> text inside a hyperlink block.
fn collect_run_text(block: &str) -> String {
    let mut text = String::new();
    let mut rest = block;
    while let Some(pos) = rest.find("<w:t") {
        rest = &rest[pos..];
        if let Some(close) = rest.find("</w:t>") {
            let inner = &rest[..close];
            text.push_str(strip_single_tag(inner));
            rest = &rest[close + "</w:t>".len()..];
        } else {
            break;
        }
    }
    text
}

/// Get the text content from `<w:t ...>content` (after the `>` but before any close tag).
fn strip_single_tag(s: &str) -> &str {
    if let Some(pos) = s.find('>') {
        &s[pos + 1..]
    } else {
        s
    }
}

fn looks_like_xml_noise(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_whitespace() || c == '\n' || c == '\r')
}

/// Extract an XML attribute value by name from a fragment like `Id="rId1" ...`.
fn attr_value(fragment: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = fragment.find(&needle)? + needle.len();
    let end = fragment[start..].find('"')? + start;
    Some(fragment[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(inner: &str) -> String {
        format!("<w:document><w:body>{inner}</w:body></w:document>")
    }

    fn para(tag: &str, text: &str) -> String {
        format!("{tag}<w:r><w:t>{text}</w:t></w:r></w:p>")
    }

    /// Google Docs / LibreOffice / python-docx write a bare `<w:p>`. Asserting
    /// on the EXACT text (not `contains`) is the point: the old chained-split
    /// emitted every paragraph twice — once mashed with no boundaries, once
    /// per paragraph — and a `contains` assertion stayed green through it.
    #[test]
    fn bare_paragraph_tags_are_not_emitted_twice() {
        let xml = body(&format!(
            "{}{}",
            para("<w:p>", "Jane Doe"),
            para("<w:p>", "Engineer")
        ));
        let (text, _) = parse_document(&xml, &HashMap::new());
        assert_eq!(text, "Jane Doe\nEngineer");
    }

    /// The MS-Word spelling (`<w:p w:rsidR="…">`) keeps working unchanged.
    #[test]
    fn attributed_paragraph_tags_still_split() {
        let xml = body(&format!(
            "{}{}",
            para("<w:p w:rsidR=\"00A1\">", "Jane Doe"),
            para("<w:p w:rsidR=\"00A2\">", "Engineer")
        ));
        let (text, _) = parse_document(&xml, &HashMap::new());
        assert_eq!(text, "Jane Doe\nEngineer");
    }

    /// A document mixing both spellings — neither branch may drop or duplicate.
    #[test]
    fn mixed_paragraph_spellings_each_appear_once() {
        let xml = body(&format!(
            "{}{}{}",
            para("<w:p w:rsidR=\"00A1\">", "Alpha"),
            para("<w:p>", "Beta"),
            para("<w:p w:rsidR=\"00A3\">", "Gamma")
        ));
        let (text, _) = parse_document(&xml, &HashMap::new());
        assert_eq!(text, "Alpha\nBeta\nGamma");
    }

    /// `<w:pPr>` (paragraph properties) starts with `<w:p` but is not a
    /// paragraph — it must not open a new slice.
    #[test]
    fn paragraph_properties_tag_is_not_a_paragraph() {
        let xml =
            body("<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:t>Solo</w:t></w:r></w:p>");
        let (text, _) = parse_document(&xml, &HashMap::new());
        assert_eq!(text, "Solo");
    }
}
