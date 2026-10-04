//! Shared post-extraction text cleanup (and the link reference list every
//! format appends).
//!
//! Resumes built with icon fonts (Font Awesome and friends) embed glyphs in the
//! Unicode Private Use Area; a text extractor recovers those code points as
//! meaningless boxes. We strip them — plus the replacement char and stray C0/C1
//! control characters — so the recovered text is clean for the AI, the structured
//! pre-pass, and the renderer.

use crate::export::parser::is_private_use;
use crate::extraction::types::Link;

/// Remove Private Use Area glyphs, the replacement char, and control characters
/// (keeping the `\n` / `\t` / `\r` whitespace that carries layout).
pub fn strip_icon_glyphs(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            !(is_private_use(c)
                || c == '\u{FFFD}'
                || (c.is_control() && c != '\n' && c != '\r' && c != '\t'))
        })
        .collect()
}

/// Append `links` to `text` as a markdown reference list (`---` rule, then one
/// `- [anchor](url)` line each); `text` unchanged when there are none.
pub(super) fn append_link_reference(text: String, links: &[Link]) -> String {
    if links.is_empty() {
        return text;
    }
    let mut out = text;
    out.push_str("\n\n---\n");
    for link in links {
        out.push_str(&format!("- [{}]({})\n", link.anchor_text, link.url));
    }
    out
}

#[cfg(test)]
mod tests;
