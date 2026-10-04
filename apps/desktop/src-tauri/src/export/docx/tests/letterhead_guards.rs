//! Letterhead-name suppression guards: a letter that opens at a salutation,
//! subject line, or bare date must never have that opening line consumed as
//! a fabricated letterhead name — and an attached `ContactProfile` must
//! survive even when the name is correctly suppressed.

use super::support::{all_font_sizes, document_xml, letter_request, REFINED_US_TEXT};
use crate::export::docx::generate_docx;
use crate::export::types::{
    DocumentType, ExportFormat, ExportRequest, GenerationMeta, LetterLayout, TemplateId,
};

/// A cover letter that opens directly at the salutation (no letterhead name/
/// contact lines) must keep its "Dear …" line and render the body normally.
///
/// The name block used to fire on the FIRST non-blank line whatever it was, so a
/// letterhead-less letter had its salutation consumed as the name (replaced with
/// `meta.candidate_name`) and, because `in_body` is set only in the salutation
/// arm, the whole body then rendered in the muted addressee style. Covers BOTH
/// letter renderers — `_classic` (Classic) and `_layout` (Refined/Banded/Navy).
#[test]
fn letterhead_less_letter_keeps_its_salutation_and_body() {
    for layout in [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let request = ExportRequest {
            text: "Dear Hiring Manager,\n\nI am writing to apply for the role.\n\nSincerely,\nJane Smith".to_string(),
            format: ExportFormat::Docx,
            document_type: DocumentType::CoverLetter,
            template_id: TemplateId::Classic,
            // `candidate_name` is what the buggy name block substituted IN PLACE
            // of the salutation, so its presence makes the drop observable.
            meta: Some(GenerationMeta {
                candidate_name: Some("Jane Smith".to_string()),
                job_title: None,
                company_name: None,
                target_language: None,
            }),
            ats_mode: false,
            locale: None,
            contact: None,
            accent: None,
            letter_layout: layout,
        };
        let bytes = generate_docx(&request).expect("docx");
        let xml = document_xml(&bytes);
        assert!(
            xml.contains("Dear Hiring Manager"),
            "{layout:?}: the salutation was consumed as the letterhead name"
        );
    }
}

/// A cover letter that opens directly at a subject/reference line (e.g. German
/// "Betreff: …") with no letterhead name above it must keep that subject line —
/// same defect class as `letterhead_less_letter_keeps_its_salutation_and_body`
/// (#876), but for `is_subject_line` instead of `is_salutation`/`is_signoff`.
/// The name block used to fire on the first non-blank line whatever it was, so
/// the subject line was consumed as the name (replaced with
/// `meta.candidate_name`) instead of rendering as a subject-styled line, and
/// the salutation/body that follow it must still render normally. Covers BOTH
/// letter renderers — `_classic` (Classic) and `_layout` (Refined/Banded/Navy).
#[test]
fn letterhead_less_letter_with_subject_line_keeps_subject_and_body() {
    for layout in [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let request = ExportRequest {
            text: "Betreff: Bewerbung als Software Engineer\n\nSehr geehrte Frau Dr. Weber,\n\nmit großem Interesse habe ich Ihre Stellenausschreibung gelesen und bewerbe mich hiermit.\n\nMit freundlichen Grüßen,\nMax Müller".to_string(),
            format: ExportFormat::Docx,
            document_type: DocumentType::CoverLetter,
            template_id: TemplateId::Classic,
            // `candidate_name` is what the buggy name block substituted IN PLACE
            // of the subject line, so its presence makes the drop observable.
            meta: Some(GenerationMeta {
                candidate_name: Some("Max Müller".to_string()),
                job_title: None,
                company_name: None,
                target_language: None,
            }),
            ats_mode: false,
            locale: None,
            contact: None,
            accent: None,
            letter_layout: layout,
        };
        let bytes = generate_docx(&request).expect("docx");
        let xml = document_xml(&bytes);
        assert!(
            xml.contains("Betreff: Bewerbung als Software Engineer"),
            "{layout:?}: the subject line was consumed as the letterhead name"
        );
        assert!(
            xml.contains("Sehr geehrte Frau Dr. Weber"),
            "{layout:?}: the salutation must still render after the subject line"
        );
        assert!(
            xml.contains("mit großem Interesse"),
            "{layout:?}: the body must still render after the subject line"
        );
    }
}

/// A cover letter that opens with a DATE and has no candidate name must not
/// fabricate a letterhead name from the date — the fourth opening kind the
/// name-block's salutation/sign-off/subject checks alone don't catch (the
/// device guard already refused it; the NAME TEXT itself didn't). Same defect
/// class as `letterhead_less_letter_keeps_its_salutation_and_body`, now via
/// `is_letterhead_name`'s date check. Covers all six layouts (`_classic` and
/// `_layout`), and both shapes "no candidate name" actually arrives in —
/// `None` and `Some(String::new())` — mirroring
/// `monogram_docx_emits_no_device_for_a_date_opening`'s matrix; the two used
/// to diverge before `resolve_letterhead_candidate` unified them (CodeRabbit
/// round 1, item 1).
#[test]
fn letterhead_less_letter_with_date_opening_suppresses_the_name_not_the_date() {
    const DATE_FIRST: &str =
        "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    let max_size = |xml: &str| all_font_sizes(xml).into_iter().max().unwrap_or(0);

    for layout in [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        for candidate_name in [None, Some(String::new())] {
            // No candidate name (either shape) and no ContactProfile — the
            // actual reachable case: three renderer call sites pass an empty
            // `candidate_name`.
            let mut request = letter_request(DATE_FIRST, layout);
            request.meta = candidate_name.clone().map(|candidate_name| GenerationMeta {
                candidate_name: Some(candidate_name),
                job_title: None,
                company_name: None,
                target_language: None,
            });
            let xml = document_xml(&generate_docx(&request).expect("docx"));

            assert!(
                xml.contains("12 March 2025"),
                "{layout:?} candidate_name={candidate_name:?}: the date line must not be \
                 dropped, just not treated as the name: {xml}"
            );
            assert!(
                xml.contains("Dear Hiring Manager"),
                "{layout:?} candidate_name={candidate_name:?}: the salutation must still \
                 render normally"
            );

            // Strongest check: a real-name render of the SAME layout emits a
            // name-sized (large) run for the header; the date-opening render
            // must not — before this guard, "12 March 2025" WAS that run, at
            // the same size as a real name.
            let with_real_name = document_xml(
                &generate_docx(&letter_request(REFINED_US_TEXT, layout)).expect("docx"),
            );
            assert!(
                max_size(&xml) < max_size(&with_real_name),
                "{layout:?} candidate_name={candidate_name:?}: a date opening must not emit a \
                 name-sized header run (date-opening max size {}, real-name max size {})",
                max_size(&xml),
                max_size(&with_real_name)
            );

            // No header content means no decorative shading either — Banded's
            // band / Sidebar's rail approximation / Monogram's device are all
            // gated on the SAME header block that's now skipped, so none of
            // them should paint an empty tinted box with nothing beside it.
            assert!(
                !xml.contains("w:shd"),
                "{layout:?} candidate_name={candidate_name:?}: no header content means no \
                 decorative shading either: {xml}"
            );
        }
    }
}

/// The other half of the matrix above: an empty-string `candidate_name` must
/// NOT suppress a REAL name that IS on the letter's own first line.
///
/// CodeRabbit round 1, item 1 (MAJOR, verified before fixing): both DOCX
/// line-scanners resolved `candidate_name` via
/// `meta.and_then(...).map(...).unwrap_or(&clean)`, with no empty-string
/// filter — `Some("")` is not `None`, so that chain returned `""` rather
/// than falling through to `clean`. The PDF parser already filtered blank
/// `meta_name` before its own fallback (`letter.rs`'s
/// `resolve_letterhead_candidate` call), so a nameless request whose letter
/// opened with a real name rendered that name in PDF while DOCX silently
/// suppressed it — the exact PDF/DOCX divergence this whole guard family
/// exists to prevent. This is the red-first test for that fix: reverting
/// `resolve_letterhead_candidate`'s empty-string filter (or re-nesting a
/// bare `unwrap_or` at either DOCX call site) turns it red.
#[test]
fn empty_candidate_name_does_not_suppress_a_real_first_line_name() {
    const NAMED_FIRST: &str =
        "Jane Smith\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    let max_size = |xml: &str| all_font_sizes(xml).into_iter().max().unwrap_or(0);

    for layout in [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let mut request = letter_request(NAMED_FIRST, layout);
        request.meta = Some(GenerationMeta {
            candidate_name: Some(String::new()),
            job_title: None,
            company_name: None,
            target_language: None,
        });
        let xml = document_xml(&generate_docx(&request).expect("docx"));

        // Case-insensitive: Banded/Navy uppercase the name in DOCX
        // (`LetterDocxStyle::uppercase_name`), matching their `.typ` small-
        // caps treatment — "JANE SMITH", not "Jane Smith" — so the presence
        // check must not assume a specific case.
        assert!(
            xml.to_lowercase().contains("jane smith"),
            "{layout:?}: candidate_name: Some(\"\") must fall through to the letter's own \
             first line, not suppress a real name: {xml}"
        );

        // Matching strength to the negative case above: the name must be
        // NAME-SIZED, not merely present as body text somewhere.
        let no_meta_baseline =
            document_xml(&generate_docx(&letter_request(NAMED_FIRST, layout)).expect("docx"));
        assert_eq!(
            max_size(&xml),
            max_size(&no_meta_baseline),
            "{layout:?}: candidate_name: Some(\"\") must render the SAME name-sized header run \
             as candidate_name: None does (both fall through to the same first line)"
        );
    }
}

/// HIGH regression (caught in review of the commit above): `profile_contact_md`
/// emission was nested INSIDE the name-validity branch, so a nameless
/// (date- or salutation-opening) letter with a REAL `ContactProfile` attached
/// lost the user's contact info from the DOCX entirely — not just the
/// fabricated name. That is strictly WORSE than the pre-guard behaviour on
/// the PII-survives-export axis: before, a garbage name at least kept the
/// contact line alive alongside it. Contact must render independent of
/// whether the name does — it comes from a separately-attached profile, not
/// from parsing the line — exactly like the PDF parser's `contact_md`, which
/// is built unconditionally before any line is classified. Covers both
/// opening kinds (date AND salutation), across all six layouts.
#[test]
fn contact_profile_survives_even_when_the_letterhead_name_is_suppressed() {
    let profile = crate::contact_profile::ContactProfile {
        email: Some("jane@example.com".to_string()),
        ..Default::default()
    };
    let max_size = |xml: &str| all_font_sizes(xml).into_iter().max().unwrap_or(0);

    for (label, text) in [
        (
            "date-opening",
            "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n",
        ),
        (
            "salutation-opening",
            "Dear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n",
        ),
    ] {
        for layout in [
            LetterLayout::Classic,
            LetterLayout::Refined,
            LetterLayout::Banded,
            LetterLayout::Navy,
            LetterLayout::Sidebar,
            LetterLayout::Monogram,
        ] {
            let mut request = letter_request(text, layout);
            request.contact = Some(profile.clone());
            let xml = document_xml(&generate_docx(&request).expect("docx"));

            assert!(
                xml.contains("jane@example.com"),
                "{label}/{layout:?}: the attached ContactProfile's contact line must survive \
                 even though there is no valid letterhead name: {xml}"
            );

            // "the fake name is NOT [present]": no name-sized run at all —
            // reusing the real-name baseline from the sibling test above.
            let with_real_name = document_xml(
                &generate_docx(&letter_request(REFINED_US_TEXT, layout)).expect("docx"),
            );
            assert!(
                max_size(&xml) < max_size(&with_real_name),
                "{label}/{layout:?}: no name-sized header run should exist when the letterhead \
                 name is suppressed, even with a contact profile attached \
                 (this render's max size {}, real-name max size {})",
                max_size(&xml),
                max_size(&with_real_name)
            );
        }
    }
}
