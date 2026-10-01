//! Tests for render option defaults + the document-meta preamble.

use super::*;

#[test]
fn normalise_accent_accepts_hash_prefix() {
    assert_eq!(
        normalise_accent(Some("#1a2b3c")),
        Some("#1a2b3c".to_string())
    );
}

#[test]
fn normalise_accent_accepts_bare_hex() {
    assert_eq!(
        normalise_accent(Some("1A2B3C")),
        Some("#1A2B3C".to_string())
    );
}

#[test]
fn normalise_accent_rejects_invalid() {
    assert_eq!(normalise_accent(Some("red")), None);
    assert_eq!(normalise_accent(Some("#GG0000")), None);
    assert_eq!(normalise_accent(Some("12345")), None);
}

#[test]
fn normalise_accent_none_is_none() {
    assert_eq!(normalise_accent(None), None);
}

#[test]
fn document_meta_preamble_sets_title_author_and_lang() {
    let meta = document_meta_preamble("data.header.name", "Résumé");
    assert!(
        meta.contains(
            "#set document(title: data.header.name + \" — Résumé\", author: data.header.name)"
        ),
        "preamble must set the PDF title + author from the candidate name; got {meta:?}"
    );
    assert!(
        meta.contains("#set text(lang: data.opts.lang)"),
        "preamble must set the document language for screen readers; got {meta:?}"
    );
}
