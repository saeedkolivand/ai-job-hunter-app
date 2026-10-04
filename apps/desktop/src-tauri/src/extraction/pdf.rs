use std::borrow::Cow;
use std::sync::LazyLock;

use lopdf::{Dictionary, Document, Object};
use regex::Regex;
use tracing::warn;

use crate::extraction::clean::append_link_reference;
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
    Some(pdf_text_string(uri_bytes))
}

fn annotation_contents(dict: &Dictionary) -> Option<String> {
    let bytes = dict.get(b"Contents").ok().and_then(|v| v.as_str().ok())?;
    Some(pdf_text_string(bytes))
}

/// Decode a PDF **text string** (PDF 32000-1 §7.9.2.2).
///
/// A PDF text string is NOT UTF-8. It is either PDFDocEncoded, or UTF-16BE
/// behind a `FE FF` byte-order mark — and Typst, LaTeX and Word all emit the
/// UTF-16BE form for annotation `/Contents`. Reading those bytes as UTF-8 turns
/// a link anchor into the BOM rendered as `��` followed by every ASCII
/// character interleaved with its NUL high byte. That mojibake then travels:
/// the extractor writes it into the `\n---\n` reference block, and
/// `packages/prompts/src/generate/links` can no longer match the anchor to a
/// project title, so every link falls through to the unmatched-links append
/// path and lands in a block of its own instead of on its project.
///
/// The BOM-less case is genuinely ambiguous — PDFDocEncoding and UTF-8 share the
/// ASCII range and disagree above it, and nothing in the bytes says which one a
/// producer meant. Resolved by trying UTF-8 strictly first and falling back to
/// PDFDocEncoding: a byte sequence that is valid UTF-8 is overwhelmingly likely
/// to BE UTF-8 (spec-defiant producers are common), while `0xE9` alone is not
/// valid UTF-8 and can only have meant PDFDocEncoding's `é`. Decoding blindly
/// either way corrupts the other population.
pub(crate) fn pdf_text_string(bytes: &[u8]) -> String {
    match bytes {
        [0xFE, 0xFF, rest @ ..] => decode_utf16(rest, true),
        // Not spec-legal for PDF text strings, but real producers emit it —
        // decode it rather than hand the user mojibake.
        [0xFF, 0xFE, rest @ ..] => decode_utf16(rest, false),
        // PDF 2.0 additionally allows UTF-8 behind a BOM.
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => decode_pdf_doc_encoding(bytes),
        },
    }
}

/// PDFDocEncoding → UTF-8 (PDF 32000-1 Annex D.2).
///
/// Latin-1 for `0x20..=0x7E` and `0xA0..=0xFF`, which is the whole range a link
/// anchor realistically occupies. The `0x18..=0x1F` and `0x80..=0x9F` bands hold
/// typography (dashes, curly quotes, dagger, bullet…) that Latin-1 maps
/// differently, so those are spelled out; PDFDocEncoding leaves `0x7F`, `0x9F`
/// and `0xAD` undefined, which become U+FFFD rather than silently inventing a
/// character.
fn decode_pdf_doc_encoding(bytes: &[u8]) -> String {
    /// Annex D.2, `0x18..=0x1F` (8 entries) — accents.
    const ACCENTS: [char; 8] = [
        '\u{02D8}', '\u{02C7}', '\u{02C6}', '\u{02D9}', // breve caron circumflex dotaccent
        '\u{02DD}', '\u{02DB}', '\u{02DA}', '\u{02DC}', // hungarumlaut ogonek ring tilde
    ];
    /// Annex D.2, `0x80..=0x9E` (31 entries) — typography and Latin extras.
    const TYPOGRAPHY: [char; 31] = [
        '\u{2022}', '\u{2020}', '\u{2021}', '\u{2026}', // • † ‡ …        0x80
        '\u{2014}', '\u{2013}', '\u{0192}', '\u{2044}', // — – ƒ ⁄        0x84
        '\u{2039}', '\u{203A}', '\u{2212}', '\u{2030}', // ‹ › − ‰        0x88
        '\u{201E}', '\u{201C}', '\u{201D}', '\u{2018}', // „ " " '        0x8C
        '\u{2019}', '\u{201A}', '\u{2122}', '\u{FB01}', // ' ‚ ™ ﬁ        0x90
        '\u{FB02}', '\u{0141}', '\u{0152}', '\u{0160}', // ﬂ Ł Œ Š        0x94
        '\u{0178}', '\u{017D}', '\u{0131}', '\u{0142}', // Ÿ Ž ı ł        0x98
        '\u{0153}', '\u{0161}', '\u{017E}', // œ š ž                       0x9C
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x18..=0x1F => ACCENTS[(b - 0x18) as usize],
            0x80..=0x9E => TYPOGRAPHY[(b - 0x80) as usize],
            0x7F | 0x9F | 0xAD => char::REPLACEMENT_CHARACTER, // undefined in PDFDocEncoding
            _ => b as char,                                    // Latin-1 elsewhere
        })
        .collect()
}

/// Decode UTF-16 code units, substituting U+FFFD for unpaired surrogates. A
/// trailing odd byte is dropped — it cannot be part of a valid code unit.
fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let units = bytes.chunks_exact(2).map(|pair| {
        let pair = [pair[0], pair[1]];
        if big_endian {
            u16::from_be_bytes(pair)
        } else {
            u16::from_le_bytes(pair)
        }
    });
    char::decode_utf16(units)
        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// One-shot repair for `documents.text` / `ai_generations.resume_text` /
/// `ai_generations.cover_letter_text` rows written **before** [`pdf_text_string`]
/// existed (PR #955). This is NOT part of any live extraction path — it only
/// undoes damage already sitting in the store, called exclusively from the
/// one-time DB migrations `documents::DocumentStore::MIGRATIONS` and
/// `ai_generations::AiGenerationStore::MIGRATIONS`, and defensively from the
/// stores' insert/save paths (so restoring an old backup bundle — which still
/// carries the corruption, `serde_json` round-trips a NUL intact — can't
/// re-inject it into an already-migrated store).
///
/// Before that fix, a PDF text string's raw bytes were decoded with
/// `String::from_utf8_lossy`. For the common producer (UTF-16BE behind a
/// `FE FF` BOM), that turns each BOM byte — individually invalid UTF-8 — into
/// its own U+FFFD, while a UTF-16BE code unit whose high byte is ASCII
/// (`0x00`) survives as two independently-valid single-byte UTF-8 chars: the
/// high byte decodes to U+0000 (NUL) and the low byte to the intended ASCII
/// char. The stored result looks like `- [` + `\u{FFFD}\u{FFFD}` (the mangled
/// BOM) + `\0a\0i\0j\0o\0b…` (the text, NUL-interleaved) + a clean,
/// never-corrupted `](url)` suffix (added by the markdown link builder, not
/// the byte decode).
///
/// **Why a global, document-wide strip is safe for PDF/DOCX but not HTML/RTF.**
/// `extraction::clean::strip_icon_glyphs` — called by `pdf::extract` and
/// `docx::extract` *before* [`inline_links`] appends the link tail — already
/// removes every U+FFFD and control char from the body text. So for a
/// PDF/DOCX-sourced row, the appended link tail is the ONLY place a NUL or a
/// genuine BOM-derived U+FFFD pair can survive, which is what made a blind
/// document-wide strip look safe. `extraction::html` and `extraction::rtf`
/// decode with `from_utf8_lossy` too but never call `strip_icon_glyphs`, so a
/// legitimate lone or paired U+FFFD can appear ANYWHERE in one of their rows
/// — this function does not assume a PDF/DOCX-shaped row and is scoped
/// narrowly enough (below) to stay correct for those too.
///
/// **Scoping.** Only rewrites a span that starts with the doubled-U+FFFD
/// immediately followed by a NUL (`\u{FFFD}\u{FFFD}\0`) — the reliable
/// signature of "mangled BOM, then the first code unit's NUL high byte" —
/// and only for as long as the following code units keep pairing as
/// (NUL, ASCII `< 0x80`).
/// - A genuine, unrelated doubled U+FFFD elsewhere in the row (not
///   immediately followed by a NUL) is left untouched: the marker check is
///   what keeps `"Berlin\u{FFFD}\u{FFFD}Germany\0"` from being fused into
///   `"BerlinGermany"`.
/// - A code unit whose high byte is non-zero (CJK, anything `>= U+0100`)
///   breaks the (NUL, ASCII) pairing immediately, so the span simply stops
///   there instead of guessing at a reconstruction. An exotic row is left
///   detectably corrupt (its U+FFFDs stay visible) rather than silently
///   reassembled into plausible-but-wrong text — recoverable later beats
///   silently wrong now.
/// - This function touches ONLY characters inside a recognized span. Any
///   other character — including a stray NUL that isn't part of one — is
///   left exactly as stored, even in the same row as a recognized span.
///   Deliberately conservative: an ASCII-prefixed anchor that transitions
///   into non-ASCII mid-run (e.g. `gi` then a CJK char then `hub`) has its
///   safe `gi` prefix recovered, but the leftover NULs past that point are
///   left in place rather than guessed at or blanket-stripped — leaving the
///   row unambiguously, mechanically detectable as still-corrupt (a future,
///   smarter repair — or a human — can find it again by the very NULs this
///   function refused to touch) instead of risking a plausible-looking but
///   silently WRONG reconstruction.
///
/// Returns `Cow::Borrowed` unchanged whenever nothing above actually
/// applies — including the overwhelmingly common already-clean case (no NUL
/// at all) — so callers pay no allocation.
pub(crate) fn repair_utf16_mojibake(s: &str) -> Cow<'_, str> {
    if !s.contains('\0') {
        return Cow::Borrowed(s);
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut changed = false;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\u{FFFD}'
            && chars.get(i + 1) == Some(&'\u{FFFD}')
            && chars.get(i + 2) == Some(&'\0')
        {
            // Greedily consume (NUL, ASCII) pairs — the shape produced when
            // every UTF-16BE code unit in the run had a `0x00` high byte.
            // Stop at the first pair that doesn't fit: that's either the
            // clean, never-corrupted suffix, or a non-ASCII code unit this
            // transform cannot safely reconstruct. `c != '\0'` additionally
            // refuses to "recover" a genuine embedded NUL char as content.
            let mut j = i + 2;
            let mut run = String::new();
            while chars.get(j) == Some(&'\0') {
                match chars.get(j + 1) {
                    Some(&c) if c != '\0' && (c as u32) < 0x80 => {
                        run.push(c);
                        j += 2;
                    }
                    _ => break,
                }
            }
            if j > i + 2 {
                // At least one full (NUL, ASCII) pair recovered — safe to
                // drop the BOM marker and splice in the decoded run.
                out.push_str(&run);
                i = j;
                changed = true;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(s)
    }
}

/// URL-ish tokens written as PLAIN TEXT in the document body.
///
/// The reference list below is built from a PDF's `/Annot` link layer. A CV
/// typeset without real hyperlinks has none, so `aijobhunter.app` survives only
/// as characters — and every consumer downstream (the TS link injector that
/// re-attaches a project's URL to its item, the résumé seeder) reads the
/// reference list. The result was a generated résumé with no project links at
/// all, whether or not the model happened to copy them.
///
/// A path-less host is only accepted for a TLD a technology name does not use.
/// `.io` is excluded there ON PURPOSE: `socket.io` and `crates.io` are a library
/// and a registry, not the candidate's links, and they are indistinguishable
/// from a real apex domain by shape alone. `.io` WITH a path
/// (`saeedkolivand.github.io/ai-engineering-hub`) is unambiguous and kept.
///
/// Deliberately local rather than a relaxation of
/// `validate::content::factual::links::URL_RE`: that regex grades link Criticals, and
/// widening it would change what counts as a claimed link everywhere.
static TEXT_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:https?://[^\s\]<>]+|(?:www\.)?[a-z0-9][a-z0-9-]*(?:\.[a-z0-9-]+)*\.(?:com|org|net|dev|app|de|co|ai|sh|me|io)(?:/[^\s\]<>]*)?)").unwrap()
});

/// TLDs a bare, path-less host may end in — see [`TEXT_URL_RE`].
const PATHLESS_TLDS: [&str; 8] = ["com", "org", "net", "dev", "app", "de", "co", "ai"];

/// Trim what trails a URL in prose without eating part of the path.
///
/// A `)` may belong to the URL (`…/Function_(mathematics)`) or to the sentence
/// around it (`(see example.com/a)`), and the regex cannot tell which. It keeps
/// every `)` and this decides after the fact: a closing paren survives only when
/// an unclosed `(` inside the token is waiting for it. Sentence punctuation and
/// an unbalanced paren are stripped in turn, since either can sit outside the
/// other (`(see example.com/a).`).
fn trim_url_tail(token: &str) -> &str {
    let mut token = token;
    loop {
        let trimmed = token.trim_end_matches(['.', ',', ';', ':']);
        let trimmed = match trimmed.strip_suffix(')') {
            Some(without) if trimmed.matches(')').count() > trimmed.matches('(').count() => without,
            _ => trimmed,
        };
        if trimmed == token {
            return token;
        }
        token = trimmed;
    }
}

fn links_from_text(text: &str) -> Vec<Link> {
    let mut out: Vec<Link> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for m in TEXT_URL_RE.find_iter(text) {
        // Never take the host half of an email address.
        if text[..m.start()].ends_with(['@', '.']) {
            continue;
        }
        let token = trim_url_tail(m.as_str());
        let has_scheme = token.to_ascii_lowercase().starts_with("http");
        if !has_scheme && !token.contains('/') {
            let tld = token
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !PATHLESS_TLDS.contains(&tld.as_str()) {
                continue;
            }
        }
        let url = if has_scheme {
            token.to_string()
        } else {
            format!("https://{token}")
        };
        if seen.insert(url.to_ascii_lowercase()) {
            out.push(Link {
                anchor_text: token.to_string(),
                url,
            });
        }
    }
    out
}

/// Append extracted links at the end of the text as a markdown reference list.
///
/// PDF text and annotation layers use separate coordinate systems; there is no
/// reliable way to splice a link inline at exactly the right word without
/// pdfium. Appending them as a reference list is accurate and never corrupts
/// surrounding text.
fn inline_links(text: &str, links: &[Link]) -> String {
    // No annotation layer: recover what the text itself spells out, so a CV
    // typeset without real hyperlinks still carries its links forward.
    let harvested;
    let links = if links.is_empty() {
        harvested = links_from_text(text);
        harvested.as_slice()
    } else {
        links
    };
    append_link_reference(text.to_string(), links)
}

#[cfg(test)]
mod tests;
