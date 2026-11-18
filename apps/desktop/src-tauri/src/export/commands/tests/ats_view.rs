//! The ATS view rides on `ExportResult::ats_view`, moved OFF the report by
//! `documents_export_document` — the only wiring onto the wire result, and what the
//! NotExposed `export_ats_view` policy row relies on for agents.

use super::*;

fn request(format: ExportFormat, document_type: DocumentType, text: &str) -> ExportRequest {
    ExportRequest {
        text: text.to_string(),
        format,
        document_type,
        ..default_request()
    }
}

#[tokio::test]
async fn resume_exports_carry_the_view_on_the_result_not_inside_the_report() {
    for format in [ExportFormat::Pdf, ExportFormat::Docx] {
        let result =
            documents_export_document(request(format, DocumentType::Resume, FIXTURE_RESUME))
                .await
                .expect("export");
        let view = result.ats_view.as_ref().expect("ats_view on the result");
        assert_eq!(view.contact.name.value.as_deref(), Some("Jane Doe"));
        let report = result.report.as_ref().expect("report");
        assert!(
            report.ats_view.is_none(),
            "{format:?}: moved off the report"
        );

        // And on the wire: top-level `atsView`, none inside `report`.
        let wire = serde_json::to_value(&result).unwrap();
        assert!(wire.get("atsView").is_some(), "{format:?}");
        assert!(wire["report"].get("atsView").is_none(), "{format:?}");
    }
}

#[tokio::test]
async fn txt_and_cover_letter_exports_have_no_view() {
    let txt = documents_export_document(request(
        ExportFormat::Txt,
        DocumentType::Resume,
        FIXTURE_RESUME,
    ))
    .await
    .expect("txt export");
    assert!(txt.ats_view.is_none());

    for format in [ExportFormat::Pdf, ExportFormat::Docx] {
        let letter = documents_export_document(request(
            format,
            DocumentType::CoverLetter,
            LETTER_FIXTURE_US,
        ))
        .await
        .expect("letter export");
        assert!(letter.ats_view.is_none(), "{format:?}");
        assert!(letter.report.expect("report").ats_view.is_none());
    }
}
