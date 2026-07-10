use super::super::types::LineKind;
use super::*;

#[test]
fn test_template_get_classic() {
    let template = Template::get(TemplateId::Classic);
    assert_eq!(template.id, TemplateId::Classic);
    assert_eq!(template.name, "ATS Classic");
}

#[test]
fn test_template_classic_colors() {
    let template = Template::classic();
    assert_eq!(template.name_color, (17, 17, 17));
    assert_eq!(template.section_color, (17, 17, 17));
}

#[test]
fn test_template_classic_not_centered() {
    let template = Template::classic();
    assert!(!template.name_centered);
}

#[test]
fn test_calculate_spacing_section_header() {
    let spacing = calculate_spacing(&LineKind::SectionHeader, None);
    assert_eq!(spacing, (12.0, 3.0));
}

#[test]
fn test_calculate_spacing_job_entry_default() {
    let spacing = calculate_spacing(&LineKind::JobEntry, None);
    assert_eq!(spacing, (6.0, 1.0));
}

#[test]
fn test_calculate_spacing_job_entry_after_bullet() {
    let spacing = calculate_spacing(&LineKind::JobEntry, Some(&LineKind::Bullet));
    assert_eq!(spacing, (8.0, 1.0));
}

#[test]
fn test_calculate_spacing_job_title() {
    let spacing = calculate_spacing(&LineKind::JobTitle, None);
    assert_eq!(spacing, (0.0, 3.0));
}

#[test]
fn test_calculate_spacing_bullet_default() {
    let spacing = calculate_spacing(&LineKind::Bullet, None);
    assert_eq!(spacing, (3.0, 2.0));
}

#[test]
fn test_calculate_spacing_bullet_after_bullet() {
    let spacing = calculate_spacing(&LineKind::Bullet, Some(&LineKind::Bullet));
    assert_eq!(spacing, (0.0, 2.0));
}

#[test]
fn test_calculate_spacing_contact() {
    let spacing = calculate_spacing(&LineKind::Contact, None);
    assert_eq!(spacing, (0.0, 0.0));
}

#[test]
fn test_calculate_spacing_name() {
    let spacing = calculate_spacing(&LineKind::Name, None);
    assert_eq!(spacing, (0.0, 2.0));
}

#[test]
fn test_calculate_spacing_text_default() {
    let spacing = calculate_spacing(&LineKind::Text, None);
    assert_eq!(spacing, (0.0, 4.0));
}

#[test]
fn test_section_style_partial_eq() {
    assert_eq!(SectionStyle::RuledBottom, SectionStyle::RuledBottom);
    assert_ne!(SectionStyle::RuledBottom, SectionStyle::Underline);
}

// ─── Document accent override ─────────────────────────────────────────────────

#[test]
fn with_accent_override_recolors_accent_and_emphasis_for_valid_hex() {
    // Baseline: Classic ships a near-black accent + pure-black emphasis.
    let base = Template::get(TemplateId::Classic);
    assert_ne!(base.accent_color, (170, 0, 0));

    // A valid override recolors both accent-derived fields (bare + #-prefixed).
    let bare = Template::get(TemplateId::Classic).with_accent_override(Some("AA0000"));
    assert_eq!(bare.accent_color, (170, 0, 0));
    assert_eq!(bare.emphasis_color, (170, 0, 0));

    let hashed = Template::get(TemplateId::Classic).with_accent_override(Some("#1A2B3C"));
    assert_eq!(hashed.accent_color, (26, 43, 60));
    assert_eq!(hashed.emphasis_color, (26, 43, 60));
}

#[test]
fn with_accent_override_is_a_noop_for_absent_or_malformed_input() {
    let base = Template::get(TemplateId::Classic);
    for bad in [
        None,
        Some(""),
        Some("not-a-color"),
        Some("#12345"),
        Some("#1234567"),
    ] {
        let t = Template::get(TemplateId::Classic).with_accent_override(bad);
        assert_eq!(
            (t.accent_color, t.emphasis_color),
            (base.accent_color, base.emphasis_color),
            "accent {bad:?} must leave the palette untouched"
        );
    }
}

// ─── Template tier metadata ───────────────────────────────────────────────────

/// Pin every template's ATS/Design tier. ATS-safe = single-column, parser-safe;
/// Design = photo / two-column layouts (which surface the ATS-mode toggle and
/// drop the photo when it's on). Update deliberately when a template is added.
#[test]
fn template_tiers_are_pinned() {
    use TemplateTier::{Ats, Design};
    let expected = [
        (TemplateId::Classic, Ats),
        (TemplateId::SwissMinimal, Ats),
        (TemplateId::Academic, Ats),
        (TemplateId::Meridian, Ats),
        (TemplateId::Throughline, Ats),
        (TemplateId::Atelier, Design),
        (TemplateId::Portrait, Design),
        (TemplateId::Lebenslauf, Design),
        (TemplateId::Cadence, Ats),
        (TemplateId::Regent, Ats),
    ];
    for (id, tier) in expected {
        assert_eq!(
            Template::get(id).tier,
            tier,
            "template {id:?} has the wrong tier"
        );
    }
}

// ─── PR3: heading_tracking / link_underline knobs ──────────────────────────────

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
