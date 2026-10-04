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
