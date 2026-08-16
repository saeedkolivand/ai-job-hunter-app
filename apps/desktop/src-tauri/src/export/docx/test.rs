use std::io::{Cursor, Read};

use super::*;
use crate::export::types::{ExportFormat, LetterLayout, TemplateId};

/// Unzip a generated DOCX and return its `word/document.xml` (where the body
/// runs and the section's `pgSz` live).
fn document_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("docx is a zip archive");
    let mut file = zip
        .by_name("word/document.xml")
        .expect("docx contains word/document.xml");
    let mut xml = String::new();
    file.read_to_string(&mut xml).expect("read document.xml");
    xml
}

fn resume_request(template_id: TemplateId) -> ExportRequest {
    ExportRequest {
        // Name + contact + section + entry + bullet exercise name/heading/body fonts.
        text: "Jane Doe\njane@example.com\n\nEXPERIENCE\nAcme Corp  2020 - Present\nSenior Engineer\n- Built things that mattered".to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    }
}

/// Cover-letter request builder for the letter-layout DOCX tests (PR5).
fn letter_request(text: &str, layout: LetterLayout) -> ExportRequest {
    ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: layout,
    }
}

const REFINED_US_TEXT: &str = "Jane Smith\njane@example.com | https://linkedin.com/in/janesmith\n\nJune 2, 2025\n\nHiring Manager\nAcme Corp\n\nRe: Application for Platform Engineer (Ref PX-2291)\n\nDear Hiring Manager,\n\nI am writing to express my strong interest in the Platform Engineer position, bringing distributed systems experience.\n\nSincerely,\n\nJane Smith\nSoftware Engineer\n";

const REFINED_DE_TEXT: &str = "Max Müller\nmax@example.de | https://linkedin.com/in/maxmueller\n\nFrankfurt, 2. Juni 2025\n\nFrau Dr. Anna Weber\nMusterfirma GmbH\n\nBetreff: Bewerbung als Software Engineer\n\nSehr geehrte Frau Dr. Weber,\n\nmit großem Interesse habe ich Ihre Stellenausschreibung gelesen und bewerbe mich hiermit.\n\nMit freundlichen Grüßen,\n\nMax Müller\n";

/// Body-only fixture — regression guardrail for the shipped
/// `complete_letter_text` fix. Mirrors `LETTER_FIXTURE_BODY_ONLY_US` in
/// `typst_engine/test.rs` (this file's fixtures are one-line `\n`-escaped;
/// duplicated rather than shared because the two test modules have no
/// production-code seam to reach a common fixture without touching
/// non-test code — this file's German fixture already drifts from the PDF
/// engine's own `LETTER_FIXTURE_DE` vs `REFINED_DE_TEXT` above, so keeping
/// this new pair in step across both files rather than trying to unify them
/// matches that existing precedent). No letterhead, no salutation, no
/// sign-off, no signature — the shape `pipeline::resume::prompts::letter_system`
/// actually asks the model for. One `**bold**` keyword.
const LETTER_FIXTURE_BODY_ONLY_US: &str = "I am writing to express my strong interest in the Software Engineer position, where I would bring five years of experience building distributed systems in Rust and Go to a team solving problems at real scale.\n\nDuring my time at Beta Inc, I led the migration of our payments service to a **microservices** architecture, reducing end-to-end latency by 40 percent and cutting infrastructure costs by 30 percent.\n\nI would welcome the opportunity to discuss how my background aligns with your team's needs and how I could contribute from day one.";

/// German body-only fixture — mirrors `LETTER_FIXTURE_BODY_ONLY_DE` in
/// `typst_engine/test.rs`: a long opening paragraph, a paragraph with digits
/// and a mid-sentence period ("von 0 % auf 90 %."), and a `**bold**` keyword.
const LETTER_FIXTURE_BODY_ONLY_DE: &str = "Mit großem Interesse habe ich Ihre Stellenausschreibung für die Position als Software Engineer gelesen und bin überzeugt, dass meine mehrjährige Erfahrung in der Entwicklung verteilter Systeme genau zu den Anforderungen passt, die Sie beschrieben haben.\n\nIn meiner bisherigen Tätigkeit bei der Beta GmbH konnte ich die Testabdeckung von 0 % auf 90 % steigern. Durch die Einführung von **Jest** und einer durchgängigen CI-Pipeline wurde die Codequalität spürbar besser.\n\nÜber eine Einladung zum Vorstellungsgespräch würde ich mich sehr freuen und stehe für Rückfragen jederzeit zur Verfügung.";

/// Every `w:sz w:val="N"` (half-points) found in a DOCX body, in document order.
/// Deliberately does not match `w:szCs` (the companion complex-script size,
/// same value) — the literal `w:sz w:val="` substring requires a space right
/// after `sz`, which `szCs` never has.
fn all_font_sizes(xml: &str) -> Vec<u32> {
    let needle = "w:sz w:val=\"";
    let mut sizes = Vec::new();
    let mut rest = xml;
    while let Some(idx) = rest.find(needle) {
        let after = &rest[idx + needle.len()..];
        let end = after
            .find('"')
            .expect("w:sz w:val opening quote must close");
        sizes.push(
            after[..end]
                .parse::<u32>()
                .expect("w:sz w:val must be numeric"),
        );
        rest = &after[end..];
    }
    sizes
}

#[test]
fn cover_letter_docx_emits_half_point_sizes_not_dxa() {
    // Classic template: name_pt 20.0, body_pt 10.5 (`templates/mod.rs`). The
    // regression this guards: routing font size through `pt_to_dxa` (×20)
    // instead of `pt_to_half_points` (×2) produced `w:sz w:val="400"`/`"210"` —
    // a 200pt/105pt name and body, the exact defect behind the 132-page export.
    let bytes = generate_docx(&letter_request(
        "Jane Doe\njane@example.com\n\nDear Hiring Manager,\n\nI am writing to apply.\n\nSincerely,\nJane Doe",
        LetterLayout::Classic,
    ))
    .expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:sz w:val="40""#),
        "Classic name_pt 20.0 must emit w:sz w:val=\"40\" (half-points), not a dxa value: {xml}"
    );
    assert!(
        xml.contains(r#"w:sz w:val="21""#),
        "Classic body_pt 10.5 must emit w:sz w:val=\"21\" (half-points), not a dxa value: {xml}"
    );
}

#[test]
fn no_cover_letter_font_size_exceeds_sane_ceiling() {
    // Independent of any one template's literal numbers: no half-point run size
    // should ever exceed 100 (50pt). This is the guard that would have caught
    // the pt_to_dxa/pt_to_half_points class outright — every real template's
    // pt sizes top out well under 40pt, so 50pt is a generous, stable ceiling.
    const MAX_HALF_POINTS: u32 = 100;
    for template_id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
        TemplateId::Atelier,
        TemplateId::Meridian,
        TemplateId::Throughline,
        TemplateId::Cadence,
        TemplateId::Regent,
        TemplateId::Jake,
        TemplateId::Awesome,
        TemplateId::Deedy,
    ] {
        for layout in [
            LetterLayout::Classic,
            LetterLayout::Refined,
            LetterLayout::Banded,
            LetterLayout::Navy,
            LetterLayout::Sidebar,
            LetterLayout::Monogram,
        ] {
            let request = ExportRequest {
                text: REFINED_US_TEXT.to_string(),
                format: ExportFormat::Docx,
                document_type: DocumentType::CoverLetter,
                template_id,
                meta: None,
                ats_mode: false,
                locale: None,
                contact: None,
                accent: None,
                letter_layout: layout,
            };
            let xml = document_xml(&generate_docx(&request).expect("docx"));
            for size in all_font_sizes(&xml) {
                assert!(
                    size <= MAX_HALF_POINTS,
                    "{template_id:?}/{layout:?}: w:sz={size} half-points ({}pt) exceeds the sane ceiling — \
                     likely a font size routed through pt_to_dxa instead of pt_to_half_points",
                    size as f32 / 2.0
                );
            }
        }
    }
}

#[test]
fn resume_docx_declares_a4_page_size() {
    let bytes = generate_docx(&resume_request(TemplateId::SwissMinimal)).expect("docx");
    let xml = document_xml(&bytes);
    // A4 in dxa, set explicitly from LocaleProfile rather than inherited.
    assert!(
        xml.contains(r#"w:w="11906""#) && xml.contains(r#"w:h="16838""#),
        "resume DOCX should declare an explicit A4 page size, got sectPr in: {xml}"
    );
}

#[test]
fn us_locale_drives_letter_page_size() {
    let mut request = resume_request(TemplateId::SwissMinimal);
    request.locale = Some("us".to_string());
    let xml = document_xml(&generate_docx(&request).expect("docx"));
    // US Letter in dxa (12240 × 15840), not the A4 default.
    assert!(
        xml.contains(r#"w:w="12240""#) && xml.contains(r#"w:h="15840""#),
        "US locale should yield a Letter page size"
    );

    // No locale → international A4.
    let a4 = document_xml(&generate_docx(&resume_request(TemplateId::SwissMinimal)).expect("docx"));
    assert!(a4.contains(r#"w:w="11906""#), "default stays A4");
}

#[test]
fn cover_letter_docx_declares_a4_page_size() {
    let request = ExportRequest {
        text: "Dear Hiring Manager,\n\nI am writing to apply.\n\nSincerely,\nJane Doe".to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_docx(&request).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:w="11906""#) && xml.contains(r#"w:h="16838""#),
        "cover-letter DOCX should declare an explicit A4 page size"
    );
}

#[test]
fn resume_docx_uses_fallback_fonts_not_bundled_names() {
    // Meridian: name/heading/body all Inter → Calibri.
    let bytes = generate_docx(&resume_request(TemplateId::Meridian)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:ascii="Calibri""#),
        "Inter should fall back to Calibri"
    );
    // Both ranges are set so accented Latin renders in the same face.
    assert!(
        xml.contains(r#"w:hAnsi="Calibri""#),
        "fallback must also cover the high-ANSI range"
    );
    let bundled = "Inter";
    assert!(
        !xml.contains(&format!(r#""{bundled}""#)),
        "un-embedded bundled font {bundled:?} must not be referenced in the DOCX"
    );
}

#[test]
fn serif_and_display_templates_fall_back_predictably() {
    // Academic: Source Serif 4 → Georgia.
    let academic =
        document_xml(&generate_docx(&resume_request(TemplateId::Academic)).expect("docx"));
    assert!(
        academic.contains(r#"w:ascii="Georgia""#),
        "Source Serif 4 should fall back to Georgia"
    );
    assert!(
        !academic.contains(r#""Source Serif 4""#),
        "bundled Source Serif 4 must not leak"
    );

    // SwissMinimal: Manrope → Calibri.
    let swiss =
        document_xml(&generate_docx(&resume_request(TemplateId::SwissMinimal)).expect("docx"));
    assert!(
        swiss.contains(r#"w:ascii="Calibri""#),
        "Manrope should fall back to Calibri"
    );
    assert!(
        !swiss.contains(r#""Manrope""#),
        "bundled Manrope must not leak"
    );
}

#[test]
fn test_generate_simple_resume() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com\n\nEXPERIENCE\nSoftware Engineer  2020-2023".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
fn test_extract_section_with_markers() {
    let text = "Header\n### START ###\nContent\n### END ###\nFooter";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Content");
}

#[test]
fn test_extract_section_no_start() {
    let text = "Content\n### END ###\nFooter";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Content\n### END ###\nFooter");
}

#[test]
fn test_extract_section_no_end() {
    let text = "Header\n### START ###\nContent\nMore";
    let result = extract_section(text, "### START ###", None);
    assert_eq!(result, "Content\nMore");
}

#[test]
fn test_extract_section_empty_text() {
    let text = "";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "");
}

#[test]
fn test_extract_section_no_markers() {
    let text = "Just some text";
    let result = extract_section(text, "NONEXISTENT", None);
    assert_eq!(result, "Just some text");
}

#[test]
fn test_generate_cover_letter() {
    let request = ExportRequest {
        text: "Dear Hiring Manager,\n\nI am writing to apply for the position.\n\nSincerely,\nJohn Doe".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
fn document_accent_overrides_docx_emphasis_color() {
    use crate::export::docx_renderer::setup_colors;
    use crate::export::templates::Template;

    // The DOCX backend derives its emphasis color from the template's
    // `emphasis_color`, which `with_accent_override` recolors — so a document
    // accent surfaces on emphasized runs. Non-accent colors (e.g. section) stay put.
    let base = setup_colors(&Template::get(TemplateId::Classic));
    let accented =
        setup_colors(&Template::get(TemplateId::Classic).with_accent_override(Some("#AA0000")));
    assert_eq!(
        accented.emphasis, "AA0000",
        "accent must recolor DOCX emphasis"
    );
    assert_ne!(
        base.emphasis, accented.emphasis,
        "override must actually change the emphasis color"
    );
    assert_eq!(
        accented.section, base.section,
        "a non-accent color (section) must be untouched"
    );
}

#[test]
fn test_generate_resume_with_meta() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: Some(GenerationMeta {
            candidate_name: Some("Jane Smith".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Test Corp".to_string()),
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
}

// ── PR5: Letter layout DOCX wiring ────────────────────────────────────────────
//
// `generate_cover_letter_docx` previously ignored `request.letter_layout`
// entirely, so a Banded/Refined choice never reached the DOCX export (a
// preview/export honesty violation). These tests lock in the fix.

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

/// Sidebar's DOCX approximates the tinted rail as paragraph shading behind the
/// name (there is no margin-anchored frame in DOCX, and a text box or a
/// two-column table would be exactly the multi-column trap the export avoids)
/// and keeps the contact at the LEFT margin, because the rail stacks it under
/// the name rather than pulling it to the right edge.
///
/// The contact alignment is the assertion that matters: it was a function of
/// "centred or right" before this layout existed, so a Sidebar that forgot to
/// state its own alignment would silently right-align — the same
/// inherit-by-omission defect that took four review rounds on Navy.
#[test]
fn sidebar_docx_shades_the_name_and_left_aligns_the_contact() {
    let xml = document_xml(
        &generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Sidebar)).expect("sidebar"),
    );
    // Classic's accent #222222 lightened 85 % toward white → #DEDEDE, the same
    // `band_tint_hex` Banded uses, so PDF and DOCX show one tint.
    assert!(
        xml.contains("w:shd") && xml.contains(r#"w:fill="DEDEDE""#),
        "Sidebar must approximate the rail with the lightened-accent shading: {xml}"
    );
    assert!(
        xml.contains(r#"w:jc w:val="left""#),
        "Sidebar must LEFT-align the contact line — the rail stacks it under the name: {xml}"
    );
    assert!(
        !xml.contains(r#"w:jc w:val="right""#),
        "Sidebar must not inherit Refined/Banded's right-aligned contact: {xml}"
    );
    assert!(
        xml.contains("Jane Smith"),
        "Sidebar must keep the name as written (no uppercasing): {xml}"
    );
}

/// Monogram's DOCX device is a shaded RUN at the head of the name paragraph,
/// carrying the SAME initials the `.typ` gets from `LetterHead.initials` — both
/// call `monogram_initials`, so the two formats cannot disagree about what the
/// device says, and both extract "JS Jane Smith" rather than putting the
/// initials on a line of their own.
#[test]
fn monogram_docx_prefixes_the_name_with_the_shaded_initials() {
    let xml = document_xml(
        &generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Monogram)).expect("monogram"),
    );
    // Run shading, not paragraph shading: the initials sit BESIDE the name in
    // the `.typ`, so they must share its paragraph.
    let name_para = xml
        .split("<w:p>")
        .find(|p| p.contains("Jane Smith"))
        .expect("a paragraph containing the name");
    let runs: Vec<&str> = name_para.split("<w:r>").collect();

    let initials_run = runs
        .iter()
        .find(|r| r.contains(">JS<"))
        .unwrap_or_else(|| panic!("no run carries the initials: {name_para}"));
    assert!(
        initials_run.contains("w:shd") && initials_run.contains(r#"w:fill="DEDEDE""#),
        "the Monogram initials run must carry the accent-tint shading: {initials_run}"
    );

    // The gap between device and name must sit OUTSIDE the tile. docx-rs always
    // writes `xml:space="preserve"`, so spaces bundled into the shaded run get
    // painted and the tile runs on past the initials — which the `.typ` square
    // never does.
    let sep_run = runs
        .iter()
        .find(|r| r.contains(">  <"))
        .unwrap_or_else(|| panic!("no separator run between the device and the name: {name_para}"));
    assert!(
        !sep_run.contains("w:shd"),
        "the separator spaces must NOT be shaded — the tint would extend past the \
         initials: {sep_run}"
    );
    assert!(
        xml.contains(r#"w:jc w:val="left""#),
        "Monogram must left-align the contact line under the lockup: {xml}"
    );
}

/// ATS mode drops the decorative tint in DOCX exactly as it does in the PDF.
/// Without this the two formats disagree: the user turns the toggle on, the PDF
/// loses its band and the Word file keeps it.
#[test]
fn ats_mode_drops_every_letter_docx_tint() {
    for layout in [
        LetterLayout::Banded,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let design = document_xml(
            &generate_docx(&letter_request(REFINED_US_TEXT, layout)).expect("design docx"),
        );
        let mut ats_request = letter_request(REFINED_US_TEXT, layout);
        ats_request.ats_mode = true;
        let ats = document_xml(&generate_docx(&ats_request).expect("ats docx"));

        assert!(
            design.contains(r#"w:fill="DEDEDE""#),
            "precondition: {layout:?} is expected to carry the accent tint in design mode"
        );
        assert!(
            !ats.contains(r#"w:fill="DEDEDE""#),
            "{layout:?} must drop its accent tint under ATS mode: {ats}"
        );
        // Degradation loses decoration, not words.
        for needle in ["Jane Smith", "Dear Hiring Manager", "distributed systems"] {
            assert!(
                ats.contains(needle),
                "ATS-mode {layout:?} DOCX dropped {needle:?}: {ats}"
            );
        }
    }
}

/// The DE market caption, in DOCX, for every caption-bearing layout.
///
/// Every other `letter_request` in this file uses `locale: None`, which resolves
/// to the label-less `intl` market — so nothing here had ever exercised a market
/// that HAS a subject label, and the DOCX suppression rule
/// (`strip_market_label` + `has_own_label`) was running untested. That gap is
/// what let the PDF side ship the duplicate: with no DE coverage on either side,
/// the two formats could disagree silently.
///
/// Asserts the label renders exactly ONCE — the caption is emitted uppercased
/// and the body keeps the market's own casing, so a duplicate shows up as two
/// case-insensitive matches.
#[test]
fn cover_letter_docx_renders_the_de_market_label_exactly_once() {
    for layout in [
        LetterLayout::Refined,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let mut request = letter_request(REFINED_DE_TEXT, layout);
        request.locale = Some("de".to_string());
        let xml = document_xml(&generate_docx(&request).expect("de docx"));

        // Text nodes only — attribute values never carry the label.
        let body: String = xml.to_lowercase();
        let count = body.matches("betreff").count();
        assert_eq!(
            count, 1,
            "{layout:?}: the DE label must appear exactly once in the DOCX, found {count} \
             — the caption is duplicating the label data.subject already carries: {xml}"
        );
        assert!(
            xml.contains("Bewerbung als Software Engineer"),
            "{layout:?}: the DE subject body went missing: {xml}"
        );
        // Same isolating check as the PDF side: the colon only survives on an
        // unstripped body, so this pins WHICH of the two occurrences was kept.
        assert!(
            !body.contains("betreff: bewerbung"),
            "{layout:?}: the label was left on the subject body instead of being stripped \
             into the caption: {xml}"
        );
    }
}

/// A letter whose first line is a DATE has no letterhead name, and the device
/// must not invent one from it.
///
/// This renderer's line filter excludes a salutation, a sign-off and a subject
/// — and nothing else — so a date reached the name branch and `12 March 2025`
/// put `12` in the device. Fixed by routing through the SHARED
/// `letterhead_initials` the `.typ` side already used, rather than the
/// unguarded `monogram_initials` this file used to call: one guard, so the two
/// formats cannot disagree about which openings are not names.
#[test]
fn monogram_docx_emits_no_device_for_a_date_opening() {
    const DATE_FIRST: &str =
        "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";

    for candidate_name in [None, Some(String::new())] {
        let mut request = letter_request(DATE_FIRST, LetterLayout::Monogram);
        request.meta = candidate_name.clone().map(|candidate_name| GenerationMeta {
            candidate_name: Some(candidate_name),
            job_title: None,
            company_name: None,
            target_language: None,
        });
        let xml = document_xml(&generate_docx(&request).expect("date-opening monogram docx"));
        assert!(
            !xml.contains(">12<"),
            "candidate_name={candidate_name:?}: the device must not read `12` off the date line: {xml}"
        );
        // …and NO device at all, not merely a different one. Asserting only
        // `!">12<"` passes on the `is_name_token` letters-only rule alone —
        // that rule turns "12 March 2025" into `M`, a device built from the
        // month. It is the date guard, not the token rule, that has to refuse
        // the line outright, and only this assertion can tell them apart.
        assert!(
            !xml.contains("w:shd"),
            "candidate_name={candidate_name:?}: a date opening must produce NO monogram device \
             (found the shaded tile): {xml}"
        );
    }
}

/// The Monogram device is TEXT, so ATS mode must remove the initials
/// themselves — not merely their shading.
#[test]
fn ats_mode_drops_the_monogram_initials_from_docx() {
    let mut request = letter_request(REFINED_US_TEXT, LetterLayout::Monogram);
    request.ats_mode = true;
    let ats = document_xml(&generate_docx(&request).expect("ats monogram docx"));
    assert!(
        !ats.contains(">JS<"),
        "ATS-mode Monogram must not emit the initials run — they extract as noise \
         before the name: {ats}"
    );
    assert!(
        ats.contains("Jane Smith"),
        "ATS-mode Monogram must still render the name: {ats}"
    );
}

/// Navy's DOCX must match Navy's PDF, not Banded's.
///
/// The renderer branched on a single `is_refined` boolean, so every non-Refined
/// layout got Banded's treatment. Two review rounds were needed to find them
/// all, because "differs from Banded" passes as soon as ONE branch is split —
/// these assert the specific features instead.
#[test]
fn navy_docx_follows_the_navy_design_not_banded() {
    let navy = document_xml(
        &generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Navy)).expect("navy"),
    );
    let banded = document_xml(
        &generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Banded)).expect("banded"),
    );

    // 1. No header band. `letter_navy.typ` has no shaded block; Banded does, and
    //    Navy silently inherited it.
    assert!(
        banded.contains("<w:shd"),
        "precondition: Banded is expected to carry the shaded band"
    );
    assert!(
        !navy.contains("<w:shd"),
        "Navy must not render Banded's shaded header band"
    );

    // 2. Centred letterhead — the NAME AND the contact line, not just one.
    //    `letter_request` supplies `contact: None`, so these exercise the
    //    no-profile FALLBACK contact path, which used to right-align Navy while
    //    the profile-backed path centred it. A single "contains center" check
    //    passed anyway, because the name alone satisfied it.
    let centred = |xml: &str| xml.matches(r#"w:val="center""#).count();
    let right = |xml: &str| xml.matches(r#"w:val="right""#).count();
    assert!(
        centred(&navy) >= 2,
        "Navy must centre the name AND the contact line; found {} centred paragraph(s)",
        centred(&navy)
    );
    assert_eq!(centred(&banded), 0, "precondition: Banded centres nothing");
    assert!(
        right(&navy) < right(&banded),
        "Navy must not right-align the header lines Banded does: navy={} banded={}",
        right(&navy),
        right(&banded)
    );

    // 3. Date and recipient stay REGULAR weight — `letter_navy.typ`'s
    //    emit-date-block / emit-recipient-block carry no `weight: "bold"`,
    //    unlike Banded's. Bold-run count is the observable proxy.
    let bold_runs = |xml: &str| xml.matches("<w:b />").count();
    assert!(
        bold_runs(&navy) < bold_runs(&banded),
        "Navy bolds fewer runs than Banded (it does not bold date/recipient):          navy={} banded={}",
        bold_runs(&navy),
        bold_runs(&banded)
    );
}

/// Navy's role line and subject caption must use NAVY's styling, not Refined's.
///
/// The style struct made each feature's PRESENCE layout-aware but left its
/// STYLING hardcoded to Refined's, so Navy rendered the role line
/// accent-coloured, uppercased and letter-spaced while `letter_navy.typ` puts it
/// in the muted date colour, plain case, untracked — and the subject caption in
/// the accent colour where the `.typ` uses the name colour. Presence and style
/// are separate decisions; asserting only presence missed both.
#[test]
fn navy_docx_styles_the_title_and_caption_like_its_typ() {
    let with_title = |layout: LetterLayout| {
        let mut req = letter_request(REFINED_US_TEXT, layout);
        req.meta = Some(GenerationMeta {
            candidate_name: Some("Jane Smith".to_string()),
            job_title: Some("Platform Engineer".to_string()),
            company_name: None,
            target_language: None,
        });
        document_xml(&generate_docx(&req).expect("docx"))
    };

    let navy = with_title(LetterLayout::Navy);
    let refined = with_title(LetterLayout::Refined);

    // Refined uppercases and tracks its role line; Navy does neither.
    assert!(
        refined.contains("PLATFORM ENGINEER"),
        "precondition: Refined uppercases the role line"
    );
    assert!(
        navy.contains("Platform Engineer"),
        "Navy must keep the role line in its original case"
    );
    assert!(
        !navy.contains("PLATFORM ENGINEER"),
        "Navy must not uppercase the role line — letter_navy.typ renders it as written"
    );

    // Letter-spacing is Refined-only (`character_spacing` ⇒ `<w:spacing w:val=…>`
    // on the run). Navy's role line carries none.
    let spaced_runs = |xml: &str| xml.matches("w:spacing w:val=\"24\"").count();
    assert!(
        spaced_runs(&refined) > spaced_runs(&navy),
        "Refined tracks more runs than Navy: refined={} navy={}",
        spaced_runs(&refined),
        spaced_runs(&navy)
    );
}

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

/// Split `word/document.xml` into one slice per real `<w:p …>` paragraph
/// element. NOT `xml.split("<w:p>")` (the pattern the Monogram test above
/// uses): every paragraph this crate's docx-rs version emits carries a
/// `w14:paraId="…"` attribute (`<w:p w14:paraId="00000001">`), so the bare
/// `"<w:p>"` literal never actually occurs and that split silently returns
/// the WHOLE document as one chunk — harmless for that test (it only reads
/// runs out of the single resulting chunk) but useless for telling two
/// paragraphs apart, which is exactly what this test needs. Matches on the
/// character immediately after `<w:p` being `>` or a space, which is true
/// for a real paragraph tag but false for `<w:pPr>`/`<w:pStyle …>` (`P`/`S`
/// follow directly with no separator).
fn docx_paragraphs(xml: &str) -> Vec<&str> {
    let bytes = xml.as_bytes();
    let marker = b"<w:p";
    let mut starts: Vec<usize> = bytes
        .windows(marker.len() + 1)
        .enumerate()
        .filter_map(|(i, w)| {
            (&w[..marker.len()] == marker && (w[marker.len()] == b'>' || w[marker.len()] == b' '))
                .then_some(i)
        })
        .collect();
    starts.push(xml.len());
    starts.windows(2).map(|w| &xml[w[0]..w[1]]).collect()
}

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

/// #28 regression guard: the letterhead name paragraph used to carry only
/// `w:after="60"` (3pt) in both cover-letter DOCX renderers
/// (`generate_cover_letter_docx_classic` for `LetterLayout::Classic`,
/// `generate_cover_letter_docx_layout` for every other layout) — crammed
/// against the contact line below, the same shape as the PDF `letter_*.typ`
/// bug. Both now emit `w:after="180"` (9pt = `pt_to_dxa(9.0)`, matching
/// `_scale.typ`'s `sp-name-below`). Checked on one layout from each renderer.
#[test]
fn cover_letter_docx_name_paragraph_has_explicit_nine_point_spacing() {
    for layout in [LetterLayout::Classic, LetterLayout::Navy] {
        let xml =
            document_xml(&generate_docx(&letter_request(REFINED_US_TEXT, layout)).expect("docx"));
        // Navy (and Banded) uppercase the letterhead name (`uppercase_name`
        // in `LetterDocxStyle`), so search both cases and take whichever
        // occurs FIRST — the letterhead is always the earliest paragraph in
        // the document; a later, plain-cased "Jane Smith" also appears in
        // the signature block and would match the wrong paragraph.
        let name_idx = [">Jane Smith<", ">JANE SMITH<"]
            .into_iter()
            .filter_map(|needle| xml.find(needle))
            .min()
            .unwrap_or_else(|| panic!("{layout:?}: letterhead name run must be present"));
        let para_start = xml[..name_idx]
            .rfind("<w:p>")
            .or_else(|| xml[..name_idx].rfind("<w:p "))
            .unwrap_or_else(|| panic!("{layout:?}: name run must sit inside a <w:p> paragraph"));
        let para_head = &xml[para_start..name_idx];
        assert!(
            para_head.contains(r#"w:after="180""#),
            "{layout:?}: name paragraph must declare `w:after=\"180\"` (9pt) \
             spacing before the run reaches the contact line; paragraph head: {para_head}"
        );
    }
}
