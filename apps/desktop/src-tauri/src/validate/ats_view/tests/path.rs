//! `ats_view_for_file`: the user-controlled path is validated before any read.

use super::super::ats_view_for_file;
use super::support::*;
use crate::error::AppError;
use crate::export::types::{ExportFormat, TemplateId};

fn rejected(path: &str) {
    let err = ats_view_for_file(path).expect_err(path);
    assert!(matches!(err, AppError::Validation(_)), "{path:?}: {err:?}");
    // The path is never echoed back.
    assert!(path.len() < 4 || !err.to_string().contains(path), "{err}");
}

#[test]
fn rejects_empty_nul_traversal_and_wrong_extension() {
    rejected("");
    rejected("   ");
    rejected("resume\0.pdf");
    rejected("../secrets/resume.pdf");
    rejected("exports/../../resume.docx");
    rejected("resume.txt");
    rejected("resume");
    rejected("resume.pdf.exe");
}

#[test]
fn rejects_overlong_path() {
    rejected(&format!("{}.pdf", "a".repeat(5000)));
}

/// Network and device forms are refused by the path check itself ("invalid path"),
/// never by a failed open: a failed open would already have made the OS connect out.
#[test]
fn rejects_unc_device_and_verbatim_forms_before_any_fs_call() {
    for path in [
        r"\\attacker.example\share\cv.pdf",
        "//attacker.example/share/cv.pdf",
        r"\\?\UNC\attacker.example\share\cv.pdf",
        r"\\?\C:\Users\x\cv.pdf",
        r"\\.\pipe\cv.pdf",
        r"\\.\GLOBALROOT\Device\x\cv.docx",
        r"\??\UNC\attacker.example\share\cv.pdf",
        r"\??\C:\Users\x\cv.pdf",
        r"C:\Users\x\cv.pdf:stream.pdf",
        "relative/cv.pdf",
    ] {
        rejected(path);
        let msg = ats_view_for_file(path).unwrap_err().to_string();
        assert!(msg.contains("invalid path"), "{path}: {msg}");
    }
}

/// A tiny zip whose `word/document.xml` inflates past the cap.
fn docx_bomb(uncompressed: usize) -> Vec<u8> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("word/document.xml", opts).unwrap();
    let chunk = vec![b' '; 1024 * 1024];
    for _ in 0..uncompressed / chunk.len() {
        zip.write_all(&chunk).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn docx_that_inflates_past_the_cap_is_refused() {
    let bytes = docx_bomb(21 * 1024 * 1024);
    assert!(bytes.len() < 1024 * 1024, "the bomb must be small on disk");
    assert!(crate::validate::extract_docx_text(&bytes).is_err());
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("bomb.docx");
    std::fs::write(&p, bytes).unwrap();
    rejected(p.to_str().unwrap());
}

#[test]
fn docx_just_under_the_cap_still_reads() {
    let bytes = docx_bomb(19 * 1024 * 1024);
    assert!(crate::validate::extract_docx_text(&bytes).is_ok());
}

#[test]
fn rejects_missing_file_directory_and_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    rejected(dir.path().join("nope.pdf").to_str().unwrap());
    let sub = dir.path().join("folder.pdf");
    std::fs::create_dir(&sub).unwrap();
    rejected(sub.to_str().unwrap());
    let empty = dir.path().join("empty.pdf");
    std::fs::write(&empty, b"").unwrap();
    rejected(empty.to_str().unwrap());
}

#[test]
fn garbage_with_a_valid_extension_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("junk.pdf", &b"%PDF-1.4 definitely not a pdf"[..]),
        ("junk.docx", &b"PK not a zip"[..]),
        ("renamed.pdf", &b"plain text"[..]),
    ] {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        rejected(p.to_str().unwrap());
    }
}

#[test]
fn a_real_exported_pdf_and_docx_read_back_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    for (fmt, name) in [
        (ExportFormat::Pdf, "cv.pdf"),
        (ExportFormat::Docx, "CV.DOCX"),
    ] {
        let bytes = render(&request(RICH, fmt, TemplateId::Classic, false));
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        let v = ats_view_for_file(p.to_str().unwrap()).expect(name);
        assert_eq!(v.contact.name.value.as_deref(), Some("Jane Doe"), "{name}");
        assert_eq!(v.roles.len(), 2, "{name}");
        assert_eq!(v.summary.missing, 0, "{name}");
    }
}

// ── absolute paths: the LATER checks decide, not the absolute-path allowlist ──

/// `path` passes the allowlist (so a later check is what rejects it) and is refused
/// with a message containing `why`.
fn refused_by(path: &str, why: &str) {
    assert!(
        super::super::file::is_local_absolute(path),
        "precondition: {path:?} must pass the absolute-path allowlist"
    );
    let msg = ats_view_for_file(path).expect_err(path).to_string();
    assert!(msg.contains(why), "{path:?}: expected {why:?}, got {msg}");
}

/// Forward-slash form of a path (the allowlist accepts both separators on Windows).
fn slashed(p: &std::path::Path) -> String {
    p.to_str().unwrap().replace(std::path::MAIN_SEPARATOR, "/")
}

fn dir_str(dir: &tempfile::TempDir) -> String {
    slashed(dir.path())
}

#[test]
fn absolute_traversal_nul_length_and_extension_are_rejected_by_their_own_checks() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir_str(&dir);
    std::fs::create_dir(dir.path().join("a")).unwrap();
    std::fs::write(dir.path().join("b.pdf"), b"%PDF-1.4").unwrap();
    std::fs::write(dir.path().join("x.txt"), b"plain").unwrap();

    refused_by(&format!("{base}/a/../b.pdf"), "invalid path");
    refused_by(&format!("{base}/x\0.pdf"), "invalid path");
    refused_by(&format!("{base}/{}.pdf", "a".repeat(5000)), "invalid path");
    refused_by(&format!("{base}/x.txt"), "only PDF and DOCX");
    refused_by(&format!("{base}/x.pdf.exe"), "only PDF and DOCX");
}

#[test]
fn a_symlink_with_a_pdf_name_to_a_non_pdf_is_refused_on_the_resolved_extension() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("secret.txt");
    std::fs::write(&target, b"not a pdf").unwrap();
    let link = dir.path().join("looks.pdf");
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&target, &link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(&target, &link).is_ok();
    if !made {
        return; // symlinks need a privilege this host does not grant
    }
    // Windows refuses any symlink in the path up front; Unix resolves it and judges the target.
    let why = if cfg!(windows) {
        "invalid path"
    } else {
        "only PDF and DOCX"
    };
    refused_by(&slashed(&link), why);
}

#[cfg(windows)]
#[test]
fn a_symlinked_directory_in_the_path_is_refused_before_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    std::fs::write(real.join("cv.pdf"), b"%PDF-1.4").unwrap();
    let link = dir.path().join("link");
    if std::os::windows::fs::symlink_dir(&real, &link).is_err() {
        return; // symlinks need a privilege this host does not grant
    }
    refused_by(&slashed(&link.join("cv.pdf")), "invalid path");
}

#[test]
fn a_file_over_the_size_cap_is_refused_without_being_read() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.pdf");
    let f = std::fs::File::create(&big).unwrap();
    f.set_len(crate::extraction::MAX_BYTES as u64 + 1).unwrap();
    drop(f);
    refused_by(&slashed(&big), "too large");
}
