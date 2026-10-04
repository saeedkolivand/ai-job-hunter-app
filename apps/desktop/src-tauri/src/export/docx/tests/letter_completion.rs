use super::support::{
    document_xml, docx_paragraphs, letter_request, LETTER_FIXTURE_BODY_ONLY_DE,
    LETTER_FIXTURE_BODY_ONLY_US,
};
use crate::export::docx::generate_docx;
use crate::export::types::LetterLayout;

/// Guardrail for the shipped fix — DOCX side, so PDF and DOCX cannot drift on
/// this. `generate_docx` (this module's `mod.rs`) never calls
/// `complete_letter_text` itself — only `export::commands::validate_and_normalize`
/// does — so `letter_request` bypasses completion exactly like
/// `typst_engine::render_letter_pdf` does on the PDF side. This test
/// completes the fixture HERE first, the same seam the sibling PDF tests
/// (`typst_engine::test::body_only_us_letter_gets_completed_furniture_in_the_pdf_text_layer`
/// / `..._de_...`) use, before building the `ExportRequest` and rendering.
#[test]
fn body_only_letter_gets_completed_furniture_in_docx_document_xml() {
    for (market, fixture, name, salutation, signoff, needle1, needle2) in [
        (
            "us",
            LETTER_FIXTURE_BODY_ONLY_US,
            "Jane Smith",
            "Dear Hiring Manager",
            "Sincerely",
            "distributed systems",
            "microservices",
        ),
        (
            "de",
            LETTER_FIXTURE_BODY_ONLY_DE,
            "Max Müller",
            "Sehr geehrte Damen und Herren",
            "Mit freundlichen Grüßen",
            "verteilter Systeme",
            "Jest",
        ),
    ] {
        let completed = crate::export::letter_shape::complete_letter_text(fixture, market, name);
        let mut request = letter_request(&completed, LetterLayout::Classic);
        // `letter_request` defaults `locale: None` (→ `intl`); this fixture's
        // salutation/sign-off came from `conventions(market)`, so the render
        // must resolve the SAME market or the two could silently disagree —
        // mirrors `export::commands::validate_and_normalize`'s own
        // `request.locale.as_deref().unwrap_or("intl")` computation feeding
        // both `complete_letter_text` and the render call with one value.
        request.locale = Some(market.to_string());
        let xml = document_xml(&generate_docx(&request).expect("docx"));

        assert!(
            xml.contains(salutation),
            "{market}: salutation {salutation:?} missing from word/document.xml — \
             complete_letter_text must have run: {xml}"
        );
        assert!(
            xml.contains(signoff),
            "{market}: sign-off {signoff:?} missing from word/document.xml — \
             complete_letter_text must have run: {xml}"
        );
        assert!(
            xml.contains(name),
            "{market}: signature name {name:?} missing from word/document.xml: {xml}"
        );

        // Body paragraphs must land as SEPARATE <w:p> elements, not flattened
        // into one run-on paragraph — the DOCX shape of the same bug the PDF
        // tests guard ("plain text, no bold, no paragraph spacing").
        let paras = docx_paragraphs(&xml);
        let idx1 = paras
            .iter()
            .position(|p| p.contains(needle1))
            .unwrap_or_else(|| panic!("{market}: no <w:p> paragraph contains {needle1:?}: {xml}"));
        let idx2 = paras
            .iter()
            .position(|p| p.contains(needle2))
            .unwrap_or_else(|| panic!("{market}: no <w:p> paragraph contains {needle2:?}: {xml}"));
        assert_ne!(
            idx1, idx2,
            "{market}: body paragraphs containing {needle1:?} and {needle2:?} must render as \
             separate <w:p> elements, not merged into one: {xml}"
        );

        // `needle2` is the `**bold**` keyword in the fixture — actually assert
        // the markdown becomes a real bold RUN, not just separate paragraphs.
        // Literal `**` must never leak into the rendered XML text...
        assert!(
            !xml.contains("**"),
            "{market}: literal ** markdown leaked into word/document.xml — \
             parse_inline_md must have consumed it: {xml}"
        );
        // ...and the paragraph carrying the bold keyword must contain a real
        // `<w:b />` run property (see `create_runs` in `docx_renderer.rs`),
        // the same proxy `navy_docx_...` above uses for bold-run assertions.
        assert!(
            paras[idx2].contains("<w:b />"),
            "{market}: paragraph containing {needle2:?} must carry a bold run \
             (<w:b />): {}",
            paras[idx2]
        );
    }
}
