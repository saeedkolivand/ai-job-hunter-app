//! Unicode / Markdown-emphasis / dash-typography normalization passes, run
//! over raw résumé text before line-by-line parsing.

use regex::Regex;
use std::sync::LazyLock;

/// Replace Unicode characters that embedded PDF fonts cannot render with safe
/// ASCII or Latin-1 equivalents. Applied before any text hits the PDF renderer.
pub fn normalize_unicode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let replacement: &str = match ch {
            // Dashes / hyphens. En-dash (U+2013) and em-dash (U+2014) are PRESERVED
            // (the bundled fonts contain both glyphs — asserted by a unit test); the
            // later `typography` pass normalizes their spacing. Collapsing them to a
            // bare hyphen used to mangle sentence-break dashes into "word- word".
            '\u{2010}' | '\u{2011}' | '\u{2012}' => "-", // hyphen / non-breaking hyphen / figure dash
            '\u{2015}' => "\u{2014}",                    // horizontal bar → em-dash
            '\u{2212}' => "-",                           // minus sign
            // Quotes
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => "\"", // double quotes
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => "'", // single quotes / apostrophes
            '\u{2032}' => "'",                                        // prime
            '\u{2033}' => "\"",                                       // double prime
            // Spaces / invisible chars
            '\u{00A0}' | '\u{202F}' | '\u{2007}' | '\u{2008}' => " ", // non-breaking / narrow no-break / figure / punctuation space
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => "",  // zero-width spaces / BOM
            '\u{00AD}' => "-",                                        // soft hyphen
            // Ellipsis
            '\u{2026}' => "...",
            // Bullets / symbols
            '\u{2022}' | '\u{2023}' | '\u{2043}' | '\u{204C}' | '\u{204D}' => "-", // bullet variants
            '\u{25E6}' | '\u{2219}' | '\u{22C5}' => "-", // white bullet / bullet operator / dot operator
            // Arrows
            '\u{2192}' => "->",
            '\u{2190}' => "<-",
            '\u{2194}' => "<->",
            '\u{21D2}' => "=>",
            '\u{2191}' => "^",
            '\u{2193}' => "v",
            // Trademark / legal
            '\u{2122}' => "(TM)",
            '\u{00AE}' => "(R)",
            '\u{00A9}' => "(c)",
            // Multiplication / fractions
            '\u{00D7}' => "x",
            '\u{00F7}' => "/",
            '\u{00BD}' => "1/2",
            '\u{00BC}' => "1/4",
            '\u{00BE}' => "3/4",
            // Superscripts
            '\u{00B2}' => "2",
            '\u{00B3}' => "3",
            '\u{00B9}' => "1",
            // Other
            '\u{2116}' => "No.",
            '\u{2020}' | '\u{2021}' => "", // daggers — drop (never emit a stray asterisk)
            '\u{00B7}' => ".",             // middle dot
            // Private Use Area + icon-font glyphs + replacement char: these render
            // as boxes/garbage (or nothing) in the bundled fonts — drop them.
            c if is_private_use(c) || c == '\u{FFFD}' => "",
            // C0/C1 control characters other than the whitespace we keep.
            c if c.is_control() && c != '\n' && c != '\r' && c != '\t' => "",
            _ => {
                out.push(ch);
                continue;
            }
        };
        out.push_str(replacement);
    }
    out
}

/// Strip stray Markdown emphasis the model occasionally leaks (`*React`, `AWS*`,
/// `AWS*-Services`) WITHOUT touching valid `**bold**` runs (the renderer turns those
/// into real bold), in-word punctuation like `snake_case`, or literal `*`/`` ` ``
/// that sit between two word characters (e.g. `5*4`, `a*b`). Runs after
/// [`normalize_unicode`], before any Markdown parsing.
pub fn sanitize_markdown(text: &str) -> String {
    // Strategy:
    // 1. Protect bold pairs (**…**) with a sentinel so the single-* pass ignores them.
    // 2. Walk the guarded string; keep a `*` or `` ` `` only when it is flanked by a
    //    word character on BOTH sides (i.e. it is a literal mid-word character like
    //    `5*4`). A marker at a word boundary (emphasis position) is dropped.
    // 3. Restore bold markers.
    const BOLD: &str = "\u{0}B\u{0}";
    let guarded = text.replace("**", BOLD);
    let chars: Vec<char> = guarded.chars().collect();
    let mut out = String::with_capacity(guarded.len());
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '*' || ch == '`' {
            let prev_word = i > 0 && is_word_char(chars[i - 1]);
            let next_word = i + 1 < chars.len() && is_word_char(chars[i + 1]);
            if prev_word && next_word {
                out.push(ch); // literal mid-word char — preserve
            }
            // else: emphasis-position marker — drop
        } else {
            out.push(ch);
        }
    }
    out.replace(BOLD, "**")
}

/// Returns `true` when `c` is an ASCII word character (`[A-Za-z0-9_]`).
/// Mirrors the `\w` class that the `regex` crate would use in ASCII mode.
/// `pub(crate)` so `validate` can import this instead of duplicating it.
#[inline]
pub(crate) fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Typography pass for dash usage. With en/em-dashes preserved by
/// [`normalize_unicode`], normalize clause-level dash spacing to a spaced en-dash
/// (" – ") and rewrite the residual ASCII "word- word" sentence-break pattern the
/// same way — but never a German suspended hyphen (`Backend- und …`) or a tight
/// compound / range (`2020–2023`, `state-of-the-art`).
pub fn typography(text: &str) -> String {
    let out = HYPHEN_BREAK_RE.replace_all(text, |c: &regex::Captures| {
        let prev = &c[1];
        let next = &c[2];
        if SUSPENDED_HYPHEN_WORDS.contains(&next.to_lowercase().as_str()) {
            format!("{prev}- {next}")
        } else {
            format!("{prev} \u{2013} {next}")
        }
    });
    DASH_CLAUSE_SPACING_RE
        .replace_all(&out, " \u{2013} ")
        .into_owned()
}

/// A complete word, an ASCII hyphen, a space, then the next word — a sentence-break
/// hyphen the model sometimes emits instead of a dash. (`e-mail`, `state-of-the-art`
/// have no space after the hyphen and never match.)
static HYPHEN_BREAK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\p{L})- (\p{L}[\p{L}.]*)").unwrap());

/// An en/em-dash used between clauses (a space on at least one side) → cleanly spaced
/// en-dash. A tight range like `2020–2023` has no surrounding space and is left alone.
static DASH_CLAUSE_SPACING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[\u{2013}\u{2014}]\s+|\s+[\u{2013}\u{2014}]\s*").unwrap());

/// German suspended-hyphen continuations: `Backend- und Frontend-…` is correct German
/// and must keep its hyphen rather than become a dash.
const SUSPENDED_HYPHEN_WORDS: &[&str] = &[
    "und",
    "oder",
    "bzw",
    "sowie",
    "als",
    "wie",
    "bis",
    "beziehungsweise",
    "respektive",
];

/// Unicode Private Use Area code points (BMP + planes 15/16). Icon fonts
/// (Font Awesome, etc.) map glyphs here, so extracted/pasted text often contains
/// PUA code points that are meaningless without the original font.
pub fn is_private_use(c: char) -> bool {
    matches!(
        c as u32,
        0xE000..=0xF8FF | 0xF_0000..=0xF_FFFD | 0x10_0000..=0x10_FFFD
    )
}
