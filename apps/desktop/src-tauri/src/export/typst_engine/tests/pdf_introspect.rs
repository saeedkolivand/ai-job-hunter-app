//! PDF-byte introspection helpers (page count, link annotations, embedded fonts, stray-Typst-token guard) shared across the typst_engine test topics.

/// Count `/Type /Page` (individual page) objects in PDF bytes.
///
/// Uses a byte-level scan rather than lopdf's `get_pages()` because lopdf's
/// page-tree walker does not handle all page-tree structures that Typst emits
/// (it misses pages under certain indirect-reference trees and returns 1 even
/// for multi-page documents). The scan finds all occurrences of the `/Type`
/// `/Page` dictionary entry that marks an individual page object (not `/Type`
/// `/Pages` which marks a page-tree node).
///
/// Tolerates zero-or-more spaces between `/Type` and `/Page`: typst-pdf 0.15's
/// krilla/pdf-writer backend serialises dict entries as `/Type/Page` (no
/// space), where the pinned 0.14.2 backend wrote `/Type /Page` (one space).
/// Matching both keeps this scan from silently reporting zero pages again on
/// the next writer-formatting tweak.
pub(super) fn count_pdf_pages(bytes: &[u8]) -> usize {
    let key = b"/Type";
    let val = b"/Page";
    let mut count = 0usize;
    let mut i = 0usize;
    while i + key.len() < bytes.len() {
        if bytes[i..i + key.len()] == *key {
            let mut j = i + key.len();
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if bytes[j..].starts_with(val) {
                // The character after `/Page` must not be `s` (which would make it `/Pages`).
                if bytes.get(j + val.len()) != Some(&b's') {
                    count += 1;
                }
            }
            i += key.len();
        } else {
            i += 1;
        }
    }
    count
}

/// Extract every `/Link` annotation target URI from a rendered PDF.
///
/// Typst writes `/Annots` as an array of **inline dictionaries**; lopdf's
/// `get_page_annotations` only resolves *indirect references* and so misses them
/// entirely — the documented regression that once made every header link read as
/// "missing". We therefore walk each object's `/Annots` array ourselves (mirroring
/// the validator's reader) and pull `/A /URI` off each `/Link`.
pub(super) fn link_uris(bytes: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(bytes).expect("rendered PDF should parse with lopdf");

    fn uri_of(annot: &lopdf::Dictionary, doc: &lopdf::Document) -> Option<String> {
        let is_link = annot
            .get(b"Subtype")
            .and_then(|v| v.as_name())
            .map(|n| n == b"Link")
            .unwrap_or(false);
        if !is_link {
            return None;
        }
        annot
            .get(b"A")
            .ok()
            .and_then(|a| match a {
                lopdf::Object::Dictionary(d) => Some(d.clone()),
                lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
                _ => None,
            })
            .and_then(|d| {
                d.get(b"URI")
                    .ok()
                    .and_then(|u| u.as_str().ok())
                    .map(|b| String::from_utf8_lossy(b).into_owned())
            })
    }

    let mut uris = Vec::new();
    for obj in doc.objects.values() {
        let Ok(dict) = obj.as_dict() else {
            continue;
        };
        let array = match dict.get(b"Annots") {
            Ok(lopdf::Object::Array(a)) => a.clone(),
            Ok(lopdf::Object::Reference(id)) => {
                match doc.get_object(*id).and_then(|o| o.as_array()) {
                    Ok(a) => a.clone(),
                    Err(_) => continue,
                }
            }
            _ => continue,
        };
        for entry in &array {
            let annot = match entry {
                lopdf::Object::Dictionary(d) => d.clone(),
                lopdf::Object::Reference(id) => match doc.get_dictionary(*id) {
                    Ok(d) => d.clone(),
                    Err(_) => continue,
                },
                _ => continue,
            };
            if let Some(uri) = uri_of(&annot, &doc) {
                uris.push(uri);
            }
        }
    }
    uris
}

/// Every embedded font's `/BaseFont` name in a rendered PDF — the same signal
/// used to confirm (or refute) which font FACES actually got embedded (a
/// synthetic/faked italic reuses the Regular face's outlines and embeds no
/// separate font program; a genuine italic run embeds its own `/BaseFont`,
/// typically containing "Italic" in a Typst/krilla-emitted subset name).
pub(super) fn embedded_font_base_names(bytes: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(bytes).expect("rendered PDF should parse with lopdf");
    let mut names = Vec::new();
    for obj in doc.objects.values() {
        let Ok(dict) = obj.as_dict() else { continue };
        let is_font = dict
            .get(b"Type")
            .and_then(|v| v.as_name())
            .map(|n| n == b"Font")
            .unwrap_or(false);
        if !is_font {
            continue;
        }
        if let Ok(base) = dict.get(b"BaseFont").and_then(|v| v.as_name()) {
            names.push(String::from_utf8_lossy(base).into_owned());
        }
    }
    names
}

//
// Renders a fixture through EVERY template (classic, swiss-minimal,
// academic, atelier, meridian, throughline, portrait, lebenslauf, letter) and
// asserts that the extracted PDF text contains NONE of the following
// case-sensitive substrings — these are Typst code tokens that would appear as
// literal printed text when a `#` prefix is accidentally omitted from a
// top-level call in markup context.
//
// Caught by this guard:
//   - `line(length` / `stroke:` / `block(above` / `block(below` / `grid(columns`
//   - `#let` / `pad(left` / `place(`
//
// This guard caught the `lebenslauf.typ` Bug 2 (missing `#` before `line` and
// `block` in the header section) and will catch any future regression across
// all templates.

pub(super) const STRAY_TOKENS: &[&str] = &[
    "line(length",
    "stroke:",
    "block(above",
    "block(below",
    "grid(columns",
    "#let",
    "pad(left",
    "place(",
    "tracking:",
    "smallcaps(",
];

/// Render `bytes` through pdf-extract and assert no stray Typst tokens appear.
pub(super) fn assert_no_stray_tokens(label: &str, bytes: &[u8]) {
    let extracted = pdf_extract::extract_text_from_mem(bytes)
        .unwrap_or_else(|e| panic!("stray-token guard: pdf-extract failed for {label}: {e}"));

    for token in STRAY_TOKENS {
        assert!(
            !extracted.contains(token),
            "stray-token guard [{label}]: found leaked Typst code token {token:?} \
             in extracted text — a `#` prefix is likely missing in the template source.\n\
             Extracted snippet (first 2000 chars):\n{:.2000}",
            extracted,
        );
    }
}
