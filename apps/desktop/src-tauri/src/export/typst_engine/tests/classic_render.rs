//! Classic template render tests: next-line-date entries, letter-page geometry, accent overrides.

use super::fixtures::opts_a4;
use super::pdf_introspect::embedded_font_base_names;
use super::resume_fixtures::FIXTURE_RESUME;
use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf, RenderOpts, TypstTemplate};
use crate::locale::PageGeometry;
use crate::model::adapter::model_from_resume_text;

#[test]
fn classic_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed");

    assert!(!bytes.is_empty(), "PDF bytes must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "output must start with %PDF header"
    );
}

/// Owner-reported résumé shape: "Title · Company" on its own line with NO
/// same-line date, the date range + location on the line right after —
/// Experience (ongoing "Present" role + a closed role), Education, and a
/// pipe-dated Certifications entry for good measure. End-to-end proof this
/// now renders as structured, bold entries with a genuinely italic date —
/// not two unrelated plain paragraphs, and not a synthetic slant with no
/// embedded italic face.
const NEXT_LINE_DATE_FIXTURE: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe

EXPERIENCE
Senior Frontend Developer \u{b7} ACTINEO GmbH
December 2022 \u{2013} November 2025, K\u{f6}ln, Deutschland
- Built scalable, multilingual enterprise applications

Independent / Open-Source R&D \u{b7} Self-directed
December 2025 \u{2013} Present, K\u{f6}ln, Deutschland
- Designed and shipped a local-first desktop app

EDUCATION
B.Sc. Computer Software Technology \u{b7} Islamic Azad University
January 2019 \u{2013} October 2021
";

#[test]
fn classic_next_line_date_entry_is_structured_bold_title_italic_date() {
    let model = model_from_resume_text(NEXT_LINE_DATE_FIXTURE);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed for the next-line-date fixture");

    assert!(bytes.starts_with(b"%PDF"));

    // Nothing was lost: every distinct piece of content — role, company,
    // BOTH date ranges, location, and the bullets — must round-trip through
    // extracted text (this is what silently regressed before the fix: the
    // whole entry fell through to plain, undated paragraphs).
    let extracted =
        pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract should read our output");
    for needle in [
        "Senior Frontend Developer",
        "ACTINEO GmbH",
        "December 2022",
        "November 2025",
        "Independent / Open-Source R&D",
        "Self-directed",
        "December 2025",
        "Present",
        "K\u{f6}ln, Deutschland",
        "Built scalable, multilingual enterprise applications",
        "B.Sc. Computer Software Technology",
        "Islamic Azad University",
        "January 2019",
        "October 2021",
    ] {
        assert!(
            extracted.contains(needle),
            "expected {needle:?} in extracted text; got: {extracted:?}"
        );
    }
}

/// The embedded-italic proof, on a fixture that can ONLY get italic from the
/// date.
///
/// `NEXT_LINE_DATE_FIXTURE` also produces italic SUBTITLE runs from its location
/// remainders, and `single_column.typ` renders subtitles italic — so asserting
/// on that fixture would still pass if the entry date silently reverted to
/// non-italic. This fixture has a date and no subtitle, so the assertion can
/// only be satisfied by the run under test.
#[test]
fn classic_entry_date_embeds_a_real_italic_face() {
    const DATE_ONLY_FIXTURE: &str = "Saeed Kolivand

EXPERIENCE

Senior Frontend Developer
Dec 2022 – Nov 2025
- Built scalable, multilingual enterprise applications
";
    let model = model_from_resume_text(DATE_ONLY_FIXTURE);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed");

    // A real italic face gets its own embedded `/BaseFont`; a synthetic slant of
    // Regular would not, which is what makes this proof rather than inference.
    let fonts = embedded_font_base_names(&bytes);
    assert!(
        fonts.iter().any(|f| f.to_lowercase().contains("italic")),
        "the entry date must render a genuinely embedded italic face; embedded fonts: {fonts:?}"
    );
}

/// Write a sample PDF for human/agent review — same convention as the other
/// `*_write_sample_pdf_for_review` tests (always passes; informational).
#[test]
fn classic_next_line_date_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(NEXT_LINE_DATE_FIXTURE);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!(
            "classic_next_line_date_write_sample_pdf_for_review: could not create target/: {e}"
        );
    }
    // `target` is derived from CARGO_MANIFEST_DIR and is therefore ABSOLUTE.
    // Path privacy (AGENTS.md) forbids emitting an absolute path anywhere — logs
    // included — so only the repo-relative artifact name is ever printed.
    const SAMPLE_REL: &str = "apps/desktop/src-tauri/target/classic_next_line_date_sample.pdf";
    let out_path = target.join("classic_next_line_date_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Sample PDF written to: {SAMPLE_REL}"),
        Err(e) => eprintln!(
            "classic_next_line_date_write_sample_pdf_for_review: could not write {SAMPLE_REL}: {e} (informational only)"
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn classic_render_letter_page_succeeds() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 215.9,
            height_mm: 279.4,
        },
        lang: "en".to_string(),
        accent: None,
        ats: false,
    };
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf(classic, Letter) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn classic_render_with_valid_accent_succeeds() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        accent: Some("#1a2b3c".to_string()),
        ..opts_a4()
    };
    // Classic now renders through the parametric SingleColumn template, which
    // honors the accent override (data.opts.accent) — it must not crash.
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf should succeed with a valid accent override");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn classic_render_with_invalid_accent_falls_back_gracefully() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        accent: Some("not-a-color".to_string()),
        ..opts_a4()
    };
    // Invalid accent must not cause an error (normalise_accent returns "" →
    // template defaults apply).
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf should succeed with an invalid accent color");
    assert!(bytes.starts_with(b"%PDF"));
}
