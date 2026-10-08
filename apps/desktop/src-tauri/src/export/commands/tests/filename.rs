use super::*;

/// #1369: a job-less export (the side panel's base-résumé attach) must not invent
/// "Role"/"Company" placeholders.
#[test]
fn generate_filename_drops_the_role_and_company_placeholders_when_absent() {
    let request = ExportRequest {
        meta: Some(GenerationMeta {
            candidate_name: Some("Jane Doe".to_string()),
            job_title: None,
            company_name: Some("  ".to_string()),
            target_language: None,
        }),
        ..default_request()
    };
    assert_eq!(generate_filename(&request, "pdf"), "Jane-Doe-resume.pdf");
}

/// Shared with the renderer's `buildFilename` test (`export.test.ts`): same inputs, same
/// expected stems, so the save dialog and the card chip can never disagree (#1398).
/// An empty expectation means the "Candidate" fallback.
const FILENAME_VECTORS: &[(&str, &str)] = &[
    (
        "Android Engineer — Experience",
        "Android-Engineer-Experience",
    ),
    ("José Müller", "José-Müller"),
    ("snake_case name", "snake_case-name"),
    ("Jane\tDoe\u{a0}Smith", "Jane-Doe-Smith"),
    ("  ...Jane Doe!!  ", "Jane-Doe"),
    ("-a--b-", "a-b"),
    ("★★★", ""),
    ("Cafe\u{301}", "Cafe"),
    ("𝒜 𝔹", "𝒜-𝔹"),
    ("x²½", "x²½"),
    (
        "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx y",
        "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
    ),
];

#[test]
fn filename_matches_the_shared_vector_table() {
    for (input, stem) in FILENAME_VECTORS {
        let request = ExportRequest {
            meta: Some(GenerationMeta {
                candidate_name: Some((*input).to_string()),
                job_title: None,
                company_name: None,
                target_language: None,
            }),
            ..default_request()
        };
        let stem = if stem.is_empty() { "Candidate" } else { stem };
        assert_eq!(
            generate_filename(&request, "pdf"),
            format!("{stem}-resume.pdf"),
            "input {input:?}"
        );
    }
}

#[test]
fn test_generate_filename() {
    let request = ExportRequest {
        format: ExportFormat::Docx,
        template_id: TemplateId::SwissMinimal,
        meta: Some(GenerationMeta {
            candidate_name: Some("John Doe".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Tech Corp".to_string()),
            target_language: None,
        }),
        ..default_request()
    };

    let filename = generate_filename(&request, "docx");
    assert!(filename.contains("John-Doe"));
    assert!(filename.contains("Software-Engineer"));
    assert!(filename.contains("resume"));
    assert!(filename.ends_with(".docx"));
}

/// A1 (hardening plan) — end-to-end, anchored to the RENDERED filename a user
/// actually sees/saves/shares, not to `is_implausible_company`'s own bool: a
/// scraper-supplied garbage company (the literal PR #960 report) must fall
/// back to the same `"Company"` default an absent one already produces,
/// never appear verbatim in the exported file's name.
#[test]
fn generate_filename_falls_back_to_company_default_for_an_implausible_name() {
    let request = ExportRequest {
        format: ExportFormat::Docx,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: Some(GenerationMeta {
            candidate_name: Some("John Doe".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Apply now | LinkedIn".to_string()),
            target_language: None,
        }),
        ..default_request()
    };

    let filename = generate_filename(&request, "pdf");
    assert!(
        filename.contains("Company"),
        "expected the absent-company fallback, got {filename:?}"
    );
    assert!(
        !filename.contains("LinkedIn") && !filename.contains("Apply"),
        "the garbage company must never reach the rendered filename, got {filename:?}"
    );
}

/// `meta.candidate_name: Some("")` (the shape TailorFlow actually sends) must
/// fall through to the attached `ContactProfile`'s name rather than winning
/// as an empty string — before the non-blank filter, `Some("")` stayed
/// `Some` and never reached this fallback rung at all.
#[test]
fn generate_filename_falls_back_to_contact_profile_when_meta_name_is_blank() {
    use crate::contact_profile::ContactProfile;

    let request = ExportRequest {
        document_type: DocumentType::CoverLetter,
        meta: Some(GenerationMeta {
            candidate_name: Some(String::new()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        contact: Some(ContactProfile {
            full_name: Some("Jane Smith".to_string()),
            ..Default::default()
        }),
        ..default_request()
    };

    let filename = generate_filename(&request, "pdf");
    assert!(
        filename.starts_with("Jane-Smith-"),
        "must fall back to the contact profile's name, not \"Candidate\": {filename}"
    );
}

/// With no name anywhere (no meta, no contact), the filename still degrades
/// to "Candidate" — the fallback rung is additive, not a replacement.
#[test]
fn generate_filename_falls_back_to_candidate_with_no_name_anywhere() {
    let request = ExportRequest {
        document_type: DocumentType::CoverLetter,
        ..default_request()
    };

    let filename = generate_filename(&request, "pdf");
    assert!(filename.starts_with("Candidate-"), "got: {filename}");
}
