use super::*;

// ── apply_to_header — name fallback ──────────────────────────────────────────

/// When `header.name` is blank, `apply_to_header` fills it from `full_name` so
/// a profile-edited name is not silently dropped during export without generation
/// metadata (the "H6 — full_name never rendered" regression).
#[test]
fn apply_to_header_fills_blank_name_from_full_name() {
    use crate::export::types::DocumentType;
    use crate::model::document::DocumentModel;

    let profile = ContactProfile {
        full_name: Some("Jordan Lee".into()),
        email: Some("jordan@example.com".into()),
        ..Default::default()
    };

    let mut model = DocumentModel::new(DocumentType::Resume);
    // Simulate a header that arrived with no name (blank).
    model.header.name = String::new();

    profile.apply_to_header(&mut model.header, "en");

    assert_eq!(
        model.header.name, "Jordan Lee",
        "blank header.name must be filled from profile.full_name"
    );
    // Contact line is also set.
    assert!(
        !model.header.contact.is_empty(),
        "contact rich text must be set from profile"
    );
}

/// When `header.name` is already set, `apply_to_header` must not overwrite it —
/// the generation metadata name takes precedence over the profile name.
#[test]
fn apply_to_header_does_not_overwrite_existing_name() {
    use crate::export::types::DocumentType;
    use crate::model::document::DocumentModel;

    let profile = ContactProfile {
        full_name: Some("Jordan Lee".into()),
        email: Some("jordan@example.com".into()),
        ..Default::default()
    };

    let mut model = DocumentModel::new(DocumentType::Resume);
    model.header.name = "Alex Carter".to_string();

    profile.apply_to_header(&mut model.header, "en");

    assert_eq!(
        model.header.name, "Alex Carter",
        "an already-populated header.name must never be overwritten"
    );
}

// ── apply_to_header — contact-line fallback (editor-is-source-of-truth) ──────

/// When the text already parses a contact line, `apply_to_header` must leave it
/// alone — the editor's text is the source of truth for what exports, and the
/// profile is only a fallback for a document that has none.
#[test]
fn apply_to_header_keeps_text_derived_contact_line() {
    use crate::model::adapter::model_from_resume_text;

    let text = "Jordan Lee\nBerlin, Germany | jordan@editor.example.com | +49 30 0000000\n\nSUMMARY\nSome text.";
    let mut model = model_from_resume_text(text);
    assert!(
        !model.header.contact.is_empty(),
        "test setup: resume text must parse a contact line"
    );
    let before = model.header.contact.clone();

    let profile = ContactProfile {
        full_name: Some("Jordan Lee".into()),
        email: Some("jordan@profile.example.com".into()),
        phone: Some("+1 555 0100".into()),
        ..Default::default()
    };
    profile.apply_to_header(&mut model.header, "en");

    assert_eq!(
        model.header.contact, before,
        "a text-derived contact line must never be overwritten by the profile"
    );
}

/// When the text has no contact line, `apply_to_header` fills it from the
/// profile — the fallback case these overrides exist for.
#[test]
fn apply_to_header_fills_contact_from_profile_when_text_has_none() {
    use crate::model::adapter::model_from_resume_text;

    let text = "Jordan Lee\n\nSUMMARY\nSome text with no contact line at all.";
    let mut model = model_from_resume_text(text);
    assert!(
        model.header.contact.is_empty(),
        "test setup: resume text must parse with no contact line"
    );

    let profile = ContactProfile {
        full_name: Some("Jordan Lee".into()),
        email: Some("jordan@profile.example.com".into()),
        ..Default::default()
    };
    profile.apply_to_header(&mut model.header, "en");

    assert!(
        !model.header.contact.is_empty(),
        "the profile must fill a header that has no contact line"
    );
    assert!(model
        .header
        .contact
        .iter()
        .any(|r| r.link.as_deref() == Some("mailto:jordan@profile.example.com")));
}
