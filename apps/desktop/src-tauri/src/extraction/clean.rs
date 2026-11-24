//! Shared post-extraction text cleanup (and the link reference list every
//! format appends).
//!
//! Resumes built with icon fonts (Font Awesome and friends) embed glyphs in the
//! Unicode Private Use Area; a text extractor recovers those code points as
//! meaningless boxes. We strip them — plus the replacement char and stray C0/C1
//! control characters — so the recovered text is clean for the AI, the structured
//! pre-pass, and the renderer.

use std::sync::LazyLock;

use regex::{Captures, Regex};

use crate::export::parser::is_private_use;
use crate::extraction::types::Link;

/// A run of single digits each one space apart, plus an optional `/` after it.
static KERNED_DIGITS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d(?: \d)+\b(?: ?/ ?)?").unwrap());

/// Rejoin dates a PDF text layer kerned digit by digit (`0 2 /2 0 2 3` → `02/2023`,
/// #1563), so every downstream date matcher sees the ordinary form. Only a 1-2 digit
/// month before `/`, or a 19xx/20xx year with a date separator (`/`, `-`, `–`, `—`)
/// directly beside it, is rejoined; any other run (a kerned phone number, a list of
/// single digits, a lone `2 0 2 3` in body text) is left as written.
pub fn rejoin_kerned_dates(text: &str) -> String {
    let is_sep = |c: Option<char>| matches!(c, Some('/' | '-' | '–' | '—'));
    KERNED_DIGITS
        .replace_all(text, |c: &Captures| {
            let m = c.get(0).expect("whole match");
            let run = m.as_str();
            let slash = run.trim_end().ends_with('/');
            let digits: String = run.chars().filter(char::is_ascii_digit).collect();
            let beside_sep = is_sep(text[..m.start()].trim_end().chars().last())
                || is_sep(text[m.end()..].trim_start().chars().next());
            let year = digits.len() == 4
                && (digits.starts_with("19") || digits.starts_with("20"))
                && beside_sep;
            match (slash, digits.len()) {
                (true, 1..=2) => format!("{digits}/"),
                (false, _) if year => digits,
                _ => run.to_string(),
            }
        })
        .into_owned()
}

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
