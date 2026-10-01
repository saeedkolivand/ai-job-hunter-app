//! Reading-order + accented-content + subject-line-gating tests shared by the new letter layouts.

use super::fixtures::{
    letter_lower, normalize_like_validator, signature_block, NO_EXTRACTABLE_TEXT_THRESHOLD,
};
use super::letter_fixtures::{
    LETTER_FIXTURE_DE, LETTER_FIXTURE_IT, LETTER_FIXTURE_LONG_US, LETTER_FIXTURE_US,
    LETTER_FIXTURE_US_SUBJECT,
};
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::render_letter_pdf;

/// (S1/M1) Both new layouts render a valid US PDF whose text extracts in
/// letterhead → recipient → salutation → body → sign-off order. Sidebar's rail
/// is a MARGIN POSITION, not a second column of prose, so it must not scramble
/// this the way a real two-column letter would.
#[test]
fn new_letter_layouts_extract_in_reading_order() {
    for layout in [LetterLayout::Sidebar, LetterLayout::Monogram] {
        let lower = letter_lower(layout, LETTER_FIXTURE_US, "us", false);

        for needle in [
            "jane smith",
            "dear hiring manager",
            "distributed systems",
            "sincerely",
        ] {
            assert!(
                lower.contains(needle),
                "{layout:?}: {needle:?} missing from the extracted text:\n{lower}"
            );
        }

        let pos_head = lower.find("jane smith").expect("letterhead present");
        let pos_recipient = lower.find("acme corp").expect("recipient present");
        let pos_sal = lower.find("dear").expect("salutation present");
        let pos_body = lower.find("distributed").expect("body present");
        let pos_signoff = lower.find("sincerely").expect("sign-off present");
        assert!(
            pos_head < pos_recipient
                && pos_recipient < pos_sal
                && pos_sal < pos_body
                && pos_body < pos_signoff,
            "{layout:?}: reading order broken — head={pos_head} recipient={pos_recipient} \
             sal={pos_sal} body={pos_body} signoff={pos_signoff}\n{lower}"
        );
        assert!(
            lower.contains("123 main street"),
            "{layout:?}: recipient inside address missing:\n{lower}"
        );
    }
}

/// The roster [`NO_SOFT_HYPHEN_LAYOUTS`] just grew from two layouts to the
/// full six — Classic/Refined/Banded/Navy picked up `#set text(hyphenate:
/// false)` in the same change (Sidebar/Monogram already had it from Phase 8).
/// `letter_lower` is the SINGLE choke point that enforces the guard, but every
/// existing call site only ever passed Sidebar or Monogram — extending the
/// roster alone would have been a silent no-op for the other four without
/// this test actually routing them through it.
///
/// [`LETTER_FIXTURE_LONG_US`] carries "microservices architecture" — the
/// exact phrase cited in every layout's `hyphenate: false` comment — inside a
/// long, narrow-column paragraph, i.e. a fixture that WOULD hyphenate if the
/// flag were ever dropped. `letter_lower` asserts the U+00AD absence
/// internally for every layout in the roster; this test's own job is just to
/// call it for all six and confirm the phrase still extracts as one
/// unbroken pair of words.
#[test]
fn every_letter_layout_stays_hyphen_free_on_a_hyphenation_prone_word() {
    for layout in [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let lower = letter_lower(layout, LETTER_FIXTURE_LONG_US, "us", false);
        assert!(
            lower.contains("microservices architecture"),
            "{layout:?}: the hyphenation-prone phrase must still extract as one \
             unbroken pair of words:\n{lower}"
        );
    }
}

/// (S2/M2) Accented-Latin round trip — the guard that caught Cologne Navy's
/// 0.14em tracking, where the name extracted as "À LVA R O È S P O S I T O".
/// Both new layouts track their name, so both need it.
#[test]
fn new_letter_layouts_extract_accented_latin_content() {
    for layout in [LetterLayout::Sidebar, LetterLayout::Monogram] {
        let t = Template::get(TemplateId::SwissMinimal);
        let bytes = render_letter_pdf(
            LETTER_FIXTURE_IT,
            &t,
            None,
            Some("Àlvaro Èsposito"),
            LetterRender {
                market: "us",
                lang: "en",
                layout,
                ats: false,
            },
        )
        .unwrap_or_else(|e| panic!("{layout:?} accented-Latin render failed: {e}"));
        let extracted = pdf_extract::extract_text_from_mem(&bytes)
            .unwrap_or_else(|e| panic!("pdf-extract on {layout:?} accented-Latin: {e}"));
        let lower = extracted
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();

        assert!(
            lower.contains("àlvaro èsposito"),
            "{layout:?}: the accented name did not survive extraction as one word pair — \
             tracking too wide?\n---\n{extracted}"
        );
        assert!(
            signature_block(&lower).contains("àlvaro èsposito"),
            "{layout:?}: accented name missing from the SIGNATURE (after the sign-off); a \
             letterhead-only match would hide a dropped signature\n---\n{extracted}"
        );
        assert!(
            lower.contains("così") || lower.contains("però") || lower.contains("città"),
            "{layout:?}: grave-accented-lowercase body word missing\n---\n{extracted}"
        );

        let normalized_len = normalize_like_validator(&extracted).len();
        assert!(
            normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
            "{layout:?} accented-Latin: only {normalized_len} normalized chars extracted — \
             the real validator's no_extractable_text gate would block this export"
        );
    }
}

/// (S4/M4) Structural elements gate on `data.opts`, NEVER on the layout id: the
/// subject line appears for a DE letter (DIN `Betreff`, `subject_line_used`) and
/// is absent for a US one whose market convention omits it — identical layout,
/// opposite outcome, decided entirely by the market.
#[test]
fn new_letter_layouts_gate_the_subject_line_on_market_opts_not_layout() {
    for layout in [LetterLayout::Sidebar, LetterLayout::Monogram] {
        let de = letter_lower(layout, LETTER_FIXTURE_DE, "de", false);
        // EXACTLY once, not merely present. `contains` is satisfied by one label
        // and by two, and two is what these layouts shipped: `parse_cover_letter`
        // publishes `data.subject` verbatim ("Betreff: Bewerbung …") and the
        // caption printed the label again on top of it, so a DE letter read
        // "BETREFF / Betreff: Bewerbung …". The four shipped layouts strip the
        // label first; these two did not, and a presence-only assertion could
        // not tell the difference.
        let betreff_count = de.matches("betreff").count();
        assert_eq!(
            betreff_count, 1,
            "{layout:?}: the DE market label must render exactly once, found \
             {betreff_count} — the caption is duplicating the label already carried \
             by data.subject:\n{de}"
        );
        // …and it must be the CAPTION that survives, not the raw prefix. The
        // count alone cannot tell those apart: stripping the label without a
        // caption, and a caption without stripping, both yield exactly one.
        // The colon is the tell — it only exists in the unstripped body.
        assert!(
            !de.contains("betreff: bewerbung"),
            "{layout:?}: the label was left on the subject body — `data.subject` must be \
             stripped before rendering, the way letter_refined/letter_navy and the DOCX \
             `strip_market_label` all do it:\n{de}"
        );
        assert!(
            de.contains("bewerbung als software engineer"),
            "{layout:?}: the DE subject body went missing:\n{de}"
        );
        assert!(
            de.contains("sehr geehr") && de.contains("freundlichen"),
            "{layout:?}: German salutation / sign-off missing:\n{de}"
        );
        assert!(
            de.contains("max") && de.contains("müller"),
            "{layout:?}: DE signature name missing:\n{de}"
        );

        let us = letter_lower(layout, LETTER_FIXTURE_US_SUBJECT, "us", false);
        assert!(
            !us.contains("px-2291"),
            "{layout:?}: the subject must NOT render when the market omits it \
             (subject_line_used=false):\n{us}"
        );
    }
}
