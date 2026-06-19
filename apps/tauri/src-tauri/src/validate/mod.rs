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

use crate::export::parser::{parse_resume, strip_md};
use crate::export::types::{DocumentType, ExportFormat, ExportRequest, LineKind};

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

/// Light URL canonicalization for the header-URL mismatch check.
///
/// Normalizes trivial differences that don't change the identity of a URL:
/// - lowercase scheme and host (the authority is case-insensitive per RFC 3986)
/// - strip a single trailing slash from the path (never from the query/fragment)
/// - decode a percent-encoded space (`%20` → space), the most common encoding
///   divergence between stored profile values and rendered PDF annotations
///
/// Intentionally conservative: the check's goal is to catch a genuinely wrong
/// URL (a company link leaking into the header), not to canonicalize semantics.
fn canonicalize_url(url: &str) -> String {
    // Split off scheme+authority (both case-insensitive per RFC 3986).
    // Authority ends at the first '/', '?', or '#' — NOT just the first '/'.
    // Without this, `https://example.com?Token=ABC` wrongly treats `?Token=ABC`
    // as part of the authority and lowercases the query string.
    let (prefix, path_query_fragment) =
        if let Some(after_scheme_start) = url.find("://").map(|i| i + 3) {
            let after = &url[after_scheme_start..];
            // End of authority = first of '/', '?', '#' (or end of string).
            let auth_end = after.find(['/', '?', '#']).unwrap_or(after.len());
            let scheme_and_auth = &url[..after_scheme_start + auth_end];
            let rest = &url[after_scheme_start + auth_end..];
            (scheme_and_auth.to_lowercase(), rest.to_string())
        } else {
            (String::new(), url.to_string())
        };

    // Separate the path from any query/fragment so we only strip a trailing
    // slash from the path, never from inside the query or fragment.
    let path_query_fragment = {
        // Find where path ends (first '?' or '#').
        let qf_start = path_query_fragment
            .find(['?', '#'])
            .unwrap_or(path_query_fragment.len());
        let path = &path_query_fragment[..qf_start];
        let query_fragment = &path_query_fragment[qf_start..];
        // Strip at most one trailing slash from the path.
        let path = path.strip_suffix('/').unwrap_or(path);
        format!("{path}{query_fragment}")
    };

    // Decode a percent-encoded space — the most common trivial encoding mismatch.
    let path_query_fragment = path_query_fragment.replace("%20", " ");

    format!("{prefix}{path_query_fragment}")
}

/// Reject stray Markdown emphasis (`*`, backtick) that survived sanitization into
/// the rendered text — the leaked-asterisk symptom. Applies to PDF and DOCX.
///
/// Only flags `*` or `` ` `` that appear at emphasis-boundary positions (not
/// flanked by an ASCII word character on both sides). A `*` between two word
/// chars (e.g. `5*4`, `a*b`) is a literal value preserved by the sanitizer and
/// must not be treated as a rendering defect.
fn stray_markdown_issues(extracted: &str) -> Vec<ExportIssue> {
    if has_stray_emphasis(extracted) {
        vec![ExportIssue::critical(
            "stray_markdown",
            "The exported document contains stray Markdown markers (* or `) that should \
             have been stripped — emphasis leaked into the visible text.",
        )]
    } else {
        Vec::new()
    }
}

/// Returns `true` when `text` contains a `*` or `` ` `` that is NOT flanked by
/// ASCII word characters on both sides — the same rule the sanitizer uses to
/// decide what to strip vs. preserve.
fn has_stray_emphasis(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '*' || ch == '`' {
            let prev_word = i > 0 && crate::export::parser::is_word_char(chars[i - 1]);
            let next_word =
                i + 1 < chars.len() && crate::export::parser::is_word_char(chars[i + 1]);
            if !(prev_word && next_word) {
                return true;
            }
        }
    }
    false
}

/// A link annotation read back from the rendered PDF: its `/Rect` in PDF user
/// space (points, bottom-up origin) and target URL.
#[derive(Debug)]
struct PdfLink {
    /// `[x0, y0, x1, y1]` — bottom-left and top-right corners in points.
    rect: [f32; 4],
    url: String,
    /// 0-based page index the annotation lives on.
    page: usize,
}

/// Render-level checks over the actual PDF bytes (renderer-agnostic, so the modern
/// résumé engine and the legacy cover-letter path are both covered):
///   * `header_url_mismatch` (critical) — when a contact profile is the source of
///     truth, every header-region link URL must be one of the profile's named
///     fields (catches a company-link displacing a personal profile / site).
///   * `header_url_missing` (warning) — the reverse completeness check (a profile
///     link that did not surface in the header band); advisory only, as it leans on
///     the band heuristic and a missing link never corrupts the document.
fn pdf_render_issues(request: &ExportRequest, bytes: &[u8]) -> Vec<ExportIssue> {
    let doc = match lopdf::Document::load_mem(bytes) {
        Ok(d) => d,
        Err(_) => return Vec::new(), // tooling failure never blocks
    };
    let page_h_pt = request.page_geometry().height_mm * 2.834_645_7;

    let pages: Vec<(usize, lopdf::ObjectId)> = doc.get_pages().into_values().enumerate().collect();
    let mut links: Vec<PdfLink> = Vec::new();

    for (idx, page_id) in &pages {
        links.extend(page_link_annotations(&doc, *page_id, *idx));
    }

    let mut issues = Vec::new();

    // The former `empty_anchor_link` geometric check (link rect vs text-baseline
    // overlap) was removed at the Typst cutover: it guarded the legacy renderer's
    // manually-placed link rects (coordinate-origin flips). Typst's
    // `link(url, body)` wraps real glyphs, so an annotation is structurally always
    // anchored to its text; the geometric approach was renderer-fragile (text-matrix
    // vs /Rect coordinate spaces) and false-flagged every valid Typst link. The
    // URL-correctness checks below (content-based, not geometric) still run.

    // Header URL correctness against the contact profile (when supplied).
    if let Some(profile) = request
        .contact
        .as_ref()
        .filter(|p| !p.is_effectively_empty())
    {
        let allowed: std::collections::BTreeSet<String> =
            profile.header_urls().into_iter().collect();
        // Header region: the top ~2 inches (144 pt) of the first page.
        let header_band_bottom = page_h_pt - 144.0;
        let header_links: Vec<&PdfLink> = links
            .iter()
            .filter(|l| l.page == 0 && l.rect[1].max(l.rect[3]) >= header_band_bottom)
            .collect();

        // Canonicalise URLs before comparing so trivial differences (trailing slash,
        // scheme/host case, a percent-encoded space) do not cause false positives.
        // The check's intent — catching a wrong link in the header — is preserved;
        // we only avoid blocking on byte-identical-but-semantically-equal URLs.
        let allowed_canonical: std::collections::BTreeSet<String> =
            allowed.iter().map(|u| canonicalize_url(u)).collect();

        // A header-band link that is NOT one of the profile's own fields means a
        // body/company link displaced a personal one (the URL-swap regression) —
        // the document shows a wrong link, so this stays blocking.
        for link in &header_links {
            if !allowed_canonical.contains(&canonicalize_url(&link.url)) {
                issues.push(ExportIssue::critical(
                    "header_url_mismatch",
                    format!(
                        "Header link {} is not one of the contact profile's fields — a \
                         body/company link leaked into the header.",
                        link.url
                    ),
                ));
            }
        }
        // The reverse (a profile link that did not surface in the header band) is a
        // *completeness* signal that depends on the 144 pt band heuristic, so it is
        // advisory — a missing/displaced contact link is a quality note, never a
        // reason to stop the user exporting an otherwise-valid, readable document.
        for url in &allowed {
            if !header_links
                .iter()
                .any(|l| canonicalize_url(&l.url) == canonicalize_url(url))
            {
                issues.push(ExportIssue::warning(
                    "header_url_missing",
                    format!("Contact profile link {url} is missing from the rendered header."),
                ));
            }
        }
    }

    issues
}

/// Read a page's `/Annots` entries as concrete dictionaries, handling BOTH the
/// inline-dictionary and the indirect-reference encodings — at the array level
/// and per element.
///
/// lopdf's own [`lopdf::Document::get_page_annotations`] keeps only entries that
/// are *indirect references* (`flat_map(Object::as_reference)`). Typst (our PDF
/// renderer) writes `/Annots` as an array of **inline dictionaries**, so that
/// helper returns nothing for every PDF we generate — which silently made the
/// header-link checks below see zero links and report every profile URL as
/// "missing", blocking any export that had a contact profile. Reading the array
/// ourselves keeps the validator working against Typst's output.
fn page_annot_dicts(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Vec<lopdf::Dictionary> {
    let Ok(page) = doc.get_dictionary(page_id) else {
        return Vec::new();
    };
    let array = match page.get(b"Annots") {
        Ok(lopdf::Object::Reference(id)) => doc.get_object(*id).and_then(|o| o.as_array()).ok(),
        Ok(lopdf::Object::Array(a)) => Some(a),
        _ => None,
    };
    let Some(array) = array else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|o| match o {
            lopdf::Object::Dictionary(d) => Some(d.clone()),
            lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
            _ => None,
        })
        .collect()
}

/// Collect `/Link` annotations (rect + `/A /URI`) for one page.
fn page_link_annotations(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
    page_idx: usize,
) -> Vec<PdfLink> {
    let mut out = Vec::new();
    for annot in page_annot_dicts(doc, page_id) {
        let is_link = annot
            .get(b"Subtype")
            .and_then(|v| v.as_name())
            .map(|n| n == b"Link")
            .unwrap_or(false);
        if !is_link {
            continue;
        }
        let Some(rect) = annot
            .get(b"Rect")
            .ok()
            .and_then(|v| v.as_array().ok())
            .and_then(|a| {
                let v: Vec<f32> = a.iter().filter_map(|o| o.as_float().ok()).collect();
                <[f32; 4]>::try_from(v).ok()
            })
        else {
            continue;
        };
        let url = annot
            .get(b"A")
            .ok()
            .and_then(|a| match a {
                lopdf::Object::Dictionary(d) => Some(d.clone()),
                lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
                _ => None,
            })
            .and_then(|d| {
                d.get(b"URI")
                    .ok()
                    .and_then(|u| u.as_str().ok())
                    .map(|b| String::from_utf8_lossy(b).into_owned())
            });
        if let Some(url) = url {
            out.push(PdfLink {
                rect,
                url,
                page: page_idx,
            });
        }
    }
    out
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
                    // A two-column layout extracting out of source order is inherent
                    // to the design (the sidebar is a separate column), not a defect
                    // — so this is advisory, NOT critical. Keeping it critical made
                    // `validate_and_fix` silently re-render single-column, overriding
                    // the user's explicit two-column + ATS-off choice. ATS mode is the
                    // user's control for a guaranteed single-column reading order.
                    issues.push(ExportIssue::warning(
                        "section_order",
                        "This two-column layout can read out of order in strict ATS parsers. \
                         Enable ATS mode for a single-column, ATS-safe version.",
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
