//! Pre-export validation + ATS round-trip gate.
//!
//! After a backend renders a resume / cover letter to bytes, this module
//! re-extracts the text from those bytes the way an ATS parser (or a human
//! pasting into a form) would, and checks that the document's critical content
//! survived in a sane reading order.
//!
//! The dangerous failure mode is a two-column PDF whose columns interleave when
//! read top-to-bottom — ATS parsers then shred the resume. When that is detected
//! the document is re-exported single-column (ATS-safe via `ats_mode`) and
//! re-checked. An export is **blocked** only when a *critical* defect survives
//! that auto-fix (e.g. the exported file has no extractable text at all).
//!
//! DOCX already linearizes two-column templates to a single column, so its
//! round-trip is a content-survival check rather than a column-order gate.

use std::io::Read;
use std::sync::LazyLock;

use crate::error::{AppError, AppResult};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::export::types::{ExportFormat, ExportRequest};

mod header_links;
mod pdf_links;
mod readback;

use self::header_links::pdf_render_issues;
use self::readback::{evaluate, expected_from_request, stray_markdown_issues};

/// The single "is this a real email address" test in the crate. `pub(crate)`
/// so the content validators police a genuine address rather than a bare `@`
/// (a body line mentioning "@channel" is not a contact block).
pub(crate) static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\w.+-]+@[\w-]+\.[\w.-]+").unwrap());

/// How serious an export issue is. Only [`Severity::Critical`] issues can block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
}

/// A single problem found while re-reading an exported document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportIssue {
    pub severity: Severity,
    /// Stable machine code (`section_order`, `missing_section`, …).
    pub code: String,
    /// Plain-language explanation for the user.
    pub message: String,
}

impl ExportIssue {
    fn critical(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Critical,
            code: code.into(),
            message: message.into(),
        }
    }
    fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Outcome of validating (and possibly auto-fixing) an export.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    /// `false` only when a critical defect survived auto-fix — the caller blocks.
    pub ok: bool,
    /// Whether the *returned* bytes were rendered in ATS (single-column) mode.
    pub ats_mode: bool,
    /// Remaining issues after any auto-fix (criticals here mean `ok == false`).
    pub issues: Vec<ExportIssue>,
    /// Human-readable description of each auto-fix that was applied.
    pub fixed: Vec<String>,
}

fn has_critical(issues: &[ExportIssue]) -> bool {
    issues.iter().any(|i| i.severity == Severity::Critical)
}

/// Render an export, validate the bytes by re-extraction, and auto-fix a
/// two-column layout that does not survive extraction by re-exporting it
/// single-column. Returns the (possibly re-rendered) bytes and a report.
///
/// `generate` renders the request to bytes; it is called again if an auto-fix is
/// needed. TXT has no layout, so it is returned unvalidated.
pub fn validate_and_fix(
    mut request: ExportRequest,
    generate: impl Fn(&ExportRequest) -> AppResult<Vec<u8>>,
) -> AppResult<(Vec<u8>, ExportReport)> {
    let mut bytes = generate(&request)?;

    if matches!(request.format, ExportFormat::Txt) {
        return Ok((
            bytes,
            ExportReport {
                ok: true,
                ats_mode: request.ats_mode,
                issues: vec![],
                fixed: vec![],
            },
        ));
    }

    let mut issues = run_validators(&request, &bytes);
    let mut fixed = Vec::new();

    // Auto-fix: a two-column layout whose sections interleave when read back is
    // re-exported single-column (linearized), then re-checked.
    let can_linearize = crate::theme::is_two_column(request.template_id) && !request.ats_mode;
    if has_critical(&issues) && can_linearize {
        // A downgrade silently changes the user's chosen layout, so it must never
        // be invisible (the lesson from the silent two-column→single-column bug).
        let codes: Vec<&str> = issues
            .iter()
            .filter(|i| i.severity == Severity::Critical)
            .map(|i| i.code.as_str())
            .collect();
        log::warn!(
            "export: re-rendering two-column {:?} as single-column because critical issues survived validation: {:?}",
            request.template_id,
            codes,
        );
        request.ats_mode = true;
        bytes = generate(&request)?;
        issues = run_validators(&request, &bytes);
        fixed.push(
            "Re-exported in ATS-safe single-column layout because the two-column \
             layout's sections interleaved when read back."
                .to_string(),
        );
    }

    let ok = !has_critical(&issues);
    Ok((
        bytes,
        ExportReport {
            ok,
            ats_mode: request.ats_mode,
            issues,
            fixed,
        },
    ))
}

fn run_validators(request: &ExportRequest, bytes: &[u8]) -> Vec<ExportIssue> {
    let extracted = match request.format {
        ExportFormat::Pdf => extract_pdf_text(bytes),
        ExportFormat::Docx => extract_docx_text(bytes),
        ExportFormat::Txt => return Vec::new(),
    };

    let extracted = match extracted {
        Ok(t) => t,
        // Never block on a tooling failure — surface it as a note instead.
        // Log the failure's *kind* only (the prefix before the first ':' —
        // "pdf extract" / "docx open" / "docx read entry" / "docx read xml",
        // set by `extract_pdf_text`/`extract_docx_text` below), never the
        // full `{e}` text: these are structural pdf_extract/zip failures, not
        // a content problem, and the full message is the underlying crate's
        // own text, not ours to vouch for.
        Err(e) => {
            let msg = e.to_string();
            let kind = msg.split(':').next().unwrap_or("unknown");
            log::warn!(
                "export: could not re-extract {:?} bytes for validation ({kind}); \
                 export proceeded unchecked",
                request.format
            );
            return vec![ExportIssue::warning(
                "roundtrip_unavailable",
                "Could not re-read the exported file to verify it; export proceeded unchecked.",
            )];
        }
    };

    let expected = expected_from_request(request);
    // Column interleaving is only possible for a two-column layout that has not
    // been linearized to a single column.
    let two_column = crate::theme::is_two_column(request.template_id) && !request.ats_mode;
    let mut issues = evaluate(&expected, &extracted, two_column, request.document_type);

    // Render-correctness gates that must hold for EVERY generated document, so the
    // header-link and stray-markdown defects can't regress for a future résumé.
    issues.extend(stray_markdown_issues(&extracted));
    if matches!(request.format, ExportFormat::Pdf) {
        issues.extend(pdf_render_issues(request, bytes));
    }
    issues
}

fn extract_pdf_text(bytes: &[u8]) -> AppResult<String> {
    pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| AppError::Parse(format!("pdf extract: {e}")))
}

fn extract_docx_text(bytes: &[u8]) -> AppResult<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| AppError::Parse(format!("docx open: {e}")))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .map_err(|e| AppError::Parse(format!("docx read entry: {e}")))?
        .read_to_string(&mut xml)
        .map_err(|e| AppError::Parse(format!("docx read xml: {e}")))?;
    Ok(strip_xml_tags(&xml))
}

/// Strip XML tags, replacing each with a space so adjacent runs don't fuse, then
/// decode the handful of entities docx-rs emits.
fn strip_xml_tags(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests;

/// Deterministic content validation of GENERATED text (facts, alignment,
/// language, voice) — the earlier sibling of this module's rendered-bytes gate.
pub mod content;
