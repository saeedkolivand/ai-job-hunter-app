//! Text-level readback checks: what the re-extracted text of an export must
//! still say about the source document (name, email, section presence and
//! order, stray Markdown).

use crate::export::parser::{parse_resume, strip_md};
use crate::export::types::{DocumentType, ExportRequest, LineKind};

use super::{ExportIssue, EMAIL_RE};

/// Critical content expected to survive a round trip, parsed from the source.
pub(super) struct Expected {
    pub(super) name: Option<String>,
    pub(super) email: Option<String>,
    /// Section headings in source order (empty for cover letters).
    pub(super) headings: Vec<String>,
}

pub(super) fn expected_from_request(request: &ExportRequest) -> Expected {
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

    // The candidate name from metadata is a FALLBACK ONLY (H: the editor is
    // the source of truth) — mirrors `export/pdf/mod.rs`'s and
    // `export/model_docx.rs`'s own precedence (both only fill `header.name`
    // from `meta.candidate_name` when the text-derived name is blank). An
    // unconditional override here made this expectation diverge from what
    // actually renders whenever the two differ (a stale `meta.candidate_name`
    // vs. an edited header) — the real, rendered document correctly shows
    // the text's own name, but this function still "expected" the stale
    // metadata one, firing a spurious `missing_name` warning against a
    // correctly-rendered document.
    let name_is_blank = name.as_deref().map(str::trim).unwrap_or("").is_empty();
    if name_is_blank {
        if let Some(meta_name) = request
            .meta
            .as_ref()
            .and_then(|m| m.candidate_name.as_deref())
        {
            if !meta_name.trim().is_empty() {
                name = Some(meta_name.to_string());
            }
        }
    }

    let email = EMAIL_RE.find(&request.text).map(|m| m.as_str().to_string());

    Expected {
        name,
        email,
        headings,
    }
}

/// Reject stray Markdown emphasis (`*`, backtick) that survived sanitization into
/// the rendered text — the leaked-asterisk symptom. Applies to PDF and DOCX.
///
/// Only flags `*` or `` ` `` that appear at emphasis-boundary positions (not
/// flanked by an ASCII word character on both sides). A `*` between two word
/// chars (e.g. `5*4`, `a*b`) is a literal value preserved by the sanitizer and
/// must not be treated as a rendering defect.
pub(super) fn stray_markdown_issues(extracted: &str) -> Vec<ExportIssue> {
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

/// Compare the expected content against the re-extracted text.
pub(super) fn evaluate(
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
                // A two-column layout extracting out of source order is inherent
                // to the design (the sidebar is a separate column), not a defect
                // — so this is advisory, NOT critical. Keeping it critical made
                // `validate_and_fix` silently re-render single-column, overriding
                // the user's explicit two-column + ATS-off choice. ATS mode is the
                // user's control for a guaranteed single-column reading order.
                let message = if two_column {
                    "This two-column layout can read out of order in strict ATS parsers. \
                     Enable ATS mode for a single-column, ATS-safe version."
                } else {
                    "Sections appear out of order when re-reading the exported file."
                };
                issues.push(ExportIssue::warning("section_order", message));
            }
        }
    }

    issues
}

/// Lowercased, whitespace-collapsed, alphanumeric-only form for tolerant
/// `contains` / ordering checks against imperfect extraction.
pub(super) fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
