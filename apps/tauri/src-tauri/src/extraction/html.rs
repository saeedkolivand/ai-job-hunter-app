//! HTML resume extraction.
//!
//! Resumes exported as HTML (e.g. "Save as Web Page", or a portfolio page) carry
//! their structure in tags. We drop `<script>` / `<style>` / `<head>`, turn
//! `<a href>` anchors into `[label](url)` markdown (and collect them as links),
//! convert block-level boundaries to newlines, strip the remaining tags, and
//! decode the common entities — yielding the same markdown-with-inline-links
//! shape the other extractors produce.

use std::sync::LazyLock;

use regex::Regex;

use crate::extraction::types::{ExtractedResume, ExtractionError, Link, SourceFormat};

static SCRIPT_STYLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(script|style|head)\b[^>]*>.*?</\s*(script|style|head)\s*>").unwrap()
});

/// `<a … href="URL" …>LABEL</a>` — captures URL and the inner (possibly tagged) label.
static ANCHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<a\b[^>]*\bhref\s*=\s*["']([^"']+)["'][^>]*>(.*?)</\s*a\s*>"#).unwrap()
});

/// Block-level boundaries that should become a line break.
static BLOCK_BREAK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<\s*/?\s*(p|div|br|li|tr|h[1-6]|ul|ol|table|section|header|footer|article)\b[^>]*>")
        .unwrap()
});

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]+>").unwrap());

static WS_RUN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
/// Adjacent block tags (e.g. `</div><div>`) yield several newlines; collapse a
/// run of blank/whitespace-only lines to a single break so each block is one line.
static NEWLINE_RUN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n[ \t\n]*").unwrap());

pub fn extract(bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    let html = String::from_utf8_lossy(bytes);
    let (text, links) = parse_html(&html);

    if text.trim().is_empty() {
        return Err(ExtractionError::EncodingError(
            "HTML contained no readable text".to_string(),
        ));
    }

    let confidence = crate::extraction::confidence::score(&text, SourceFormat::Html);
    Ok(ExtractedResume {
        text,
        links,
        confidence,
        warnings: vec![],
        source_format: SourceFormat::Html,
    })
}

/// Returns `(markdown_text, links)`.
fn parse_html(html: &str) -> (String, Vec<Link>) {
    // 1. Drop non-content blocks.
    let without_noise = SCRIPT_STYLE_RE.replace_all(html, " ");

    // 2. Anchors → [label](url) markdown, collecting external links.
    let mut links = Vec::new();
    let with_links = ANCHOR_RE.replace_all(&without_noise, |caps: &regex::Captures| {
        let url = caps[1].trim().to_string();
        let label = decode_entities(&TAG_RE.replace_all(&caps[2], "")).trim().to_string();
        let label = if label.is_empty() { url.clone() } else { label };
        if is_keepable_link(&url) {
            links.push(Link {
                anchor_text: label.clone(),
                url: url.clone(),
            });
            format!("[{label}]({url})")
        } else {
            label
        }
    });

    // 3. Block boundaries → newlines, then strip remaining tags.
    let broken = BLOCK_BREAK_RE.replace_all(&with_links, "\n");
    let stripped = TAG_RE.replace_all(&broken, "");

    // 4. Decode entities and normalize whitespace.
    let decoded = decode_entities(&stripped);
    let text = normalize_ws(&decoded);

    (text, links)
}

fn is_keepable_link(url: &str) -> bool {
    let u = url.trim();
    u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:")
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&middot;", "·")
        .replace("&bull;", "•")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
}

fn normalize_ws(s: &str) -> String {
    let collapsed = WS_RUN_RE.replace_all(s, " ");
    let collapsed = NEWLINE_RUN_RE.replace_all(&collapsed, "\n");
    collapsed
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_and_drops_script_style() {
        let html = r#"<html><head><title>x</title></head><body>
            <style>.a{color:red}</style>
            <h1>Jane Doe</h1>
            <script>var x = 1;</script>
            <p>Senior Engineer</p>
        </body></html>"#;
        let (text, _) = parse_html(html);
        assert!(text.contains("Jane Doe"));
        assert!(text.contains("Senior Engineer"));
        assert!(!text.contains("color:red"));
        assert!(!text.contains("var x"));
    }

    #[test]
    fn converts_anchors_to_markdown_links() {
        let html =
            r#"<p>Contact: <a href="https://linkedin.com/in/jane">LinkedIn</a> | <a href="mailto:jane@example.com">Email</a></p>"#;
        let (text, links) = parse_html(html);
        assert!(text.contains("[LinkedIn](https://linkedin.com/in/jane)"));
        assert!(text.contains("[Email](mailto:jane@example.com)"));
        assert_eq!(links.len(), 2);
        assert!(links.iter().any(|l| l.url == "https://linkedin.com/in/jane"));
    }

    #[test]
    fn block_tags_become_line_breaks() {
        let html = "<div>Experience</div><div>Acme Corp</div>";
        let (text, _) = parse_html(html);
        assert_eq!(text, "Experience\nAcme Corp");
    }

    #[test]
    fn decodes_entities() {
        let html = "<p>R&amp;D &middot; Tools&nbsp;&amp;&nbsp;Tech</p>";
        let (text, _) = parse_html(html);
        assert!(text.contains("R&D"));
        assert!(text.contains("·"));
    }

    #[test]
    fn full_extract_sets_html_source() {
        let html = b"<html><body><h1>Jane</h1><p>jane@example.com</p></body></html>";
        let r = extract(html).expect("html");
        assert_eq!(r.source_format, SourceFormat::Html);
        assert!(r.text.contains("Jane"));
    }

    #[test]
    fn empty_html_errors() {
        let html = b"<html><head></head><body></body></html>";
        assert!(extract(html).is_err());
    }
}
