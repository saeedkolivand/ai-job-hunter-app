//! RTF resume extraction.
//!
//! A pragmatic RTF reader: it walks the control-word stream, skips the
//! non-content destinations (font/color/style tables, pictures, info), maps the
//! paragraph/line/tab/unicode/hex control words to text, and recovers `HYPERLINK`
//! field targets as links. Output is the same markdown-with-a-link-reference-list
//! shape the PDF extractor produces, so downstream code treats every format
//! uniformly.

use std::sync::LazyLock;

use regex::Regex;

use crate::extraction::types::{ExtractedResume, ExtractionError, Link, SourceFormat};
use crate::model::rich::url_label;

/// Destinations whose contents are not body text.
const SKIP_DESTINATIONS: &[&str] = &[
    "fonttbl",
    "colortbl",
    "stylesheet",
    "info",
    "pict",
    "themedata",
    "colorschememapping",
    "latentstyles",
    "listtable",
    "listoverridetable",
    "rsidtbl",
    "generator",
    "datastore",
    "mmathPr",
    "wgrffmtfilter",
    "xmlnstbl",
    "header",
    "footer",
    "headerl",
    "headerr",
    "footerl",
    "footerr",
    "headerf",
    "footerf",
    "fldinst",
];

static HYPERLINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"HYPERLINK\s+"([^"]+)""#).unwrap());

pub fn extract(bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    let rtf = String::from_utf8_lossy(bytes);
    if !rtf.trim_start().starts_with("{\\rtf") {
        return Err(ExtractionError::EncodingError(
            "not a valid RTF document".to_string(),
        ));
    }

    let text = rtf_to_text(&rtf);
    let links = extract_hyperlinks(&rtf);
    let text = append_link_reference(text, &links);

    if text.trim().is_empty() {
        return Err(ExtractionError::EncodingError(
            "RTF contained no readable text".to_string(),
        ));
    }

    let confidence = crate::extraction::confidence::score(&text, SourceFormat::Rtf);
    Ok(ExtractedResume {
        text,
        links,
        confidence,
        warnings: vec![],
        source_format: SourceFormat::Rtf,
    })
}

/// HYPERLINK field targets, de-duplicated, in first-seen order.
fn extract_hyperlinks(rtf: &str) -> Vec<Link> {
    let mut seen = std::collections::HashSet::new();
    let mut links = Vec::new();
    for caps in HYPERLINK_RE.captures_iter(rtf) {
        let url = caps[1].trim().to_string();
        if (url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:"))
            && seen.insert(url.clone())
        {
            links.push(Link {
                anchor_text: url_label(&url),
                url,
            });
        }
    }
    links
}

fn append_link_reference(text: String, links: &[Link]) -> String {
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

/// Walk the RTF control stream into plain text.
fn rtf_to_text(rtf: &str) -> String {
    let bytes = rtf.as_bytes();
    let n = bytes.len();
    let mut out = String::new();
    // One skip-flag per open group; a group is skipped if it (or an ancestor) is
    // a non-content destination.
    let mut skip_stack: Vec<bool> = vec![false];
    let mut uc_skip = 1usize; // chars to swallow after a \uN (set by \ucN)
    let mut to_swallow = 0usize;
    let mut i = 0usize;

    while i < n {
        let skipping = *skip_stack.last().unwrap_or(&false);
        let c = bytes[i];
        match c {
            b'{' => {
                skip_stack.push(skipping);
                i += 1;
            }
            b'}' => {
                skip_stack.pop();
                if skip_stack.is_empty() {
                    skip_stack.push(false);
                }
                i += 1;
            }
            b'\\' => {
                if i + 1 >= n {
                    break;
                }
                let d = bytes[i + 1];
                match d {
                    b'\\' | b'{' | b'}' => {
                        if !skipping && to_swallow == 0 {
                            out.push(d as char);
                        }
                        to_swallow = to_swallow.saturating_sub(1);
                        i += 2;
                    }
                    b'*' => {
                        // Ignorable destination — skip the whole group.
                        if let Some(top) = skip_stack.last_mut() {
                            *top = true;
                        }
                        i += 2;
                    }
                    b'\'' => {
                        // \'xx hex byte (Windows-1252). The two digits are read out
                        // of `bytes`, NOT out of `rtf`: `i` is a byte index, so
                        // `&rtf[i + 2..i + 4]` PANICS ("byte index is not a char
                        // boundary") whenever a multi-byte scalar starts at `i + 3`
                        // — e.g. `\'€`. The length guard cannot rule that out.
                        if i + 3 < n {
                            let value = Some(&bytes[i + 2..i + 4])
                                .filter(|d| d.iter().all(u8::is_ascii_hexdigit))
                                .and_then(|d| std::str::from_utf8(d).ok())
                                .and_then(|s| u8::from_str_radix(s, 16).ok());
                            match value {
                                Some(v) => {
                                    if !skipping {
                                        if to_swallow > 0 {
                                            to_swallow -= 1;
                                        } else {
                                            out.push(win1252(v));
                                        }
                                    }
                                    i += 4;
                                }
                                // Not a well-formed escape: drop only the `\'` and
                                // resume at the following character, so its bytes
                                // are not sliced in half.
                                None => i += 2,
                            }
                        } else {
                            i = n;
                        }
                    }
                    _ if d.is_ascii_alphabetic() => {
                        let mut j = i + 1;
                        while j < n && bytes[j].is_ascii_alphabetic() {
                            j += 1;
                        }
                        let word = &rtf[i + 1..j];
                        // Optional signed numeric parameter.
                        let mut k = j;
                        let neg = k < n && bytes[k] == b'-';
                        if neg {
                            k += 1;
                        }
                        let ps = k;
                        while k < n && bytes[k].is_ascii_digit() {
                            k += 1;
                        }
                        let param = if k > ps {
                            rtf[ps..k]
                                .parse::<i64>()
                                .ok()
                                .map(|v| if neg { -v } else { v })
                        } else {
                            None
                        };
                        // A single trailing space delimits the control word.
                        let mut next = k;
                        if next < n && bytes[next] == b' ' {
                            next += 1;
                        }
                        i = next;

                        if SKIP_DESTINATIONS.contains(&word) {
                            if let Some(top) = skip_stack.last_mut() {
                                *top = true;
                            }
                            continue;
                        }
                        if skipping {
                            continue;
                        }
                        match word {
                            "par" | "line" | "sect" | "page" => out.push('\n'),
                            "tab" => out.push('\t'),
                            "uc" => uc_skip = param.unwrap_or(1).max(0) as usize,
                            "u" => {
                                if let Some(code) = param {
                                    let cp = if code < 0 {
                                        (code + 65536) as u32
                                    } else {
                                        code as u32
                                    };
                                    if let Some(ch) = char::from_u32(cp) {
                                        out.push(ch);
                                    }
                                    to_swallow = uc_skip;
                                }
                            }
                            _ => {}
                        }
                    }
                    b'~' => {
                        if !skipping {
                            out.push(' ');
                        }
                        i += 2;
                    }
                    b'-' | b'_' | b':' | b'|' => i += 2, // optional hyphen, etc.
                    b'\r' | b'\n' => i += 2,             // escaped line break → ignore
                    _ => i += 2,
                }
            }
            b'\r' | b'\n' => i += 1, // raw line breaks are not text in RTF
            _ => {
                if !skipping {
                    if to_swallow > 0 {
                        to_swallow -= 1;
                    } else {
                        out.push(c as char);
                    }
                }
                i += 1;
            }
        }
    }

    normalize(&out)
}

/// Map a Windows-1252 byte to its Unicode char (ASCII passes through).
fn win1252(b: u8) -> char {
    match b {
        0x91 | 0x92 => '\'',
        0x93 | 0x94 => '"',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x85 => '…',
        0xA0 => ' ',
        _ => b as char,
    }
}

fn normalize(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `\'` followed by a multi-byte character used to slice the `str` mid-scalar
    /// (`&rtf[i + 2..i + 4]`) and panic "byte index is not a char boundary",
    /// aborting extraction of the whole résumé.
    #[test]
    fn hex_escape_before_a_multibyte_char_does_not_panic() {
        let text = rtf_to_text("{\\rtf1\\ansi Jane\\'€ Doe\\par}");
        assert!(text.contains("Jane"), "got: {text:?}");
        assert!(text.contains("Doe"), "got: {text:?}");
    }

    /// A `\'` escape at the very end of the input is truncated, not a panic.
    #[test]
    fn truncated_hex_escape_at_end_of_input_does_not_panic() {
        assert!(!rtf_to_text("{\\rtf1\\ansi Jane\\'e").contains("HYPERLINK"));
        assert!(!rtf_to_text("{\\rtf1\\ansi Jane\\'").is_empty());
    }

    /// A well-formed escape still decodes through the Windows-1252 table.
    #[test]
    fn well_formed_hex_escape_still_decodes() {
        assert!(rtf_to_text(r"{\rtf1\ansi Jos\'e9}").contains("José"));
    }

    #[test]
    fn extracts_paragraph_text() {
        let rtf = r"{\rtf1\ansi\deff0 {\fonttbl{\f0 Calibri;}} Jane Doe\par Senior Engineer\par}";
        let text = rtf_to_text(rtf);
        assert!(text.contains("Jane Doe"), "got: {text:?}");
        assert!(text.contains("Senior Engineer"));
        // font table content must not leak
        assert!(!text.contains("Calibri"));
    }

    #[test]
    fn decodes_unicode_and_hex() {
        // \u233 = é (with a '?' ANSI fallback to swallow), \'e9 = é in win-1252
        let rtf = r"{\rtf1\ansi caf\u233?\par r\'e9sum\'e9\par}";
        let text = rtf_to_text(rtf);
        assert!(text.contains("café"), "unicode escape failed: {text:?}");
        assert!(text.contains("résumé"), "hex escape failed: {text:?}");
    }

    #[test]
    fn recovers_hyperlink_fields() {
        let rtf = r#"{\rtf1 {\field{\*\fldinst{ HYPERLINK "https://linkedin.com/in/jane" }}{\fldrslt LinkedIn}}\par}"#;
        let links = extract_hyperlinks(rtf);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://linkedin.com/in/jane");
        assert_eq!(links[0].anchor_text, "LinkedIn");
        // The field instruction text (HYPERLINK "...") must not appear in body text.
        assert!(!rtf_to_text(rtf).contains("HYPERLINK"));
    }

    #[test]
    fn full_extract_sets_rtf_source_and_appends_links() {
        let rtf = br#"{\rtf1\ansi Jane Doe\par jane@example.com\par {\field{\*\fldinst{ HYPERLINK "https://janedoe.dev" }}{\fldrslt Site}}\par}"#;
        let r = extract(rtf).expect("rtf");
        assert_eq!(r.source_format, SourceFormat::Rtf);
        assert!(r.text.contains("Jane Doe"));
        assert!(r.text.contains("[janedoe.dev](https://janedoe.dev)"));
        assert_eq!(r.links.len(), 1);
    }

    #[test]
    fn rejects_non_rtf() {
        assert!(extract(b"just plain text").is_err());
    }
}
