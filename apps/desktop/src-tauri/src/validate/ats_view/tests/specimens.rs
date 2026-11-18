//! Real specimens: an actual résumé rendered by the real PDF/DOCX backends for
//! every canonical template, re-extracted exactly as the export gate does,
//! then read by the ATS view. These pin what each template family does TODAY,
//! including its known ATS shortcomings (called out per test).

use super::super::{
    ats_view_from_text, AtsHints, AtsReason, AtsRole, AtsSectionKind, AtsStatus, AtsView,
};
use super::support::*;
use crate::export::templates::CANONICAL_TEMPLATE_IDS;
use crate::export::types::{DocumentType, ExportFormat, TemplateId};
use crate::validate::validate_and_fix;

const GERMAN: &str = "\
Erika Mustermann
München, Deutschland | +49 89 1234 5678 | erika@example.de

PROFIL
Softwareentwicklerin mit acht Jahren Erfahrung.

BERUFSERFAHRUNG
Beispiel GmbH  März 2021 – heute
Senior Entwicklerin
- Leitung eines Teams

Muster AG  03/2018 – 02/2021
Entwicklerin
- Aufbau der Abrechnung

AUSBILDUNG
Technische Universität München  2013 – 2017
M.Sc. Informatik

KENNTNISSE
- Rust, TypeScript
";

const TWO_COLUMN: [TemplateId; 4] = [
    TemplateId::Atelier,
    TemplateId::Portrait,
    TemplateId::Aria,
    TemplateId::Saffron,
];

fn view(text: &str, fmt: ExportFormat, id: TemplateId, ats: bool, name: &str) -> AtsView {
    let extracted = extracted(&request(text, fmt, id, ats));
    ats_view_from_text(&extracted, AtsHints { name: Some(name) })
}

fn assert_role(r: &AtsRole, title: &str, company: &str, start: &str, end: &str, ctx: &str) {
    for (f, want) in [
        (&r.title, title),
        (&r.company, company),
        (&r.start, start),
        (&r.end, end),
    ] {
        assert_eq!(f.status, AtsStatus::Found, "{ctx}: {r:?}");
        assert_eq!(f.value.as_deref(), Some(want), "{ctx}: {r:?}");
    }
}

/// Identity + dates survive every template and both formats — the contract.
#[test]
fn every_template_and_format_keeps_contact_sections_and_role_dates() {
    for id in CANONICAL_TEMPLATE_IDS {
        for fmt in [ExportFormat::Pdf, ExportFormat::Docx] {
            let ctx = format!("{id:?} {fmt:?}");
            let v = view(RICH, fmt, id, false, "Jane Doe");
            let c = &v.contact;
            assert_eq!(c.name.status, AtsStatus::Found, "{ctx}");
            assert_eq!(c.email.value.as_deref(), Some("jane@example.com"), "{ctx}");
            assert_eq!(
                c.location.value.as_deref(),
                Some("Berlin, Germany"),
                "{ctx}"
            );
            assert!(
                c.phone.value.as_deref().unwrap_or("").contains("2345"),
                "{ctx}: {:?}",
                c.phone
            );
            for kind in [
                AtsSectionKind::Summary,
                AtsSectionKind::Experience,
                AtsSectionKind::Education,
                AtsSectionKind::Skills,
            ] {
                assert!(
                    v.sections
                        .iter()
                        .any(|s| s.kind == kind && s.status != AtsStatus::Missing),
                    "{ctx}: {kind:?} section lost: {:?}",
                    v.sections
                );
            }
            assert_eq!(v.roles.len(), 2, "{ctx}");
            for (r, (s, e)) in v
                .roles
                .iter()
                .zip([("2021-03", "present"), ("2018-03", "2021-02")])
            {
                assert_eq!(r.start.value.as_deref(), Some(s), "{ctx}");
                assert_eq!(r.end.value.as_deref(), Some(e), "{ctx}");
            }
        }
    }
}

/// Every single-column template reads cleanly end to end in PDF and DOCX. That
/// includes the Lebenslauf PDF (company and title come out of pdf-extract fused)
/// and the SwissMinimal PDF (headings fused with their content): `pdftotext` reads
/// both with the pieces apart, so they are extractor artifacts, reported as found.
#[test]
fn single_column_templates_are_fully_found() {
    for id in CANONICAL_TEMPLATE_IDS {
        if TWO_COLUMN.contains(&id) {
            continue;
        }
        for fmt in [ExportFormat::Pdf, ExportFormat::Docx] {
            let ctx = format!("{id:?} {fmt:?}");
            let v = view(RICH, fmt, id, false, "Jane Doe");
            let s = v.summary;
            assert_eq!(s.found, s.total, "{ctx}: {v:?}");
            assert_role(
                &v.roles[0],
                "Senior Engineer",
                "Acme Corp",
                "2021-03",
                "present",
                &ctx,
            );
            assert_role(
                &v.roles[1],
                "Software Engineer",
                "Beta GmbH",
                "2018-03",
                "2021-02",
                &ctx,
            );
        }
    }
}

/// FINDING: a two-column PDF extracts its sidebar BEFORE the name/summary, so
/// a rigid parser reads the body before the header. DOCX (linearised) and the
/// ATS-mode single-column PDF are in order — the heuristic separates them.
#[test]
fn two_column_pdfs_interleave_but_docx_and_ats_mode_do_not() {
    for id in TWO_COLUMN {
        let pdf = view(RICH, ExportFormat::Pdf, id, false, "Jane Doe");
        assert_eq!(pdf.reading_order.status, AtsStatus::Suspicious, "{id:?}");
        assert_eq!(
            pdf.reading_order.reason,
            Some(AtsReason::Interleaved),
            "{id:?}"
        );

        let ats_pdf = view(RICH, ExportFormat::Pdf, id, true, "Jane Doe");
        assert_eq!(
            ats_pdf.reading_order.status,
            AtsStatus::Found,
            "{id:?} ats mode"
        );
        let docx = view(RICH, ExportFormat::Docx, id, false, "Jane Doe");
        assert_eq!(docx.reading_order.status, AtsStatus::Found, "{id:?} docx");
    }
}

/// FINDING (real template defect): the Portrait PDF wraps the phone number across
/// two lines in its narrow sidebar. `pdftotext -layout` and `-raw` both show the
/// same wrap (`+49 151 2345` / `6789 | …`), so it is not an extractor artifact.
#[test]
fn portrait_pdf_phone_wraps() {
    let v = view(
        RICH,
        ExportFormat::Pdf,
        TemplateId::Portrait,
        false,
        "Jane Doe",
    );
    assert_eq!(v.contact.phone.reason, Some(AtsReason::Wrapped));
}

/// pdf-extract joins the Lebenslauf PDF's company and title with no separator
/// (`Acme CorpSenior Engineer`) and SwissMinimal's headings with their content
/// (`WORK EXPERIENCEAcme Corp`); `pdftotext` shows both files with the pieces apart.
/// The view splits them and reports the real values as found — not a user-fixable flag.
#[test]
fn pdf_extractor_glyph_joins_are_split_and_found() {
    let leb = view(
        RICH,
        ExportFormat::Pdf,
        TemplateId::Lebenslauf,
        false,
        "Jane Doe",
    );
    for (r, (t, c)) in leb.roles.iter().zip([
        ("Senior Engineer", "Acme Corp"),
        ("Software Engineer", "Beta GmbH"),
    ]) {
        assert_eq!(r.title.status, AtsStatus::Found, "{r:?}");
        assert_eq!(r.company.status, AtsStatus::Found, "{r:?}");
        assert_eq!(r.title.value.as_deref(), Some(t));
        assert_eq!(r.company.value.as_deref(), Some(c));
    }
    let swiss = view(
        RICH,
        ExportFormat::Pdf,
        TemplateId::SwissMinimal,
        false,
        "Jane Doe",
    );
    assert!(
        swiss.sections.iter().all(|s| s.status == AtsStatus::Found),
        "{:?}",
        swiss.sections
    );
    assert_eq!(swiss.roles.len(), 2);
}

#[test]
fn german_resume_reads_in_german_across_formats() {
    for (id, fmt) in [
        (TemplateId::Classic, ExportFormat::Pdf),
        (TemplateId::Classic, ExportFormat::Docx),
        (TemplateId::Lebenslauf, ExportFormat::Docx),
        (TemplateId::Cadence, ExportFormat::Pdf),
    ] {
        let ctx = format!("{id:?} {fmt:?}");
        let mut r = request(GERMAN, fmt, id, false);
        r.locale = Some("de".to_string());
        let extracted = extracted(&r);
        let v = ats_view_from_text(
            &extracted,
            AtsHints {
                name: Some("Erika Mustermann"),
            },
        );
        assert_eq!(
            v.contact.email.value.as_deref(),
            Some("erika@example.de"),
            "{ctx}"
        );
        assert_eq!(
            v.contact.location.value.as_deref(),
            Some("München, Deutschland"),
            "{ctx}"
        );
        assert_eq!(v.roles.len(), 2, "{ctx}: {:?}", v.roles);
        assert_eq!(v.roles[0].start.value.as_deref(), Some("2021-03"), "{ctx}");
        assert_eq!(v.roles[0].end.value.as_deref(), Some("present"), "{ctx}");
        assert_eq!(v.roles[1].end.value.as_deref(), Some("2021-02"), "{ctx}");
        assert!(
            v.sections.iter().all(|s| s.status != AtsStatus::Missing),
            "{ctx}: {:?}",
            v.sections
        );
    }
}

// ── export integration ───────────────────────────────────────────────────

fn export(text: &str, fmt: ExportFormat, doc: DocumentType) -> crate::validate::ExportReport {
    let mut r = request(text, fmt, TemplateId::Classic, false);
    r.document_type = doc;
    let generate = |r: &crate::export::types::ExportRequest| match r.format {
        ExportFormat::Pdf => crate::export::pdf::generate_pdf(r),
        _ => crate::export::docx::generate_docx(r).map_err(crate::error::AppError::from),
    };
    validate_and_fix(r, generate).expect("export").1
}

#[test]
fn export_attaches_the_view_for_resumes_only() {
    for fmt in [ExportFormat::Pdf, ExportFormat::Docx] {
        let view = export(RICH, fmt, DocumentType::Resume)
            .ats_view
            .expect("resume view");
        assert_eq!(view.contact.name.value.as_deref(), Some("Jane Doe"));
        assert!(export(
            "Dear team,\n\nI am applying.\n\nJane Doe",
            fmt,
            DocumentType::CoverLetter
        )
        .ats_view
        .is_none());
    }
    assert!(export(RICH, ExportFormat::Txt, DocumentType::Resume)
        .ats_view
        .is_none());
}

#[test]
fn the_view_never_changes_whether_an_export_succeeds() {
    // A document the view dislikes (creative heading, no contact) still exports ok.
    let report = export(
        "Jane Doe\n\nMY JOURNEY\nSome text here for the document body.\n",
        ExportFormat::Pdf,
        DocumentType::Resume,
    );
    assert!(report.ok, "{:?}", report.issues);
    assert!(report.ats_view.unwrap().summary.missing > 0);
}
