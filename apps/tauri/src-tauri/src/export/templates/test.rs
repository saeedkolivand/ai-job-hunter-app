use super::*;
use super::super::types::LineKind;

#[test]
fn test_template_get_classic() {
    let template = Template::get(TemplateId::Classic);
    assert_eq!(template.id, TemplateId::Classic);
    assert_eq!(template.name, "ATS Classic");
}

#[test]
fn test_template_get_modern() {
    let template = Template::get(TemplateId::Modern);
    assert_eq!(template.id, TemplateId::Modern);
    assert_eq!(template.name, "Modern Technical");
}

#[test]
fn test_template_get_executive() {
    let template = Template::get(TemplateId::Executive);
    assert_eq!(template.id, TemplateId::Executive);
    assert_eq!(template.name, "Executive");
}

#[test]
fn test_template_classic_colors() {
    let template = Template::classic();
    assert_eq!(template.name_color, (17, 17, 17));
    assert_eq!(template.section_color, (17, 17, 17));
}

#[test]
fn test_template_modern_colors() {
    let template = Template::modern();
    assert_eq!(template.name_color, (13, 31, 60));
    assert_eq!(template.section_color, (13, 31, 60));
}

#[test]
fn test_template_executive_centered() {
    let template = Template::executive();
    assert!(template.name_centered);
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
