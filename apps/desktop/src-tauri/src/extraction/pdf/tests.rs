use super::*;

/// `s` as UTF-16BE behind the `FE FF` byte-order mark — the PDF text-string form every real
/// producer (Typst, LaTeX, Word) emits for annotation `/Contents`.
fn utf16be_with_bom(s: &str) -> Vec<u8> {
    let mut bytes = vec![0xFE, 0xFF];
    for unit in s.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    bytes
}

/// The reported defect: Typst/Word write annotation `/Contents` as UTF-16BE
/// behind a `FE FF` BOM. Read as UTF-8 that became
/// `��g\0i\0t\0h\0u\0b\0…` — a BOM as two replacement chars followed by
/// every ASCII byte interleaved with its NUL high byte.
#[test]
fn utf16be_annotation_text_decodes_instead_of_becoming_mojibake() {
    let decoded = pdf_text_string(&utf16be_with_bom("github.com/saeedkolivand"));
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
    assert_eq!(pdf_text_string(&utf16be_with_bom(source)), source);
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
    let corrupt = String::from_utf8_lossy(&utf16be_with_bom("中文x")).into_owned();
    assert!(
        corrupt.contains('\0'),
        "the ASCII 'x' code unit's NUL high byte must still gate the repair"
    );

    let repaired = repair_utf16_mojibake(&corrupt);
    assert_eq!(
        repaired, corrupt,
        "no recognized marker exists here, so nothing must be touched"
    );
    assert!(
        matches!(repaired, Cow::Borrowed(_)),
        "a true no-op must not allocate"
    );
}

#[test]
fn repair_utf16_mojibake_recovers_only_the_safe_ascii_prefix_of_a_mixed_anchor() {
    // An anchor that starts ASCII ("gi"), then transitions mid-run to a
    // non-ASCII code unit (CJK), then resumes ASCII ("hub") — recovering
    // "gi" is safe (the marker + a full (NUL, ASCII) pair), but the
    // transform must STOP there rather than keep guessing: silently
    // producing "giN-hub" (the naive blind-strip result) would hide real
    // data loss behind plausible-looking text. Built from real bytes.
    let corrupt = String::from_utf8_lossy(&utf16be_with_bom("gi中hub")).into_owned();

    let repaired = repair_utf16_mojibake(&corrupt);

    assert_eq!(repaired, "giN-\0h\0u\0b");
    assert!(
        repaired.contains('\0'),
        "the un-recovered exotic suffix must stay detectably corrupt, not be \
             silently smoothed into \"giN-hub\"; got {repaired:?}"
    );
}

/// Real shape from a reported CV: the links are typeset as plain text, with
/// no `/Annot` layer behind them, so the reference list came out empty and
/// every generated résumé lost its project links.
const PLAIN_TEXT_CV: &str = "\
Saeed Kolivand
iamsaeed.dev  ·  github.com/saeedkolivand

SELECTED PROJECTS

AI Job Hunter   aijobhunter.app
Tauri 2 · Rust · React 19
Local-first desktop application.

CrossKit   crosskit.iamsaeed.dev
TypeScript · React · Vue
Framework-agnostic component library.
";

#[test]
fn plain_text_links_are_recovered_when_a_pdf_has_no_annotations() {
    let urls: Vec<String> = links_from_text(PLAIN_TEXT_CV)
        .into_iter()
        .map(|l| l.url)
        .collect();
    assert_eq!(
        urls,
        vec![
            "https://iamsaeed.dev",
            "https://github.com/saeedkolivand",
            "https://aijobhunter.app",
            "https://crosskit.iamsaeed.dev",
        ],
        "every link the CV spells out, in document order, deduped"
    );
}

/// The reason a path-less `.io` is refused: these are libraries and
/// registries on a technology line, not the candidate's links, and nothing
/// about their SHAPE distinguishes them from an apex domain.
#[test]
fn technology_names_that_look_like_domains_are_not_links() {
    for text in [
        "Node.js · socket.io · Express",
        "Published to crates.io; adopted by fourteen organisations",
        "Vue.js and Next.js",
        "Contact: jane@example.com",
    ] {
        assert!(
            links_from_text(text).is_empty(),
            "must not harvest a link from {text:?}: {:?}",
            links_from_text(text)
        );
    }
}

/// A `)` may belong to the URL or to the sentence around it. Both shapes
/// appear in real résumés, and truncating the first produces a dead link.
#[test]
fn a_balanced_parenthesis_in_a_path_survives_but_a_sentence_paren_does_not() {
    let urls = |t: &str| -> Vec<String> { links_from_text(t).into_iter().map(|l| l.url).collect() };
    assert_eq!(
        urls("See https://example.com/Function_(mathematics) for details"),
        vec!["https://example.com/Function_(mathematics)"],
        "a balanced pair belongs to the path"
    );
    assert_eq!(
        urls("(see https://example.com/a)"),
        vec!["https://example.com/a"],
        "an unmatched closing paren belongs to the sentence"
    );
    assert_eq!(
        urls("(see https://example.com/a)."),
        vec!["https://example.com/a"],
        "punctuation and an unbalanced paren can nest either way round"
    );
    assert_eq!(
        urls("Portfolio: https://example.com/work."),
        vec!["https://example.com/work"],
        "a trailing full stop is sentence punctuation"
    );
}

/// A `.io` host WITH a path is unambiguous, so it stays.
#[test]
fn a_dot_io_host_with_a_path_is_still_a_link() {
    let urls: Vec<String> =
        links_from_text("AI Engineering Hub   saeedkolivand.github.io/ai-engineering-hub")
            .into_iter()
            .map(|l| l.url)
            .collect();
    assert_eq!(
        urls,
        vec!["https://saeedkolivand.github.io/ai-engineering-hub"]
    );
}

/// The harvest is a FALLBACK: a PDF with a real annotation layer keeps it,
/// so an accurate anchor is never replaced by a guessed one.
#[test]
fn a_real_annotation_layer_wins_over_the_text_harvest() {
    let annotated = [Link {
        anchor_text: "My Portfolio".to_string(),
        url: "https://iamsaeed.dev/".to_string(),
    }];
    let out = inline_links(PLAIN_TEXT_CV, &annotated);
    assert!(out.contains("- [My Portfolio](https://iamsaeed.dev/)"));
    assert!(
        !out.contains("https://aijobhunter.app"),
        "the text harvest must not run when annotations exist"
    );
}
