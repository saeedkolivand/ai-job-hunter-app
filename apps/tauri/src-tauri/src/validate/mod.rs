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

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::export::parser::{parse_resume, strip_md};
use crate::export::types::{DocumentType, ExportFormat, ExportRequest, LineKind, TemplateId};

static EMAIL_RE: LazyLock<Regex> =
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
    generate: impl Fn(&ExportRequest) -> Result<Vec<u8>>,
) -> Result<(Vec<u8>, ExportReport)> {
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
    let can_linearize = matches!(request.template_id, TemplateId::TwoColumn) && !request.ats_mode;
    if has_critical(&issues) && can_linearize {
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

/// Critical content expected to survive a round trip, parsed from the source.
struct Expected {
    name: Option<String>,
    email: Option<String>,
    /// Section headings in source order (empty for cover letters).
    headings: Vec<String>,
}

fn expected_from_request(request: &ExportRequest) -> Expected {
    let parsed = parse_resume(&request.text);

    let mut name = None;
    let mut headings = Vec::new();
    for line in &parsed.lines {
        match line.kind {
            LineKind::Name if name.is_none() => name = Some(strip_md(&line.text)),
            LineKind::SectionHeader => headings.push(strip_md(&line.text)),
            _ => {}
        }
    }

    // The candidate name from metadata overrides the parsed first line.
    if let Some(meta_name) = request
        .meta
        .as_ref()
        .and_then(|m| m.candidate_name.as_deref())
    {
        if !meta_name.trim().is_empty() {
            name = Some(meta_name.to_string());
        }
    }

    let email = EMAIL_RE.find(&request.text).map(|m| m.as_str().to_string());

    Expected {
        name,
        email,
        headings,
    }
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
        Err(_) => {
            return vec![ExportIssue::warning(
                "roundtrip_unavailable",
                "Could not re-read the exported file to verify it; export proceeded unchecked.",
            )]
        }
    };

    let expected = expected_from_request(request);
    // Column interleaving is only possible for a two-column layout that has not
    // been linearized to a single column.
    let two_column = matches!(request.template_id, TemplateId::TwoColumn) && !request.ats_mode;
    evaluate(&expected, &extracted, two_column, request.document_type)
}

/// Compare the expected content against the re-extracted text.
fn evaluate(
    expected: &Expected,
    extracted: &str,
    two_column: bool,
    doc_type: DocumentType,
) -> Vec<ExportIssue> {
    let mut issues = Vec::new();
    let hay = normalize(extracted);

    // A document that produces (almost) no extractable text is broken in a way
    // that linearizing cannot fix — block it.
    let has_expected_content =
        expected.name.is_some() || expected.email.is_some() || !expected.headings.is_empty();
    if has_expected_content && hay.len() < 20 {
        issues.push(ExportIssue::critical(
            "no_extractable_text",
            "The exported file has no machine-readable text — most parsers and ATS \
             systems would see an empty document.",
        ));
        return issues; // nothing else is meaningful
    }

    // Identity: extraction is imperfect, so a miss is a warning, not a block.
    if let Some(name) = &expected.name {
        let n = normalize(name);
        if !n.is_empty() && !hay.contains(&n) {
            issues.push(ExportIssue::warning(
                "missing_name",
                format!("The name \u{201c}{name}\u{201d} was not found when re-reading the exported file."),
            ));
        }
    }
    if let Some(email) = &expected.email {
        if !hay.contains(&normalize(email)) {
            issues.push(ExportIssue::warning(
                "missing_email",
                "The email address was not found when re-reading the exported file.",
            ));
        }
    }

    // Section presence + reading order (resumes only).
    if matches!(doc_type, DocumentType::Resume) && !expected.headings.is_empty() {
        let mut positions: Vec<usize> = Vec::new();
        for h in &expected.headings {
            let hn = normalize(h);
            if hn.is_empty() {
                continue;
            }
            match hay.find(&hn) {
                Some(pos) => positions.push(pos),
                None => issues.push(ExportIssue::warning(
                    "missing_section",
                    format!("Section \u{201c}{h}\u{201d} was not found when re-reading the exported file."),
                )),
            }
        }

        // Only judge order when we recovered enough headings that a mismatch
        // means interleaving rather than an extraction gap.
        let recovered = positions.len();
        if recovered >= 2 && recovered * 2 >= expected.headings.len() {
            let out_of_order = positions.windows(2).any(|w| w[1] < w[0]);
            if out_of_order {
                if two_column {
                    issues.push(ExportIssue::critical(
                        "section_order",
                        "The two-column layout's sections interleave when read top-to-bottom, \
                         which ATS parsers misread.",
                    ));
                } else {
                    issues.push(ExportIssue::warning(
                        "section_order",
                        "Sections appear out of order when re-reading the exported file.",
                    ));
                }
            }
        }
    }

    issues
}

/// Lowercased, whitespace-collapsed, alphanumeric-only form for tolerant
/// `contains` / ordering checks against imperfect extraction.
fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn extract_pdf_text(bytes: &[u8]) -> Result<String> {
    pdf_extract::extract_text_from_mem(bytes).map_err(|e| anyhow::anyhow!("pdf extract: {e}"))
}

fn extract_docx_text(bytes: &[u8]) -> Result<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")?.read_to_string(&mut xml)?;
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
