//! PR5: Letter layout DOCX wiring. `generate_cover_letter_docx` previously
//! ignored `request.letter_layout` entirely, so a Banded/Refined choice never
//! reached the DOCX export (a preview/export honesty violation). These tests
//! lock in the fix for Classic/Refined/Banded plus the whole-roster
//! distinct-bytes guard.

use super::support::{document_xml, letter_request, REFINED_DE_TEXT, REFINED_US_TEXT};
use crate::export::docx::generate_docx;
use crate::export::types::LetterLayout;

#[test]
fn cover_letter_docx_classic_renders_and_omits_new_markup() {
    // Classic must stay on the untouched original renderer: no shading, no
    // extra paragraph borders introduced by the Refined/Banded wiring.
    let bytes =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Classic)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(!xml.contains("w:shd"), "Classic must not carry any shading");
    assert!(
        !xml.contains("w:pBdr"),
        "Classic must not carry any paragraph borders"
    );
    assert!(
        xml.contains("Dear Hiring Manager") || xml.contains("Dear"),
        "Classic must still render the salutation"
    );
}

#[test]
fn cover_letter_docx_refined_right_aligns_contact_and_adds_bottom_border() {
    let bytes =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Refined)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:jc w:val="right""#),
        "Refined must right-align the contact block: {xml}"
    );
    assert!(
        xml.contains("w:pBdr") && xml.contains("w:bottom"),
        "Refined must add a bottom-border rule under the header: {xml}"
    );
}

#[test]
fn cover_letter_docx_refined_shows_reference_line_from_subject_de() {
    // DE market: subject_line_label = "Betreff" — the market's own label, so
    // the caption is NOT suppressed and both the caption and the (label-
    // stripped) body must appear.
    let mut request = letter_request(REFINED_DE_TEXT, LetterLayout::Refined);
    request.locale = Some("de".to_string());
    let bytes = generate_docx(&request).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains("BETREFF"),
        "Refined DE must render the uppercase BETREFF caption: {xml}"
    );
    assert!(
        xml.contains("Bewerbung"),
        "Refined DE must render the (label-stripped) subject body: {xml}"
    );
}

#[test]
fn cover_letter_docx_refined_suppresses_redundant_reference_caption_us() {
    // US market: subject_line_label = "" but the text carries its own "Re:"
    // prefix — the caption must be suppressed to avoid "SUBJECT / Re: …".
    let mut request = letter_request(REFINED_US_TEXT, LetterLayout::Refined);
    request.locale = Some("us".to_string());
    let bytes = generate_docx(&request).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains("PX-2291"),
        "Refined US must still render the reference text itself: {xml}"
    );
    assert!(
        !xml.contains("SUBJECT"),
        "Refined US must suppress the redundant caption when the subject already opens with 'Re:': {xml}"
    );
}

#[test]
fn cover_letter_docx_banded_shades_name_paragraph_and_uppercases() {
    let bytes =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Banded)).expect("docx");
    let xml = document_xml(&bytes);
    // Classic's accent is #222222; lightened 85% toward white → #DEDEDE
    // (34 + (255-34)*0.85 ≈ 222 per channel — `lighten_rgb`).
    assert!(
        xml.contains("w:shd") && xml.contains(r#"w:fill="DEDEDE""#),
        "Banded must shade the name paragraph with the lightened accent: {xml}"
    );
    assert!(
        xml.contains("JANE SMITH"),
        "Banded must uppercase the candidate name: {xml}"
    );
}

#[test]
fn cover_letter_docx_banded_adds_right_aligned_contact_and_footer_border() {
    let bytes =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Banded)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:jc w:val="right""#),
        "Banded must right-align the contact block: {xml}"
    );
    assert!(
        xml.contains("w:pBdr") && xml.contains("w:bottom"),
        "Banded must add a bottom-border footer rule: {xml}"
    );
}

#[test]
fn cover_letter_docx_layouts_produce_distinct_bytes() {
    // Pairwise over the WHOLE roster. Hand-written pairs are how a new layout
    // ends up silently rendering as another one: `generate_cover_letter_docx_
    // layout` branched on a single `is_refined` boolean, so Navy inherited
    // Banded's shaded header band and rule footer — the same export produced a
    // Navy PDF and a Banded DOCX. Navy vs Banded remains the load-bearing pair
    // and now cannot be dropped by accident.
    let rendered: Vec<(LetterLayout, Vec<u8>)> = [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ]
    .into_iter()
    .map(|layout| {
        let bytes = generate_docx(&letter_request(REFINED_US_TEXT, layout))
            .unwrap_or_else(|e| panic!("{layout:?} docx: {e}"));
        assert!(!bytes.is_empty(), "{layout:?} produced empty DOCX bytes");
        (layout, bytes)
    })
    .collect();

    for (i, (a_id, a)) in rendered.iter().enumerate() {
        for (b_id, b) in rendered.iter().skip(i + 1) {
            assert_ne!(a, b, "{a_id:?} and {b_id:?} DOCX bytes must differ");
        }
    }
}
