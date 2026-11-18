//! `ats_view_for_file`: read a previously exported file from a user-controlled path.

use std::io::Read;
use std::path::{Component, Path, Prefix};

use super::{ats_view_from_text, AtsHints, AtsView, MAX_FILE_BYTES};
use crate::error::{AppError, AppResult};

/// An allowlist, not a denylist: Windows has too many spellings of a remote or device
/// path (`\\host\share`, `\\?\UNC\…`, `\??\UNC\…`, `\\.\…`) to enumerate, so only a plain
/// drive path (`C:\…` / `C:/…`, no `?`, no second `:`) passes. Elsewhere only an absolute
/// POSIX path (and not `//host/…`, which some systems treat as a network root).
pub(super) fn is_local_absolute(path: &str) -> bool {
    if cfg!(windows) {
        let b = path.as_bytes();
        b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && matches!(b[2], b'\\' | b'/')
            && !path.contains('?')
            && !path[2..].contains(':')
    } else {
        path.starts_with('/') && !path.starts_with("//")
    }
}

/// Read a previously exported PDF/DOCX and build its view (no source name known).
///
/// `path` is user-controlled: it must be a plain path (no `..` component) to
/// an existing regular `.pdf`/`.docx` file of at most [`MAX_FILE_BYTES`]. The
/// path is never echoed in an error.
pub fn ats_view_for_file(path: &str) -> AppResult<AtsView> {
    let bad = |why: &str| AppError::Validation(format!("Cannot read this file: {why}."));
    if path.trim().is_empty() || path.len() > 4096 || path.contains('\0') {
        return Err(bad("invalid path"));
    }
    // Before ANY filesystem call: a network/device path makes the OS connect out
    // (SMB/WebDAV, NTLM) on open or canonicalize, whatever the extension.
    if !is_local_absolute(path) {
        return Err(bad("invalid path"));
    }
    let raw = Path::new(path);
    let refused = raw.components().any(|c| match c {
        Component::ParentDir => true,
        Component::Prefix(p) => !matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)),
        _ => false,
    });
    if refused {
        return Err(bad("invalid path"));
    }
    let ext = raw
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("pdf" | "docx")) {
        return Err(bad("only PDF and DOCX files are supported"));
    }
    // ponytail: Windows-only. A local symlink/junction aimed at a UNC share would make
    // canonicalize connect out (NTLM leak) before any later check. `is_symlink` covers
    // symlinks and junctions but not OneDrive placeholders. Unix is exempt: macOS
    // `/var` and `/tmp` are symlinks.
    #[cfg(windows)]
    if raw
        .ancestors()
        .any(|a| std::fs::symlink_metadata(a).is_ok_and(|m| m.file_type().is_symlink()))
    {
        return Err(bad("invalid path"));
    }
    // Resolve symlinks, re-check the extension of what the path really points at, then
    // judge the OPENED handle (not the path again), so a swap cannot change what is read.
    let real = std::fs::canonicalize(raw).map_err(|_| bad("file not found"))?;
    let real_ext = real
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(real_ext.as_deref(), Some("pdf" | "docx")) {
        return Err(bad("only PDF and DOCX files are supported"));
    }
    let file = std::fs::File::open(&real).map_err(|_| bad("file not readable"))?;
    let meta = file.metadata().map_err(|_| bad("file not readable"))?;
    if !meta.is_file() {
        return Err(bad("not a file"));
    }
    if meta.len() == 0 || meta.len() > MAX_FILE_BYTES {
        return Err(bad("file is empty or too large"));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(MAX_FILE_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|_| bad("file not readable"))?;
    let ext = real_ext;

    // Same crate and the same residual risk (a malformed PDF can panic pdf-extract) as the
    // user-PDF path in `extraction/pdf.rs`; release builds abort on panic, so no catch here.
    let extracted = match ext.as_deref() {
        Some("pdf") => crate::validate::extract_pdf_text(&bytes),
        _ => crate::validate::extract_docx_text(&bytes),
    }
    .map_err(|_| bad("the file could not be parsed"))?;
    Ok(ats_view_from_text(&extracted, AtsHints::default()))
}
