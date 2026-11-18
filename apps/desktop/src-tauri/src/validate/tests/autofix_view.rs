//! The auto-fix re-render must replace the ATS view too: the view returned with the
//! bytes is the one built from the bytes actually returned, never from the rejected render.

use std::cell::Cell;
use std::io::Write;

use super::{support::*, *};

/// A structurally valid DOCX with no text at all: the readback's critical
/// `no_extractable_text`.
fn empty_docx() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("word/document.xml", opts).unwrap();
    zip.write_all(b"<w:document><w:body></w:body></w:document>")
        .unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn the_returned_view_is_the_second_renders_after_an_auto_fix() {
    let calls = Cell::new(0);
    let (bytes, report) =
        validate_and_fix(req(ExportFormat::Docx, TemplateId::Atelier, false), |r| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                assert!(!r.ats_mode, "first render is the user's own layout");
                Ok(empty_docx())
            } else {
                assert!(r.ats_mode, "the re-render is the ATS-safe one");
                crate::export::docx::generate_docx(r).map_err(crate::error::AppError::from)
            }
        })
        .expect("export");

    assert_eq!(
        calls.get(),
        2,
        "critical first render must trigger one re-render"
    );
    assert!(report.ok, "{:?}", report.issues);
    assert!(report.ats_mode);
    assert_eq!(report.fixed.len(), 1);
    assert!(
        bytes.len() > empty_docx().len(),
        "the re-render's bytes are returned"
    );

    let view = report.ats_view.expect("view from the re-render");
    assert_eq!(view.contact.name.value.as_deref(), Some("Jane Doe"));
    assert_eq!(
        view.contact.email.value.as_deref(),
        Some("jane@example.com")
    );
    assert!(
        !view.roles.is_empty(),
        "an empty first render would have no roles"
    );
}
