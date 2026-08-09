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

/// The `n` links sitting highest on the page, ordered top-down.
///
/// Selection is **geometric**, never `/Annots` emission order.
/// `page_link_annotations` yields annotations in array order, which carries no
/// guarantee about vertical position — and for a two-column template it very
/// likely doesn't, since a sidebar is emitted as its own run. Taking the first
/// `n` in emission order could therefore pick a *body* link, raise a CRITICAL
/// `header_url_mismatch`, and make `validate_and_fix` silently re-render the
/// document single-column — losing the user's chosen layout to a false
/// positive. Sorting by the same `rect` top edge the band filter already reads
/// makes "the header's own n links" a statement about the page, not about the
/// writer that produced it.
///
/// Pinned by `validate::tests::topmost_n_orders_by_geometry_not_annotation_order`.
fn topmost_n<'a>(links: &[&'a PdfLink], n: usize) -> Vec<&'a PdfLink> {
    let top = |l: &PdfLink| l.rect[1].max(l.rect[3]);
    let mut by_position: Vec<&'a PdfLink> = links.to_vec();
    by_position.sort_by(|a, b| top(b).total_cmp(&top(a)));
    by_position.truncate(n);
    by_position
}

/// Render-level checks over the actual PDF bytes (renderer-agnostic, so the modern
/// résumé engine and the legacy cover-letter path are both covered):
///   * `header_url_mismatch` (critical) — a self-consistency check: every
///     link at the header's OWN position must be one of the document's own,
///     actually-authoritative header links (catches a company-link
///     displacing a personal profile / site, or a render/model
///     desynchronization). For a résumé this is whichever side (text or
///     profile) actually supplied the header — not only the profile;
///     narrowed to the header's own expected link COUNT (not the whole
///     144pt band) so a genuine, correctly-placed body link near the top of
///     page 1 is never mistaken for a header link. For a cover letter it is
///     always the profile, narrowed the SAME way (the topmost
///     `allowed.len()` band links): the letterhead (name/contact) always
///     renders before date/recipient/salutation/body, so its own links are
///     always the topmost ones — but a markdown link inside the OPENING
///     PARAGRAPH (e.g. a company-research URL the AI echoes in its first
///     sentence) can still land inside the same 144pt band as a short
///     letterhead, and a cover letter has no `can_linearize` remediation for
///     a false block (never two-column).
///   * `header_url_missing` (warning) — the reverse completeness check (a
///     profile link that did not surface anywhere in the full header band,
///     only checked when the profile is what supplied the header); advisory
///     only, as it leans on the band heuristic and a missing link never
///     corrupts the document.
///   * `header_url_job_board` (warning) — a job-board/ATS host anywhere in
///     the full header band, whoever put it there, résumé or cover letter;
///     exempts a personal Xing profile.
fn pdf_render_issues(request: &ExportRequest, bytes: &[u8]) -> Vec<ExportIssue> {
    let doc = match lopdf::Document::load_mem(bytes) {
        Ok(d) => d,
        Err(e) => {
            // Tooling failure never blocks — debug only, this is diagnostic
            // noise a valid export will never hit.
            log::debug!("export: lopdf failed to parse rendered PDF bytes: {e}");
            return Vec::new();
        }
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

    // A self-consistency check against the header ACTUALLY rendered — not a
    // profile-parity check, and not skipped for any document shape (see ADR
    // 0021's "Export validation" section for the full rationale/history).
    //
    // H — the editor is the source of truth: `ContactProfile::apply_to_header`
    // fills the header's contact line from the profile ONLY when the
    // text-derived header has none. `allowed` below is built from whichever
    // header runs are actually authoritative for THIS render (reconstructed
    // by running the SAME `model_from_resume_text` → `apply_to_header`
    // pipeline `prepare_resume_render` uses), not only the profile's fields —
    // so the strict block runs unconditionally for résumés, text-owned
    // header or not, profile supplied or not. Cover letters are unaffected
    // (their header override wasn't part of H) and keep the original,
    // profile-only form.
    //
    // Parses the SAME text `prepare_resume_render` actually renders from, not
    // the raw `request.text` — which can still carry the
    // "### CANDIDATE RESUME ###" / "### JOB ADVERTISEMENT ###" wrapper. Left
    // un-extracted, `parse_resume` reads that marker as an ATX heading at line
    // 0 (`strip_atx_heading` runs before the idx==0 name/contact case),
    // `seen_section` flips true immediately, and the real name/contact lines
    // never reach `header.contact` at all.
    let resume_text = (request.document_type == DocumentType::Resume).then(|| {
        let extracted = crate::export::pdf::extract_section(
            &request.text,
            "### CANDIDATE RESUME ###",
            Some("### JOB ADVERTISEMENT ###"),
        );
        if extracted.is_empty() {
            request.text.as_str()
        } else {
            extracted
        }
    });
    // `(profile_is_header_source, header)` — the boolean is recorded from the
    // text-only parse, BEFORE the profile fallback runs, so it still answers
    // "did the profile actually supply this header's contact line" for the
    // completeness/"missing" check below (only a meaningful signal relative
    // to a profile that was actually applied).
    let resume_header = resume_text.map(|t| {
        let mut model = crate::model::adapter::model_from_resume_text(t);
        let profile_is_header_source = model.header.contact.is_empty();
        if let Some(profile) = request.contact.as_ref() {
            profile.apply_to_header(&mut model.header, &request.target_lang());
        }
        (profile_is_header_source, model.header)
    });

    // Header region: the top ~2 inches (144 pt) of the first page. A
    // HEURISTIC, not a semantic boundary — a short header followed
    // immediately by a section (EXPERIENCE, say) can put a genuine, correctly
    // placed BODY link (a job's own company site) geometrically inside this
    // band. `header_links` (every band-falling annotation) still backs the
    // advisory completeness/job-board checks below, where a false positive
    // only produces a non-blocking warning; the BLOCKING mismatch check uses
    // `header_owned_links` instead (narrower — see there).
    let header_band_bottom = page_h_pt - 144.0;
    let header_links: Vec<&PdfLink> = links
        .iter()
        .filter(|l| l.page == 0 && l.rect[1].max(l.rect[3]) >= header_band_bottom)
        .collect();

    match &resume_header {
        Some((profile_is_header_source, header)) => {
            // `allowed`: the URLs the document's own, actually-authoritative
            // header claims — the reconstructed header's own link runs,
            // whichever side (text or profile) supplied them.
            let allowed: std::collections::BTreeSet<String> = header
                .contact
                .iter()
                .filter_map(|r| r.link.clone())
                .collect();
            let allowed_canonical: std::collections::BTreeSet<String> =
                allowed.iter().map(|u| canonicalize_url(u)).collect();

            // CodeRabbit (security re-review): the mismatch check below must
            // NOT run over the full `header_links` band — the band is a
            // geometric heuristic (see above), and a genuine body link
            // rendering early on the page (a short header + an immediate
            // section) is not "the header's" just because it falls inside
            // the same 144 pt zone; flagging it there is exactly the
            // false-block this check must never produce. Narrowed to
            // `header_owned_links`: the FIRST `allowed.len()` band links, in
            // the PDF's own annotation order — which tracks top-to-bottom
            // render order for a normal single-column document, so this is
            // "the header's own N links, whichever N are actually
            // expected," not "everything that happens to be nearby." A
            // document whose header renders fewer links than expected (the
            // band clipped a wrapped line) simply checks fewer — never more
            // than are legitimately the header's.
            //
            // A SECOND reviewer (post-push, round 8) read `take(allowed.len())`
            // silently degrading to zero checks when `allowed` is empty as an
            // unintentional hole and proposed falling back to the FULL band
            // in that case. Verified empirically before writing this: that
            // fallback is wrong and would reintroduce the exact false-block
            // this round removed — a name-only header with no profile and no
            // email/phone-with-a-link produces `allowed = {}` (a phone/
            // location line never gets a `.link` run; a bare name never
            // does either), and a job's own company link rendering early on
            // the page (short header, immediate EXPERIENCE section) would be
            // flagged as `header_url_mismatch` and BLOCK a perfectly valid
            // export, on a fresh un-narrowed-band check identical in kind to
            // the one this round already fixed.
            //
            // INVARIANT, made explicit rather than left as an accident of
            // `take(0)`: an empty `allowed` means the reconstructed header —
            // the one the renderer will actually emit — owns NO links at
            // all. `header.contact`'s runs carry zero `.link` values (no
            // email, no LinkedIn/GitHub/Website, no extra link), and
            // `header.name` is a plain `String` that can never carry one
            // either. There is therefore nothing legitimately "the
            // header's" to check a band link against; every link in the
            // band in this state is body content, full stop, and must not
            // block. Pinned by `validate::tests::linkless_header_with_a_genuine_body_link_in_the_band_does_not_false_block`.
            if !allowed.is_empty() {
                let header_owned_links = topmost_n(&header_links, allowed.len());

                // A header-owned link that is NOT one of the header's own
                // claimed links means the render itself disagrees with the
                // reconstructed model (the URL-swap regression: the document
                // shows a wrong link where its own header should be) — this
                // stays blocking, unconditionally (no profile required).
                for link in &header_owned_links {
                    if !allowed_canonical.contains(&canonicalize_url(&link.url)) {
                        issues.push(ExportIssue::critical(
                            "header_url_mismatch",
                            format!(
                                "Header link {} is not one of the document header's own links \
                                 — a body/company link leaked into the header.",
                                link.url
                            ),
                        ));
                    }
                }
            }

            // The reverse (a profile link that did not surface in the header
            // band) is a *completeness* signal that depends on the 144 pt
            // band heuristic, so it is advisory — checked against the FULL
            // `header_links` (not narrowed: "did this expected link render
            // somewhere in the band at all" is a different question than
            // "is this band link one of the header's own") — and only
            // meaningful when the profile actually supplied this header
            // (comparing an unrelated profile's links against a text-owned
            // header would be a false, unactionable "missing" for every one
            // of them).
            if *profile_is_header_source {
                if let Some(profile) = request
                    .contact
                    .as_ref()
                    .filter(|p| !p.is_effectively_empty())
                {
                    for url in profile.header_urls() {
                        if !header_links
                            .iter()
                            .any(|l| canonicalize_url(&l.url) == canonicalize_url(&url))
                        {
                            issues.push(ExportIssue::warning(
                                "header_url_missing",
                                format!(
                                    "Contact profile link {url} is missing from the rendered \
                                     header."
                                ),
                            ));
                        }
                    }
                }
            }
        }
        None => {
            // Cover letter: H doesn't apply — the header always comes
            // straight from the profile (there is no text-derived header to
            // prefer), so this keeps the original, simpler profile-parity
            // form. The strict checks genuinely need a profile to compare
            // against — no profile (or an empty one) means nothing to check.
            //
            // Narrowed the SAME way the résumé arm is (`topmost_n`) — a
            // production incident (a macOS user's PDF export blocked with
            // no remediation) traced back to this arm running unnarrowed
            // over the full band. The original reasoning here ("no body
            // SECTION can render early into a cover letter's header-band
            // the way a résumé's can") is true but incomplete: it rules out
            // a body *section* racing the header, not a body *link* inside
            // the opening paragraph itself. `parse_cover_letter` parses the
            // letterhead first and the body last (letterhead → date →
            // recipient → salutation → body, in that order), so the
            // letterhead's own links always render ABOVE the body — the
            // same "topmost N are the header's own" invariant the résumé
            // arm already relies on holds here too. A markdown link the AI
            // echoes in the opening sentence (e.g. from a company-research
            // brief) can still land inside the same 144pt band as a short
            // letterhead (no date/recipient lines to push it down), and a
            // cover letter is never two-column — `can_linearize` is always
            // false, so a false block here had NO remediation for the user.
            // Pinned by
            // `validate::tests::cover_letter_body_link_in_a_short_letterhead_band_does_not_false_block`.
            if let Some(profile) = request
                .contact
                .as_ref()
                .filter(|p| !p.is_effectively_empty())
            {
                let allowed: std::collections::BTreeSet<String> =
                    profile.header_urls().into_iter().collect();
                let allowed_canonical: std::collections::BTreeSet<String> =
                    allowed.iter().map(|u| canonicalize_url(u)).collect();

                // Same empty-`allowed` invariant as the résumé arm: a
                // profile can be non-empty (e.g. phone/location only, which
                // `header_urls` never turns into a link) yet supply zero
                // links, in which case nothing in the band is legitimately
                // "the letterhead's" and every link there is body content.
                if !allowed.is_empty() {
                    let header_owned_links = topmost_n(&header_links, allowed.len());
                    for link in &header_owned_links {
                        if !allowed_canonical.contains(&canonicalize_url(&link.url)) {
                            issues.push(ExportIssue::critical(
                                "header_url_mismatch",
                                format!(
                                    "Header link {} is not one of the contact profile's fields \
                                     — a body/company link leaked into the header.",
                                    link.url
                                ),
                            ));
                        }
                    }
                }
                for url in &allowed {
                    if !header_links
                        .iter()
                        .any(|l| canonicalize_url(&l.url) == canonicalize_url(url))
                    {
                        issues.push(ExportIssue::warning(
                            "header_url_missing",
                            format!(
                                "Contact profile link {url} is missing from the rendered header."
                            ),
                        ));
                    }
                }
            }
        }
    }

    // A job-board/ATS host is never a legitimate personal contact link,
    // whoever put it there and whichever document type this is — checked
    // unconditionally: not gated on a profile being present (the people most
    // likely to export a raw imported header untouched are exactly the ones
    // who never filled one in), and now hoisted above the résumé/cover-letter
    // match (LOW, security re-review — it used to run only in the résumé
    // arm, silently never firing for a cover letter's own job-board host,
    // contradicting this warning's "unconditional" design intent). Runs
    // against the FULL band, same as the missing-completeness check: a
    // job-board URL that IS part of the header's own claimed links (the
    // user's own header literally contains one) passes the mismatch check
    // above by construction but must still warn here. Exempts a personal
    // Xing profile (`/profile/…`) — `xing.com` is also a job-board host (it
    // lists job postings too), so without the exemption a legitimate DACH
    // candidate's own profile link would warn every time.
    for link in &header_links {
        if crate::contact_profile::is_job_board(&link.url)
            && !crate::contact_profile::is_personal_xing(&link.url)
        {
            issues.push(ExportIssue::warning(
                "header_url_job_board",
                format!(
                    "Header link {} looks like a job board/ATS URL, not a personal contact link.",
                    link.url
                ),
            ));
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
                    // Same PDF-text-string decode as the extractor: a UTF-16
                    // `/URI` read as UTF-8 is mojibake that no content check
                    // could match. Single decoder so the two can't drift.
                    .map(crate::extraction::pdf::pdf_text_string)
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

/// Deterministic content validation of GENERATED text (facts, alignment,
/// language, voice) — the earlier sibling of this module's rendered-bytes gate.
pub mod content;
