//! Shared post-extraction text cleanup.
//!
//! Resumes built with icon fonts (Font Awesome and friends) embed glyphs in the
//! Unicode Private Use Area; a text extractor recovers those code points as
//! meaningless boxes. We strip them — plus the replacement char and stray C0/C1
//! control characters — so the recovered text is clean for the AI, the structured
//! pre-pass, and the renderer.

use crate::export::parser::is_private_use;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_private_use_glyphs() {
        let input = "Email \u{f0e0} jane@example.com \u{f08c} LinkedIn";
        let out = strip_icon_glyphs(input);
        assert!(!out.contains('\u{f0e0}'));
        assert!(!out.contains('\u{f08c}'));
        assert!(out.contains("jane@example.com"));
        assert!(out.contains("LinkedIn"));
    }

    #[test]
    fn keeps_newlines_and_tabs() {
        assert_eq!(strip_icon_glyphs("a\nb\tc"), "a\nb\tc");
    }

    #[test]
    fn removes_replacement_char_and_controls() {
        assert_eq!(strip_icon_glyphs("a\u{FFFD}b\u{0007}c"), "abc");
    }
}
