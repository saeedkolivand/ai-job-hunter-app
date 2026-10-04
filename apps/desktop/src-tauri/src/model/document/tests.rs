use super::*;

#[test]
fn from_header_classifies_common_sections() {
    assert_eq!(
        SectionId::from_header("Professional Summary"),
        SectionId::Summary
    );
    assert_eq!(
        SectionId::from_header("WORK EXPERIENCE"),
        SectionId::Experience
    );
    assert_eq!(SectionId::from_header("Education"), SectionId::Education);
    assert_eq!(
        SectionId::from_header("Technical Skills"),
        SectionId::Skills
    );
    assert_eq!(
        SectionId::from_header("Certifications"),
        SectionId::Certifications
    );
    assert_eq!(SectionId::from_header("Languages"), SectionId::Languages);
}

#[test]
fn from_header_preserves_unknown_headings_as_custom() {
    assert_eq!(
        SectionId::from_header("  Speaking Engagements  "),
        SectionId::Custom("Speaking Engagements".to_string())
    );
}

#[test]
fn new_stamps_current_schema_version() {
    let m = DocumentModel::new(DocumentType::Resume);
    assert_eq!(m.schema_version, SCHEMA_VERSION);
    assert!(m.sections.is_empty());
    assert_eq!(m.header, HeaderBlock::default());
}
