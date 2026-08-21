//! Golden invariants for the model-based DOCX backend. Each test builds a real
//! DOCX and inspects the unzipped OOXML parts (`document.xml` and the hyperlink
//! relationships), since DOCX is flow-based and has no pixel geometry to compare.

use std::io::{Cursor, Read};

use super::*;
use crate::export::types::TemplateId;

const RESUME: &str = "\
Jane Doe
jane@example.com | [LinkedIn](https://linkedin.com/in/jane)

Experienced engineer building reliable web applications end to end.

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform

SKILLS
- Rust, TypeScript, React

EDUCATION
State University  2013 - 2017
BSc Computer Science
";

fn build(template_id: TemplateId, ats_mode: bool) -> Vec<u8> {
    let template = Template::get(template_id);
    let docx = generate_resume_docx(RESUME, None, &template, ats_mode).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    buffer.into_inner()
}

/// The entry date run is italic (not bold) — matches the PDF path
/// (`single_column.typ`'s date-str run) so the duration reads as a
/// consistently distinguishable, fast-to-scan element across both export
/// formats, not just in the PDF.
#[test]
fn entry_date_run_is_italic_matching_pdf() {
    let bytes = build(TemplateId::Classic, false);
    let xml = part(&bytes, "word/document.xml");
    // "2020 - Present" is the right-aligned date run for the Acme Corp entry
    // (RESUME's legacy two-space format: `right_align_date` is true for
    // Classic's wide-flow layout). Find the run containing that text and
    // confirm it carries `<w:i/>`.
    let idx = xml
        .find("2020 - Present")
        .expect("date text must appear in document.xml");
    let run_start = xml[..idx].rfind("<w:r>").expect("enclosing run start");
    let run_end = idx + xml[idx..].find("</w:r>").expect("enclosing run end");
    let run_xml = &xml[run_start..run_end];
    assert!(
        run_xml.contains("<w:i "),
        "expected the date run to carry <w:i /> (italic); run xml: {run_xml:?}"
    );
    assert!(
        !run_xml.contains("<w:b "),
        "date run must stay non-bold; run xml: {run_xml:?}"
    );
}

/// Read a named part out of the DOCX zip.
fn part(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("docx zip");
    let mut s = String::new();
    zip.by_name(name)
        .expect(name)
        .read_to_string(&mut s)
        .expect("read part");
    s
}

/// Strip XML tags so body text can be checked for content survival.
fn text_of(xml: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Every `w:sz w:val="N"` (half-points) found in a DOCX body, in document
/// order. Deliberately does not match `w:szCs` (the companion complex-script
/// size, same value) — the literal `w:sz w:val="` substring requires a space
/// right after `sz`, which `szCs` never has.
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
fn resume_docx_emits_half_point_sizes_not_dxa() {
    // SwissMinimal: name_pt 22.0, body_pt 10.5 (`templates/mod.rs`). The
    // regression this guards: routing font size through `pt_to_dxa` (×20)
    // instead of `pt_to_half_points` (×2) produced `w:sz w:val="440"`/`"210"` —
    // a 220pt/105pt name and body, the exact defect behind the 132-page export.
    let xml = part(&build(TemplateId::SwissMinimal, false), "word/document.xml");
    assert!(
        xml.contains(r#"w:sz w:val="44""#),
        "SwissMinimal name_pt 22.0 must emit w:sz w:val=\"44\" (half-points), not a dxa value: {xml}"
    );
    assert!(
        xml.contains(r#"w:sz w:val="21""#),
        "SwissMinimal body_pt 10.5 must emit w:sz w:val=\"21\" (half-points), not a dxa value: {xml}"
    );
}

#[test]
fn no_resume_font_size_exceeds_sane_ceiling() {
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
        for ats_mode in [false, true] {
            let xml = part(&build(template_id, ats_mode), "word/document.xml");
            for size in all_font_sizes(&xml) {
                assert!(
                    size <= MAX_HALF_POINTS,
                    "{template_id:?} (ats_mode={ats_mode}): w:sz={size} half-points ({}pt) exceeds \
                     the sane ceiling — likely a font size routed through pt_to_dxa instead of \
                     pt_to_half_points",
                    size as f32 / 2.0
                );
            }
        }
    }
}

#[test]
fn two_column_renders_a_borderless_shaded_table() {
    let xml = part(&build(TemplateId::Atelier, false), "word/document.xml");
    assert!(xml.contains("<w:tbl"), "two-column DOCX must use a table");
    // Atelier sidebar tint (240,239,248) → F0EFF8 fill on the sidebar cell.
    assert!(
        xml.contains(r#"w:fill="F0EFF8""#),
        "sidebar cell should carry the Atelier tint"
    );
}

/// Awesome's PDF (`awesome.typ`) draws a full-width accent-tinted header band.
/// DOCX has no page-background primitive, so `add_header` approximates it with
/// the pairing the Banded cover-letter layout already established
/// (`docx::mod`'s `header_band` branch, pinned by
/// `cover_letter_docx_banded_shades_name_paragraph_and_uppercases`):
/// PARAGRAPH-level shading filled with the accent lightened 85 % toward white,
/// keeping the normal dark ink.
///
/// The shape this rules out is the inverse — run-level `w:shd` in the raw
/// accent behind hardcoded `FFFFFF` text. That tints only the glyph boxes, and
/// wherever run shading is not honoured the name is white on white: exactly the
/// hazard `awesome_matches_spec`'s `assert_ne!(name_color, white)` guards the
/// registry against, reintroduced in the renderer.
#[test]
fn awesome_name_paragraph_is_shaded_with_a_pale_tint_and_keeps_dark_ink() {
    let xml = part(&build(TemplateId::Awesome, false), "word/document.xml");

    // Accent #C41E3A lightened 85 % toward white → #F6DDE1
    // (196 + (255-196)*0.85 ≈ 246 = F6, 30 + 225*0.85 ≈ 221 = DD,
    //  58 + 197*0.85 ≈ 225 = E1 — `docx::band_tint_hex`).
    assert!(
        xml.contains(r#"w:fill="F6DDE1""#),
        "awesome header band must be the accent lightened 85% toward white: {xml}"
    );
    assert!(
        !xml.contains(r#"w:fill="C41E3A""#),
        "the raw accent is far too dark to read normal ink on — the band fill \
         must be the pale tint, not the accent itself: {xml}"
    );
    // Dark ink, never white: `name_color` is (26,26,26) → 1A1A1A.
    assert!(
        xml.contains(r#"w:color w:val="1A1A1A""#),
        "awesome name must keep the registry's dark ink: {xml}"
    );
    assert!(
        !xml.contains(r#"w:color w:val="FFFFFF""#),
        "white DOCX name text is invisible wherever the shading is dropped: {xml}"
    );

    // The shading must sit inside the paragraph properties (`w:pPr`), not in a
    // run — run-level `w:shd` tints only the glyph boxes, so it reads as a
    // highlighter stripe the width of the name rather than a header band.
    let ppr_open = xml.find("<w:pPr").expect("header paragraph must have pPr");
    let shd = xml.find("<w:shd").expect("header paragraph must be shaded");
    let ppr_close = xml.find("</w:pPr>").expect("pPr must close");
    assert!(
        ppr_open < shd && shd < ppr_close,
        "w:shd must be inside the first w:pPr (paragraph-level shading), not on a \
         run — got pPr@{ppr_open} shd@{shd} /pPr@{ppr_close}: {xml}"
    );

    // Control: a template with no band special-case must NOT gain shading —
    // this would fail if the `TemplateId::Awesome` branch leaked to everyone.
    let classic_xml = part(&build(TemplateId::Classic, false), "word/document.xml");
    assert!(
        !classic_xml.contains("w:shd"),
        "classic must not gain shading it never had: {classic_xml}"
    );
}

/// The band is decorative colour, so the ATS toggle drops it — the DOCX mirror
/// of the PDF's `awesome_ats_mode_drops_the_header_band_and_section_markers`.
/// `add_header` took no `ats_mode` at all, so an ATS-mode DOCX kept a band the
/// ATS-mode PDF had already dropped: the same request producing two documents
/// that disagree about what "ATS mode" means.
#[test]
fn awesome_ats_mode_drops_the_docx_header_band() {
    let ats_xml = part(&build(TemplateId::Awesome, true), "word/document.xml");
    assert!(
        !ats_xml.contains("w:shd"),
        "ATS mode must drop the Awesome header band entirely: {ats_xml}"
    );
    // …and the name must still be there, in dark ink.
    assert!(
        text_of(&ats_xml).contains("Jane Doe"),
        "dropping the band must not drop the name: {ats_xml}"
    );

    // Mutation guard: the non-ATS export of the very same fixture DOES shade,
    // so the assertion above is about `ats_mode`, not about shading never
    // being emitted.
    let banded_xml = part(&build(TemplateId::Awesome, false), "word/document.xml");
    assert!(
        banded_xml.contains("w:shd"),
        "non-ATS Awesome must still draw the band: {banded_xml}"
    );
}

#[test]
fn two_column_splits_sections_between_columns() {
    // SKILLS + EDUCATION are sidebar sections; EXPERIENCE is a main section.
    // All three must survive somewhere in the document.
    let text = text_of(&part(
        &build(TemplateId::Atelier, false),
        "word/document.xml",
    ));
    for needle in ["EXPERIENCE", "SKILLS", "EDUCATION", "Acme Corp", "Rust"] {
        assert!(text.contains(needle), "two-column lost content: {needle:?}");
    }
}

#[test]
fn ats_mode_emits_no_table() {
    // ATS mode linearizes to a single column — no two-column table.
    let xml = part(&build(TemplateId::Atelier, true), "word/document.xml");
    assert!(
        !xml.contains("<w:tbl"),
        "ATS mode must not emit a two-column table"
    );
    let text = text_of(&xml);
    for needle in ["EXPERIENCE", "SKILLS", "EDUCATION"] {
        assert!(
            text.contains(needle),
            "ATS linearization dropped {needle:?}"
        );
    }
}

#[test]
fn single_column_template_has_no_table() {
    let xml = part(&build(TemplateId::SwissMinimal, false), "word/document.xml");
    assert!(
        !xml.contains("<w:tbl"),
        "single-column template must not use a table"
    );
    let text = text_of(&xml);
    for needle in [
        "Jane Doe",
        "EXPERIENCE",
        "Rust, TypeScript, React",
        "BSc Computer Science",
    ] {
        assert!(
            text.contains(needle),
            "single-column lost content: {needle:?}"
        );
    }
}

#[test]
fn contact_links_become_hyperlinks_with_correct_targets() {
    let bytes = build(TemplateId::SwissMinimal, false);
    let doc = part(&bytes, "word/document.xml");
    assert!(
        doc.contains("<w:hyperlink"),
        "contact links must render as hyperlinks"
    );

    // External hyperlink targets live in the relationships part.
    let rels = part(&bytes, "word/_rels/document.xml.rels");
    assert!(
        rels.contains("https://linkedin.com/in/jane"),
        "LinkedIn URL must be a hyperlink target"
    );
    assert!(
        rels.contains("mailto:jane@example.com"),
        "email must be a mailto hyperlink target"
    );

    // The visible label, not the raw URL, is shown.
    let text = text_of(&doc);
    assert!(text.contains("LinkedIn"), "link label should display");
    assert!(
        !text.contains("https://linkedin.com/in/jane"),
        "raw URL must not be visible text"
    );
}

// ── candidate_name metadata is a fallback, not an override (H) ───────────────

#[test]
fn candidate_name_metadata_is_fallback_when_text_has_a_name() {
    let template = Template::get(TemplateId::SwissMinimal);
    let meta = GenerationMeta {
        candidate_name: Some("Someone Else".to_string()),
        job_title: None,
        company_name: None,
        target_language: None,
    };
    let docx = generate_resume_docx(RESUME, Some(&meta), &template, false).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    let text = text_of(&part(&buffer.into_inner(), "word/document.xml"));
    assert!(
        text.contains("Jane Doe"),
        "text-derived name must win over meta.candidate_name"
    );
    assert!(
        !text.contains("Someone Else"),
        "metadata name must not override a text-derived name"
    );
}

#[test]
fn candidate_name_metadata_fills_header_when_text_has_none() {
    let template = Template::get(TemplateId::SwissMinimal);
    let text = "jane@example.com\n\nSUMMARY\nSome text.";
    // Padded on purpose: the emptiness check (`!name.trim().is_empty()`) used
    // to trim while the assignment (`model.header.name = name.to_string()`)
    // didn't, so a padded metadata name rendered with stray leading/trailing
    // whitespace baked into the header run.
    let meta = GenerationMeta {
        candidate_name: Some("  Jane Smith  ".to_string()),
        job_title: None,
        company_name: None,
        target_language: None,
    };
    let docx = generate_resume_docx(text, Some(&meta), &template, false).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    let bytes = buffer.into_inner();
    let xml = part(&bytes, "word/document.xml");
    // The run text itself must be exactly the trimmed name — bounded
    // immediately by tags, no leaked interior whitespace from the untrimmed
    // metadata field.
    assert!(
        xml.contains(">Jane Smith<"),
        "the header run must contain the trimmed name with no stray \
         whitespace: {xml}"
    );
    // Not just "appears somewhere in the body" — it must land as the header,
    // first in the document, not e.g. folded into a later section by a
    // fallback that reached the wrong branch.
    let doc_text = text_of(&xml);
    assert!(
        doc_text.trim_start().starts_with("Jane Smith"),
        "metadata name must fill the header and land first in the document, \
         not merely appear somewhere in the body: {doc_text:?}"
    );
}

#[test]
fn declares_a4_page_size_and_fallback_fonts() {
    // Academic: name/heading/body all SourceSerif4 → Georgia.
    let xml = part(&build(TemplateId::Academic, false), "word/document.xml");
    assert!(
        xml.contains(r#"w:w="11906""#) && xml.contains(r#"w:h="16838""#),
        "A4 page size"
    );
    assert!(
        xml.contains(r#"w:ascii="Georgia""#),
        "SourceSerif4 → Georgia"
    );
    let bundled = "Source Serif 4";
    assert!(
        !xml.contains(&format!(r#""{bundled}""#)),
        "bundled font {bundled:?} must not leak"
    );
}

/// #28 regression guard: the résumé DOCX header's candidate-name paragraph
/// used to carry NO explicit spacing at all, leaving the name→contact gap to
/// whatever Word's own default paragraph spacing happens to be. `add_header`
/// now sets `w:after="180"` (9pt = `pt_to_dxa(9.0)`, matching `_scale.typ`'s
/// `sp-name-below`) on that paragraph explicitly. Checked by locating the
/// `w:spacing` tag immediately preceding the name's own `w:t` run — DOCX has
/// no pixel geometry to measure (see this file's module doc comment), so this
/// is the OOXML-part equivalent of the render-based checks in
/// `typst_engine::test`.
#[test]
fn resume_docx_header_name_paragraph_has_explicit_spacing_before_contact() {
    let xml = part(&build(TemplateId::SwissMinimal, false), "word/document.xml");
    let name_idx = xml
        .find(">Jane Doe<")
        .expect("candidate name run must be present in document.xml");
    let para_start = xml[..name_idx]
        .rfind("<w:p>")
        .or_else(|| xml[..name_idx].rfind("<w:p "))
        .expect("name run must sit inside a <w:p> paragraph");
    let para_head = &xml[para_start..name_idx];
    assert!(
        para_head.contains(r#"w:after="180""#),
        "the name paragraph must declare `w:after=\"180\"` (9pt) spacing \
         before the run reaches the contact line; paragraph head: {para_head}"
    );
}

/// A project's `·`-separated tech-stack line must reach DOCX as the entry
/// SUBTITLE — the italic run `RunOpts::subtitle` styles — and not as an ordinary
/// body paragraph. This is the structural guard for the adapter regrouping
/// (`model::adapter::absorb_project_line`) surviving all the way to the second
/// export format: the PDF matrix test can only see that the WORDS rendered,
/// while run properties here can tell a styled meta line from flat prose.
const PROJECTS_RESUME: &str = "\
Jane Doe
jane@example.com

PROJECTS

**Ledger CLI** · https://github.com/janedoe/ledger
Rust · SQLite · Clap
A double-entry bookkeeping tool for the terminal.
";

#[test]
fn project_tech_stack_lands_in_the_italic_subtitle_run() {
    let template = Template::get(TemplateId::Classic);
    let docx =
        generate_resume_docx(PROJECTS_RESUME, None, &template, false).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    let xml = part(&buffer.into_inner(), "word/document.xml");

    let idx = xml
        .find("SQLite")
        .expect("the tech-stack text must appear in document.xml");
    let run_start = xml[..idx].rfind("<w:r>").expect("enclosing run start");
    let run_end = idx + xml[idx..].find("</w:r>").expect("enclosing run end");
    let run_xml = &xml[run_start..run_end];
    assert!(
        run_xml.contains("<w:i "),
        "the tech-stack line must render as the italic subtitle run, not flat \
         body prose; run xml: {run_xml:?}"
    );

    // The project NAME stays bold, and the description is still there.
    let name_idx = xml
        .find("Ledger CLI")
        .expect("project name in document.xml");
    let name_start = xml[..name_idx].rfind("<w:r>").expect("name run start");
    let name_end = name_idx + xml[name_idx..].find("</w:r>").expect("name run end");
    assert!(
        xml[name_start..name_end].contains("<w:b "),
        "the project name must stay bold; run xml: {:?}",
        &xml[name_start..name_end]
    );
    assert!(xml.contains("double-entry bookkeeping"));
}
