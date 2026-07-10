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
