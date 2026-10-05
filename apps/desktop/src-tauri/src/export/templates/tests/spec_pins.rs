//! Per-template registry field pins — one test per template family that
//! carries distinguishing spec values, plus the cross-cutting PR3 knob
//! defaults and the Deedy section-rhythm / canonical-id guards.

use crate::export::templates::{
    ParagraphIndent, SectionStyle, Template, TemplateTier, CANONICAL_TEMPLATE_IDS,
};
use crate::export::types::TemplateId;

/// Every pre-PR3 template must keep the two new knobs at their neutral default
/// (0.0 / false) — `single_column.typ` only emits `tracking:`/`underline(…)` when
/// non-zero/true, so this is what keeps existing output byte-identical.
#[test]
fn heading_tracking_and_link_underline_default_to_neutral_for_pre_pr3_templates() {
    for id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
        TemplateId::Atelier,
        TemplateId::Meridian,
        TemplateId::Throughline,
        TemplateId::Portrait,
        TemplateId::Lebenslauf,
    ] {
        let t = Template::get(id);
        assert_eq!(
            t.heading_tracking, 0.0,
            "{id:?}: heading_tracking must default to 0.0"
        );
        assert!(
            !t.link_underline,
            "{id:?}: link_underline must default to false"
        );
    }
}

// ─── Cadence / Regent spec pins ─────────────────────────────────────────────────

#[test]
fn cadence_matches_spec() {
    let t = Template::get(TemplateId::Cadence);
    assert_eq!(t.tier, TemplateTier::Ats);
    assert_eq!(t.name_pt, 28.0);
    assert_eq!(t.section_pt, 10.5);
    assert_eq!(t.body_pt, 10.0);
    assert_eq!(t.margin_in, 0.8);
    assert_eq!(t.line_spacing, 1.15);
    assert_eq!(t.section_spacing_before, 12.0);
    assert_eq!(t.accent_color, (74, 103, 133));
    assert!(t.section_all_caps);
    assert_eq!(t.section_style, SectionStyle::RuledBottom);
    assert_eq!(t.rule_thickness, 0.75);
    assert!(!t.job_title_italic);
    assert!(!t.section_small_caps);
    assert_eq!(t.heading_tracking, 0.08);
    assert!(t.link_underline);
    assert!(t.two_column.is_none());
    assert_eq!(
        t.cover_letter.paragraph_indent,
        ParagraphIndent::BlockNoIndent
    );
    assert_eq!(t.cover_letter.paragraph_spacing_pt, 8.0);
}

#[test]
fn regent_matches_spec() {
    let t = Template::get(TemplateId::Regent);
    assert_eq!(t.tier, TemplateTier::Ats);
    assert_eq!(t.name_pt, 26.0);
    assert_eq!(t.section_pt, 11.0);
    assert_eq!(t.body_pt, 10.5);
    assert_eq!(t.margin_in, 0.9);
    assert_eq!(t.line_spacing, 1.2);
    assert_eq!(t.section_spacing_before, 14.0);
    assert_eq!(t.accent_color, (110, 30, 43));
    assert_eq!(t.rule_color, (201, 169, 174));
    assert!(!t.section_all_caps);
    assert!(t.section_small_caps);
    assert_eq!(t.section_style, SectionStyle::RuledBottom);
    assert_eq!(t.rule_thickness, 0.5);
    assert!(t.job_title_italic);
    assert_eq!(t.heading_tracking, 0.04);
    assert!(!t.link_underline);
    assert!(t.two_column.is_none());
    assert_eq!(t.cover_letter.paragraph_indent, ParagraphIndent::FirstLine);
    assert_eq!(t.cover_letter.paragraph_spacing_pt, 0.0);
}

// ─── PR4: Aria / Saffron spec pins ──────────────────────────────────────────────

#[test]
fn aria_matches_spec() {
    let t = Template::get(TemplateId::Aria);
    assert_eq!(t.tier, TemplateTier::Design);
    assert_eq!(t.name_pt, 30.0);
    assert_eq!(t.section_pt, 10.5);
    assert_eq!(t.body_pt, 10.0);
    assert_eq!(t.margin_in, 0.6);
    assert_eq!(t.line_spacing, 1.25);
    assert_eq!(t.section_spacing_before, 16.0);
    assert_eq!(t.name_color, (17, 17, 17));
    assert_eq!(t.section_color, (26, 26, 26));
    assert_eq!(t.accent_color, (70, 80, 92));
    assert_eq!(t.body_color, (42, 42, 42));
    assert_eq!(t.date_color, (122, 122, 122));
    assert_eq!(t.emphasis_color, (70, 80, 92));
    assert_eq!(t.rule_color, (214, 217, 221));
    assert!(t.section_all_caps);
    assert!(!t.section_small_caps);
    assert!(!t.job_title_italic);
    assert_eq!(t.heading_tracking, 0.06);
    assert!(!t.link_underline);
    let tc = t.two_column.as_ref().expect("Aria is two-column");
    assert_eq!(tc.sidebar_width_ratio, 0.32);
    assert_eq!(tc.sidebar_bg_color, (255, 255, 255));
    assert_eq!(
        t.cover_letter.paragraph_indent,
        ParagraphIndent::BlockNoIndent
    );
    assert_eq!(t.cover_letter.paragraph_spacing_pt, 8.0);
}

#[test]
fn saffron_matches_spec() {
    let t = Template::get(TemplateId::Saffron);
    assert_eq!(t.tier, TemplateTier::Design);
    assert_eq!(t.name_pt, 24.0);
    assert_eq!(t.section_pt, 11.0);
    assert_eq!(t.body_pt, 10.5);
    assert_eq!(t.margin_in, 0.55);
    assert_eq!(t.line_spacing, 1.2);
    assert_eq!(t.section_spacing_before, 12.0);
    assert_eq!(t.name_color, (58, 46, 40));
    assert_eq!(t.section_color, (168, 90, 62));
    assert_eq!(t.accent_color, (168, 90, 62));
    assert_eq!(t.body_color, (48, 42, 38));
    assert_eq!(t.date_color, (138, 122, 110));
    assert_eq!(t.emphasis_color, (168, 90, 62));
    assert_eq!(t.rule_color, (226, 201, 180));
    assert!(!t.section_all_caps);
    assert!(t.section_small_caps);
    assert!(t.job_title_italic);
    assert_eq!(t.heading_tracking, 0.0);
    assert!(!t.link_underline);
    let tc = t.two_column.as_ref().expect("Saffron is two-column");
    assert_eq!(tc.sidebar_width_ratio, 0.34);
    assert_eq!(tc.sidebar_bg_color, (245, 231, 218));
    assert_eq!(
        t.cover_letter.paragraph_indent,
        ParagraphIndent::BlockNoIndent
    );
    assert_eq!(t.cover_letter.paragraph_spacing_pt, 8.0);
}

// ─── Phase 8 Track B: Jake / Awesome / Deedy spec pins ─────────────────────────

/// Registry FIELDS only. `name_centered` in particular is a declaration, not
/// evidence: this test passed for the whole life of the bug where Jake's name
/// rendered flush left because `single_column.typ` centred inside an
/// auto-width block. The rendered geometry is pinned by
/// `typst_engine::tests::every_template::name_centered_actually_centres_the_rendered_header` —
/// don't read a green here as "the name is centred".
#[test]
fn jake_matches_spec() {
    let t = Template::get(TemplateId::Jake);
    assert_eq!(t.tier, TemplateTier::Ats);
    assert_eq!(t.name_pt, 24.0);
    assert_eq!(t.section_pt, 11.0);
    assert_eq!(t.body_pt, 10.0);
    assert_eq!(t.margin_in, 0.6);
    assert!(t.name_centered);
    assert!(t.section_all_caps);
    assert_eq!(t.section_style, SectionStyle::RuledBottom);
    assert_eq!(t.rule_thickness, 0.4);
    assert_eq!(t.heading_tracking, 0.0);
    assert!(!t.link_underline);
    assert!(!t.section_small_caps);
    assert!(t.two_column.is_none());
}

#[test]
fn awesome_matches_spec() {
    let t = Template::get(TemplateId::Awesome);
    assert_eq!(t.tier, TemplateTier::Design);
    // Registry name_color must stay a real dark ink for the DOCX renderer —
    // NOT the white the PDF band text renders in (awesome.typ hardcodes that
    // separately). A regression here would print invisible white-on-white
    // DOCX name text.
    //
    // Pinned by EQUALITY, like deedy/aria below. The former `assert_ne!(t
    // .name_color, (255, 255, 255))` only excluded pure white: (254, 254, 254)
    // — or any other near-white — sailed through it and still prints
    // unreadable DOCX name text on the pale `band_tint_hex` shading. "Not
    // exactly one bad value" is not the same claim as "a dark ink".
    assert_eq!(t.name_color, (26, 26, 26));
    assert_eq!(t.accent_color, (196, 30, 58));
    assert_eq!(t.emphasis_color, (196, 30, 58));
    assert!(t.section_all_caps);
    assert_eq!(t.section_style, SectionStyle::RuledBottom);
    assert_eq!(t.heading_tracking, 0.0);
    assert!(t.two_column.is_none());
}

#[test]
fn deedy_matches_spec() {
    let t = Template::get(TemplateId::Deedy);
    assert_eq!(t.tier, TemplateTier::Design);
    assert_eq!(t.name_pt, 27.0);
    assert_eq!(t.accent_color, (30, 79, 179));
    assert!(t.section_all_caps);
    assert!(!t.job_title_italic);
    assert_eq!(t.section_style, SectionStyle::RuledBottom);
    assert!(t.two_column.is_none());
    // Deedy's "generous section spacing" trait. It lives here, in the registry,
    // because `_scale.typ`'s rhythm is LOCKED — `deedy.typ` used to fork it with
    // a local `sp-section-extra = 8pt`, the only template doing so.
    assert_eq!(t.section_above_extra, 8.0);
}

/// The wider rhythm is Deedy's alone: every other template must keep the shared
/// `_scale.typ` `sp-section-above` untouched. Without this, adding the knob
/// would be one careless copy-paste away from silently re-spacing the roster.
#[test]
fn only_deedy_supplements_the_shared_section_rhythm() {
    for id in CANONICAL_TEMPLATE_IDS {
        let expected = if id == TemplateId::Deedy { 8.0 } else { 0.0 };
        assert_eq!(
            Template::get(id).section_above_extra,
            expected,
            "{id:?}: section_above_extra must be {expected} — 0.0 means \
             'use the locked house rhythm unchanged'"
        );
    }
}

/// [`crate::export::templates::CANONICAL_TEMPLATE_IDS`] is the list every test
/// matrix iterates, so a gap in it silently un-covers a template everywhere at
/// once. Pin that it holds each id exactly once and that `Template::get` really
/// returns that id (a copy-pasted constructor returning a neighbour's id would
/// otherwise make one template invisible to every matrix).
#[test]
fn canonical_template_ids_are_unique_and_self_describing() {
    let ids = CANONICAL_TEMPLATE_IDS;
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(
            Template::get(*id).id,
            *id,
            "Template::get({id:?}) returned a different template's id"
        );
        assert!(
            !ids[..i].contains(id),
            "{id:?} appears twice in CANONICAL_TEMPLATE_IDS"
        );
    }
}
