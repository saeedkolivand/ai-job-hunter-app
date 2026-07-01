use super::super::types::{DocumentType, ExportFormat, TemplateId};
use super::*;

// ── Fixtures (mirrors typst_engine/test.rs — minimal but complete) ────────────

/// Short résumé fixture — exercises header + experience + skills blocks.
const FIXTURE_RESUME: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe

EXPERIENCE
Senior Engineer | Acme Corp | 2021 – Present
- Designed distributed task scheduler reducing latency by 40 percent

SKILLS
Rust, TypeScript, PostgreSQL
";

/// US English cover-letter fixture — exercises letterhead + body + sign-off.
const LETTER_FIXTURE_US: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp

Dear Hiring Manager,

I am writing to express my strong interest in the Software Engineer position \
at Acme Corp.

Sincerely,

Jane Smith
";

// ── documents_render_preview_images ──────────────────────────────────────────

/// Résumé request → at least one page, every page is an SVG document,
/// mime_type is "image/svg+xml".
#[tokio::test]
async fn preview_resume_returns_svg_pages() {
    let request = ExportRequest {
        text: FIXTURE_RESUME.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
    };
    let result = documents_render_preview_images(request)
        .await
        .expect("preview_resume should succeed");

    assert!(
        !result.pages.is_empty(),
        "résumé preview must produce at least one page"
    );
    for (i, page) in result.pages.iter().enumerate() {
        assert!(
            page.contains("<svg"),
            "résumé preview page {i} must contain <svg; got start: {:?}",
            &page[..page.len().min(80)]
        );
    }
    assert_eq!(
        result.mime_type, "image/svg+xml",
        "mime_type must be image/svg+xml"
    );
}

/// Cover-letter request → at least one SVG page.
#[tokio::test]
async fn preview_cover_letter_returns_svg_pages() {
    let request = ExportRequest {
        text: LETTER_FIXTURE_US.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Modern,
        meta: None,
        ats_mode: false,
        locale: Some("us".to_string()),
        contact: None,
    };
    let result = documents_render_preview_images(request)
        .await
        .expect("preview_cover_letter should succeed");

    assert!(
        !result.pages.is_empty(),
        "cover-letter preview must produce at least one page"
    );
    for (i, page) in result.pages.iter().enumerate() {
        assert!(
            page.contains("<svg"),
            "cover-letter preview page {i} must contain <svg; got start: {:?}",
            &page[..page.len().min(80)]
        );
    }
    assert_eq!(result.mime_type, "image/svg+xml");
}

/// Empty text → same Validation error as `documents_export_document` (shared
/// `validate_and_normalize`).
#[tokio::test]
async fn preview_empty_text_is_rejected() {
    let request = ExportRequest {
        text: "".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
    };
    let err = documents_render_preview_images(request)
        .await
        .expect_err("empty text must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("Cannot export empty document"),
        "expected the shared empty-text error, got: {msg}"
    );
}

/// Whitespace-only text → same Validation error (trim check in
/// `validate_and_normalize`).
#[tokio::test]
async fn preview_whitespace_text_is_rejected() {
    let request = ExportRequest {
        text: "   \n\t  ".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
    };
    let err = documents_render_preview_images(request)
        .await
        .expect_err("whitespace-only text must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("Cannot export empty document"),
        "expected the shared empty-text error, got: {msg}"
    );
}

/// Unknown templateId deserializes to Classic (serde fallback) — the preview
/// command must NOT error on an unknown id.
#[tokio::test]
async fn preview_unknown_template_id_falls_back_to_classic() {
    // Construct via JSON round-trip to exercise the serde-tolerant Deserialize.
    let json = serde_json::json!({
        "text": FIXTURE_RESUME,
        "format": "pdf",
        "documentType": "resume",
        "templateId": "bogus-unknown-id",
        "atsMode": false,
    });
    let request: ExportRequest =
        serde_json::from_value(json).expect("should deserialize with Classic fallback");

    assert_eq!(
        request.template_id,
        TemplateId::Classic,
        "unknown templateId must fall back to Classic before reaching the command"
    );

    // And the command itself must succeed (not error).
    let result = documents_render_preview_images(request)
        .await
        .expect("unknown templateId must not error — Classic fallback renders successfully");
    assert!(!result.pages.is_empty());
}

// ── documents_export_document (existing surface, matching error path) ─────────

/// Confirm the export command uses the SAME empty-text error so the two
/// commands stay in lock-step if `validate_and_normalize` is changed.
#[tokio::test]
async fn export_empty_text_error_matches_preview_error() {
    let mk_request = |text: &str| ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
    };

    let export_err = documents_export_document(mk_request(""))
        .await
        .expect_err("export must reject empty text");
    let preview_err = documents_render_preview_images(mk_request(""))
        .await
        .expect_err("preview must reject empty text");

    // Both must carry the same human-readable fragment.
    assert!(
        export_err
            .to_string()
            .contains("Cannot export empty document"),
        "export error: {export_err}"
    );
    assert!(
        preview_err
            .to_string()
            .contains("Cannot export empty document"),
        "preview error: {preview_err}"
    );
}

// ── Existing helpers ──────────────────────────────────────────────────────────

#[test]
fn test_sanitize_filename() {
    assert_eq!(sanitize_filename("John Doe"), "John-Doe");
    assert_eq!(sanitize_filename("John@Doe!"), "JohnDoe");
    assert_eq!(sanitize_filename("  Spaces  "), "Spaces");
}

#[test]
fn test_generate_filename() {
    use super::super::types::{DocumentType, ExportFormat, GenerationMeta, TemplateId};

    let request = ExportRequest {
        text: "Test".to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Modern,
        meta: Some(GenerationMeta {
            candidate_name: Some("John Doe".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Tech Corp".to_string()),
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: None,
    };

    let filename = generate_filename(&request, "docx");
    assert!(filename.contains("John-Doe"));
    assert!(filename.contains("Software-Engineer"));
    assert!(filename.contains("resume"));
    assert!(filename.ends_with(".docx"));
}
