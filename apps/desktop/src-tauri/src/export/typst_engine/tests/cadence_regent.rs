//! PR3 styling-knob defaults + Cadence + Regent render tests.

use super::fixtures::{opts_sc, template_style};
use super::pdf_introspect::assert_no_stray_tokens;
use super::resume_fixtures::FIXTURE_RESUME;
use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf, render_resume_svg_pages, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

// (backward-compat proof)
//
// Every pre-PR3 template ships heading_tracking: 0.0, link_underline: false, and
// (for the ones whose rule is actually drawn) rule_thickness: 0.5 — the house
// default. By construction:
//   - `heading-run(...)` only emits `tracking: …` when `heading-tracking != 0.0`;
//     at 0.0 it falls through to the exact `text(size:, weight:, fill:, font:,
//     content)` call that existed before the knob (byte-for-byte, verified by
//     reading the branch).
//   - `render-runs(...)` only wraps a link in `underline(…)` when `link-underline`
//     is true; at `false` the branch reduces to the bare `styled` value — the
//     same `link(r.link, text(fill: c-accent, t))` call as before.
//   - the rule stroke resolves `(rule-thickness * 1pt) + c-rule`, and every
//     pre-PR3 ruled template ships `rule_thickness: 0.5` — `0.5 * 1pt == 0.5pt`,
//     the same literal stroke as before. (SwissMinimal ships `0.0`, but its
//     `section_style` is `BoldOnly`, so the ruled-bottom `line(...)` call — the
//     only reader of `rule-thickness` — never executes for it either way.)
//
// Rather than re-deriving that proof at test time by rendering the same code
// path twice and diffing the bytes (tautological — it would always pass), this
// renders ONCE per template and checks two INDEPENDENT anchors decoded from the
// compiled PDF: (a) no stray Typst source token leaked into the extracted text
// (`assert_no_stray_tokens`, which also now guards the new `tracking:` /
// `smallcaps(` syntax), and (b) the known section headings still appear intact,
// in the expected reading order.

#[test]
fn pr3_knob_defaults_leave_pre_pr3_headings_and_content_intact() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    for id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
    ] {
        let t = Template::get(id);
        assert_eq!(
            t.heading_tracking, 0.0,
            "{id:?}: heading_tracking must be 0.0"
        );
        assert!(!t.link_underline, "{id:?}: link_underline must be false");

        let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
            .unwrap_or_else(|e| panic!("{id:?}: render failed: {e:?}"));
        assert_no_stray_tokens(&format!("{id:?}-knob-defaults"), &bytes);

        let extracted = pdf_extract::extract_text_from_mem(&bytes)
            .unwrap_or_else(|e| panic!("{id:?}: pdf-extract failed: {e}"));
        let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = normalised.to_lowercase();

        assert!(
            lower.contains("jane doe"),
            "{id:?}: candidate name missing after knob threading\n---\n{lower}"
        );
        let order = ["summary", "experience", "education", "skills"];
        let mut last = 0usize;
        for h in &order {
            let pos = lower.find(h).unwrap_or_else(|| {
                panic!("{id:?}: heading '{h}' missing after knob threading\n---\n{lower}")
            });
            assert!(
                pos >= last,
                "{id:?}: '{h}' ({pos}) appeared before previous heading ({last})\n---\n{lower}"
            );
            last = pos;
        }
    }
}

#[test]
fn cadence_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Cadence);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(cadence) should succeed");
    assert!(!bytes.is_empty(), "Cadence PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Cadence output must start with %PDF"
    );
}

#[test]
fn cadence_accent_override_applies() {
    let base = Template::get(TemplateId::Cadence);
    let overridden = Template::get(TemplateId::Cadence).with_accent_override(Some("#00AA33"));
    assert_ne!(base.accent_color, overridden.accent_color);
    assert_eq!(overridden.accent_color, (0, 170, 51));
    assert_eq!(overridden.emphasis_color, (0, 170, 51));
}

#[test]
fn cadence_tracking_and_underline_change_the_rendered_svg() {
    // Cadence sets heading_tracking 0.08 and link_underline true — prove the
    // knobs actually perturb the rendered SVG (not just config plumbing) by
    // diffing against a neutral (0.0 / false) variant of the same template.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let cadence = Template::get(TemplateId::Cadence);
    assert_eq!(cadence.heading_tracking, 0.08);
    assert!(cadence.link_underline);

    let neutral = Template {
        heading_tracking: 0.0,
        link_underline: false,
        ..Template::get(TemplateId::Cadence)
    };

    let with_knobs = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&cadence),
    )
    .expect("cadence render should succeed");
    let without_knobs = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&neutral),
    )
    .expect("cadence-neutral render should succeed");

    assert_ne!(
        with_knobs, without_knobs,
        "heading_tracking/link_underline must visibly change the rendered SVG"
    );
}

#[test]
fn cadence_rule_thickness_changes_the_rendered_stroke() {
    // Cadence specs a 0.75pt section rule (vs the house 0.5pt default) — prove
    // `rule_thickness` is actually threaded into the rendered stroke width, not
    // just pinned config (the finding this test exists to close: the field was
    // previously dead in every renderer). Isolate it from the other PR3 knobs by
    // holding heading_tracking/link_underline fixed and varying only the stroke.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let cadence = Template::get(TemplateId::Cadence);
    assert_eq!(cadence.rule_thickness, 0.75);

    let half_pt_rule = Template {
        rule_thickness: 0.5,
        ..Template::get(TemplateId::Cadence)
    };

    let with_075 = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&cadence),
    )
    .expect("cadence (0.75pt rule) render should succeed");
    let with_05 = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&half_pt_rule),
    )
    .expect("cadence (0.5pt rule) render should succeed");

    assert_ne!(
        with_075, with_05,
        "rule_thickness must visibly change the rendered SVG stroke"
    );
}

#[test]
fn regent_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Regent);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(regent) should succeed");
    assert!(!bytes.is_empty(), "Regent PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Regent output must start with %PDF"
    );
}

#[test]
fn regent_accent_override_applies() {
    let base = Template::get(TemplateId::Regent);
    let overridden = Template::get(TemplateId::Regent).with_accent_override(Some("#123456"));
    assert_ne!(base.accent_color, overridden.accent_color);
    assert_eq!(overridden.accent_color, (18, 52, 86));
    assert_eq!(overridden.emphasis_color, (18, 52, 86));
}

#[test]
fn regent_maps_to_serif_small_caps_burgundy_style() {
    use super::super::render::style_from_template;

    let regent = Template::get(TemplateId::Regent);
    let style = style_from_template(&regent);

    // Source Serif 4 throughout, burgundy accent, small-caps (not all-caps)
    // headings, light heading tracking, no link underline.
    assert_eq!(style.font_heading, "Source Serif 4");
    assert_eq!(style.font_name, "Source Serif 4");
    assert_eq!(style.font_body, "Source Serif 4");
    assert!(
        style.section_small_caps,
        "Regent headings must be small-caps"
    );
    assert!(
        !style.section_all_caps,
        "Regent headings must not be all-caps"
    );
    assert_eq!(style.c_accent, "#6E1E2B");
    assert!((style.heading_tracking - 0.04).abs() < f32::EPSILON);
    assert!(!style.link_underline);
    assert_eq!(style.rule_thickness, 0.5);

    // Exercise the actual render path too (not just the JsonStyle mapping) and
    // guard the new `smallcaps(…)` call site — the bundled Source Serif 4 TTF
    // has neither `smcp` nor `c2sc`, and Typst 0.15's `smallcaps` does not yet
    // synthesize small caps for fonts lacking those features, so this renders
    // headings at 0.85× size in their original case rather than visually
    // distinct small-caps glyphs today; `smallcaps(…)` still keeps the PDF text
    // layer's characters unmodified (extraction-safe) and is forward-compatible
    // with a future smcp-capable font swap.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&regent),
    )
    .expect("regent render_pdf should succeed");
    assert!(bytes.starts_with(b"%PDF"));
    assert_no_stray_tokens("regent-small-caps", &bytes);
}
