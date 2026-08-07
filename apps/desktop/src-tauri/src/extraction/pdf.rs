use std::borrow::Cow;

use lopdf::{Dictionary, Document, Object};
use tracing::warn;

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
mod test {
    use super::*;

    /// The reported defect: Typst/Word write annotation `/Contents` as UTF-16BE
    /// behind a `FE FF` BOM. Read as UTF-8 that became
    /// `��g\0i\0t\0h\0u\0b\0…` — a BOM as two replacement chars followed by
    /// every ASCII byte interleaved with its NUL high byte.
    #[test]
    fn utf16be_annotation_text_decodes_instead_of_becoming_mojibake() {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in "github.com/saeedkolivand".encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        let decoded = pdf_text_string(&bytes);
        assert_eq!(decoded, "github.com/saeedkolivand");
        assert!(
            !decoded.contains('\u{FFFD}') && !decoded.contains('\0'),
            "decoded anchor must carry no replacement chars or NULs; got {decoded:?}"
        );
    }

    /// Not spec-legal for a PDF text string, but producers emit it.
    #[test]
    fn utf16le_with_bom_also_decodes() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "crosskit.iamsaeed.dev".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(pdf_text_string(&bytes), "crosskit.iamsaeed.dev");
    }

    /// Non-ASCII must survive, including astral-plane chars (surrogate pairs) —
    /// decoding code units one at a time would mangle these.
    #[test]
    fn utf16be_survives_accents_and_surrogate_pairs() {
        let source = "Café — Ingénieur 🚀";
        let mut bytes = vec![0xFE, 0xFF];
        for unit in source.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        assert_eq!(pdf_text_string(&bytes), source);
    }

    /// The common case — a BOM-less ASCII string — must pass through untouched,
    /// so the fix cannot regress PDFs that were already extracting correctly.
    #[test]
    fn bomless_ascii_is_unchanged() {
        assert_eq!(
            pdf_text_string(b"https://github.com/saeedkolivand/crosskit"),
            "https://github.com/saeedkolivand/crosskit"
        );
    }

    /// PDF 2.0 allows UTF-8 behind a BOM; the BOM must not leak into the text.
    #[test]
    fn utf8_bom_is_stripped() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("aijobhunter.app".as_bytes());
        assert_eq!(pdf_text_string(&bytes), "aijobhunter.app");
    }

    /// A truncated UTF-16 payload must not panic — a trailing odd byte cannot
    /// form a code unit, so it is dropped.
    #[test]
    fn odd_trailing_byte_does_not_panic() {
        let bytes = [0xFE, 0xFF, 0x00, b'a', 0x00];
        assert_eq!(pdf_text_string(&bytes), "a");
    }

    /// BOM-less non-ASCII is the ambiguous case. `0xE9` is not valid UTF-8, so
    /// it can only have meant PDFDocEncoding's `é` — decoding it as UTF-8
    /// (lossy) would have produced U+FFFD and corrupted the anchor.
    #[test]
    fn bomless_pdfdocencoding_high_bytes_decode_to_their_characters() {
        assert_eq!(pdf_text_string(b"Caf\xE9"), "Café");
        assert_eq!(pdf_text_string(b"Ing\xE9nieur"), "Ingénieur");
        assert!(!pdf_text_string(b"Caf\xE9").contains('\u{FFFD}'));
    }

    /// PDFDocEncoding's 0x80..=0x9E band is typography, where Latin-1 has
    /// control characters — so it cannot be decoded as Latin-1 throughout.
    #[test]
    fn pdfdocencoding_typography_band_is_not_latin1() {
        assert_eq!(pdf_text_string(b"\x88x\x89"), "‹x›"); // guilsingl left/right
        assert_eq!(pdf_text_string(b"a\x83b"), "a…b"); // ellipsis
        assert_eq!(pdf_text_string(b"a\x84b"), "a—b"); // emdash
        assert_eq!(pdf_text_string(b"\x9E"), "ž"); // last entry — off-by-one guard
    }

    /// Every byte must decode without panicking. This is the test that caught a
    /// truncated lookup table indexing past its end for 0x97..=0x9E.
    #[test]
    fn every_byte_decodes_without_panicking() {
        for b in 0u8..=0xFF {
            let _ = pdf_text_string(&[b]);
        }
        let all: Vec<u8> = (0u8..=0xFF).collect();
        let _ = pdf_text_string(&all);
    }

    /// A BOM-less byte string that IS valid UTF-8 must be read as UTF-8 — many
    /// producers emit it in defiance of the spec, and reading those bytes as
    /// PDFDocEncoding would render "é" as "Ã©".
    #[test]
    fn bomless_valid_utf8_wins_over_pdfdocencoding() {
        assert_eq!(pdf_text_string("Café".as_bytes()), "Café");
        assert_eq!(pdf_text_string("東京".as_bytes()), "東京");
    }

    /// Undefined PDFDocEncoding code points must not silently become a wrong
    /// character.
    #[test]
    fn undefined_pdfdocencoding_bytes_become_replacement_chars() {
        assert_eq!(pdf_text_string(b"a\xADb"), "a\u{FFFD}b");
    }

    // ── repair_utf16_mojibake (one-shot pre-#955 data repair) ─────────────────

    /// The exact byte shape hex-dumped from a live `documents.text` row:
    /// `- [` + doubled U+FFFD (the BOM misread as UTF-8) + NUL-interleaved
    /// "aijobhunter.app" (the UTF-16BE text misread as UTF-8) + the clean
    /// `](url)\n` suffix, which was never corrupted — it's appended by the
    /// markdown link builder, not produced by the PDF byte decode.
    fn corrupt_markdown_link_tail() -> String {
        let mut bytes = b"- [".to_vec();
        bytes.extend_from_slice(&[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD]); // doubled U+FFFD
        for &b in b"aijobhunter.app" {
            bytes.push(0x00);
            bytes.push(b);
        }
        bytes.extend_from_slice(b"](https://aijobhunter.app/)\n");
        String::from_utf8(bytes).expect("every byte here is individually valid UTF-8")
    }

    #[test]
    fn repair_utf16_mojibake_recovers_the_exact_live_row_shape() {
        let corrupt = corrupt_markdown_link_tail();
        assert!(
            corrupt.contains('\0'),
            "test input must actually contain the NUL that gates the repair"
        );
        assert_eq!(
            repair_utf16_mojibake(&corrupt),
            "- [aijobhunter.app](https://aijobhunter.app/)\n"
        );
    }

    #[test]
    fn repair_utf16_mojibake_leaves_a_clean_string_unchanged() {
        let clean = "Software Engineer with 5 years experience";
        assert_eq!(repair_utf16_mojibake(clean), clean);
        assert!(
            matches!(repair_utf16_mojibake(clean), Cow::Borrowed(_)),
            "the common clean case must not allocate"
        );
    }

    #[test]
    fn repair_utf16_mojibake_leaves_a_lone_replacement_char_without_a_nul_untouched() {
        // A genuinely undecodable byte elsewhere in a document (no NUL nearby)
        // must survive — the NUL gate is what keeps this repair scoped to real
        // pre-#955 rows instead of eating every replacement char in the store.
        let legit = "Caf\u{FFFD} — one bad byte, no embedded NUL";
        assert_eq!(repair_utf16_mojibake(legit), legit);
    }

    #[test]
    fn repair_utf16_mojibake_does_not_fuse_an_unrelated_adjacent_replacement_char_pair() {
        // A genuine, UNRELATED doubled U+FFFD elsewhere in the row (not the
        // BOM marker — not immediately followed by a NUL) must survive
        // untouched, even though the row does contain some other, unrelated
        // stray NUL that gates the repair. Blindly stripping every doubled
        // U+FFFD would fuse "Berlin" and "Germany" into one word here. No
        // recognized marker exists anywhere in this string, so the whole row
        // — trailing NUL included — is left byte-for-byte as stored.
        let input = "Berlin\u{FFFD}\u{FFFD}Germany\0";
        assert_eq!(repair_utf16_mojibake(input), input);
    }

    #[test]
    fn repair_utf16_mojibake_leaves_a_row_untouched_when_the_first_code_unit_is_non_ascii() {
        // A UTF-16BE anchor whose FIRST code unit already has a non-zero
        // high byte (CJK here) never matches the doubled-FFFD-then-NUL
        // marker, so nothing is touched — the row stays exactly as stored,
        // still visibly corrupt (U+FFFD-bearing), rather than being silently
        // reassembled into plausible-but-wrong text. Built from real bytes,
        // not hand-waved — the exact `from_utf8_lossy` output for a genuine
        // UTF-16BE-behind-BOM encoding of "中文x".
        let mut bytes = vec![0xFE, 0xFF];
        for unit in "中文x".encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        let corrupt = String::from_utf8_lossy(&bytes).into_owned();
        assert!(
            corrupt.contains('\0'),
            "the ASCII 'x' code unit's NUL high byte must still gate the repair"
        );

        let repaired = repair_utf16_mojibake(&corrupt);
        assert_eq!(
            repaired, corrupt,
            "no recognized marker exists here, so nothing must be touched"
        );
        assert!(matches!(repaired, Cow::Borrowed(_)), "a true no-op must not allocate");
    }

    #[test]
    fn repair_utf16_mojibake_recovers_only_the_safe_ascii_prefix_of_a_mixed_anchor() {
        // An anchor that starts ASCII ("gi"), then transitions mid-run to a
        // non-ASCII code unit (CJK), then resumes ASCII ("hub") — recovering
        // "gi" is safe (the marker + a full (NUL, ASCII) pair), but the
        // transform must STOP there rather than keep guessing: silently
        // producing "giN-hub" (the naive blind-strip result) would hide real
        // data loss behind plausible-looking text. Built from real bytes.
        let mut bytes = vec![0xFE, 0xFF];
        for unit in "gi中hub".encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        let corrupt = String::from_utf8_lossy(&bytes).into_owned();

        let repaired = repair_utf16_mojibake(&corrupt);

        assert_eq!(repaired, "giN-\0h\0u\0b");
        assert!(
            repaired.contains('\0'),
            "the un-recovered exotic suffix must stay detectably corrupt, not be \
             silently smoothed into \"giN-hub\"; got {repaired:?}"
        );
    }
}
