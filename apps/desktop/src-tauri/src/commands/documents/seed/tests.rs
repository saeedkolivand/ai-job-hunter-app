use super::*;
use crate::extraction::structured::{structure, Field};
use crate::extraction::types::SourceFormat;

fn extracted(text: &str) -> ExtractedResume {
    ExtractedResume {
        text: text.into(),
        links: vec![],
        confidence: Confidence::High,
        source_format: SourceFormat::PlainText,
        warnings: vec![],
    }
}

const RESUME: &str = "Jane Doe\njane@example.com | linkedin.com/in/jane-doe\n\nEXPERIENCE\nEngineer | Acme | 2020 – 2024\n- Built things\n\nSKILLS\nRust\n";

#[test]
fn a_confident_name_and_a_schemeless_linkedin_are_suggested() {
    let ex = extracted(RESUME);
    let s = suggest_contact(&ex, &structure(&ex));
    assert_eq!(s.full_name.as_deref(), Some("Jane Doe"));
    assert_eq!(
        s.linkedin.as_deref(),
        Some("https://linkedin.com/in/jane-doe")
    );
}

#[test]
fn a_low_confidence_name_is_not_suggested() {
    let ex = extracted(RESUME);
    let mut st = structure(&ex);
    st.name = Field {
        value: "Maybe Not".into(),
        confidence: Confidence::Low,
        source_span: None,
    };
    assert_eq!(suggest_contact(&ex, &st).full_name, None);
}

#[test]
fn seeding_fills_an_empty_name_never_overwrites_and_skips_when_nothing_to_seed() {
    let suggested = ContactProfile {
        full_name: Some("Jane Doe".into()),
        ..Default::default()
    };
    let seeded = seeded_profile(&ContactProfile::default(), &suggested).expect("name alone seeds");
    assert_eq!(seeded.full_name.as_deref(), Some("Jane Doe"));
    let kept = ContactProfile {
        full_name: Some("J. Doe".into()),
        ..Default::default()
    };
    assert_eq!(
        seeded_profile(&kept, &suggested)
            .unwrap()
            .full_name
            .as_deref(),
        Some("J. Doe")
    );
    assert!(seeded_profile(&kept, &ContactProfile::default()).is_none());
}
