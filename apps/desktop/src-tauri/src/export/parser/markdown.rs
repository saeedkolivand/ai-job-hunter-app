//! Inline Markdown: `**bold**` segment tokenizing and marker stripping.

use super::super::types::TextSegment;

/// Parse **bold** markers into text segments
pub fn parse_inline_md(line: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_bold = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '*' && chars.peek() == Some(&'*') {
            // Found ** marker
            chars.next(); // consume second *

            // Save current segment if any
            if !current.is_empty() {
                segments.push(TextSegment {
                    text: current.clone(),
                    bold: in_bold,
                });
                current.clear();
            }

            // Toggle bold state
            in_bold = !in_bold;
        } else {
            current.push(ch);
        }
    }

    // Save final segment
    if !current.is_empty() {
        segments.push(TextSegment {
            text: current,
            bold: in_bold,
        });
    }

    if segments.is_empty() {
        segments.push(TextSegment {
            text: line.to_string(),
            bold: false,
        });
    }

    segments
}

/// Strip **bold** markers and leading/trailing # heading markers from text
pub fn strip_md(text: &str) -> String {
    let no_bold = text.replace("**", "");
    let s = no_bold.trim_start_matches('#').trim_start();
    s.trim_end_matches('#').trim_end().to_string()
}
