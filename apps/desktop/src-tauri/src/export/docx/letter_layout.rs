//! Refined / Banded / Navy / Sidebar / Monogram cover-letter DOCX (PR5).
//!
//! Shares the Classic renderer's line-by-line scan (same salutation/signoff/
//! subject/date-ish detection — DOCX has no structured `LetterModel`, so this
//! stays the single source of truth for that classification) but restyles each
//! recognized zone per the chosen layout. Word/docx-rs has no angled-polygon or
//! width-limited-rule primitive, so several `.typ` arrangement details are
//! **documented approximations** here rather than faithful reproductions:
//!
//! - Refined's full-width horizontal rule under the header → a paragraph
//!   BOTTOM BORDER (docx-rs borders are always full-paragraph-width, which
//!   already matches "full-width" — no approximation needed there).
//! - Refined's role line reads the `.typ`'s parsed `signature_title` (only
//!   known after the sign-off, in a single forward pass over the model). DOCX's
//!   flat scanner doesn't have that lookahead, so it uses `meta.job_title` —
//!   the immediately-available equivalent — as the role text instead.
//! - Banded's angled accent-tint polygon is NOT reproducible in DOCX at all;
//!   approximated as a full-width PARAGRAPH SHADING band (lightened accent)
//!   behind the (uppercased) name paragraph.
//! - Banded's short (~28%-width) rule footer → docx-rs paragraph borders can't
//!   be width-limited without a table, so it's approximated as a full-width
//!   bottom border on the final paragraph.
//! - Sidebar's tinted full-height LEFT MARGIN rail is not expressible in DOCX at
//!   all (no margin-anchored frame that ATS parse safely, and a text box or
//!   two-column table would be exactly the multi-column trap the whole export
//!   avoids). Approximated the same way Banded's band is: paragraph SHADING in
//!   the same `band_tint_hex` accent tint behind the (left-aligned) name
//!   paragraph, with the contact stacked under it at the left margin rather than
//!   pulled right. Same tint, same words, one column.
//! - Monogram's initials device (a pale square before the name lockup) becomes a
//!   shaded RUN carrying the same initials at the head of the name paragraph, so
//!   both formats extract the identical "JS Jane Smith" — a separate shaded
//!   paragraph would have put the initials on their own line, which the PDF
//!   never does.
//!
//! ATS mode: every tint above is decorative and is suppressed when the request
//! sets `ats_mode`, mirroring the `.typ` side's `data.opts.ats` gate so the two
//! formats degrade together instead of one keeping a band the other dropped.
//!
//! Paragraph builders for the header-name/title and subject-line zones live in
//! [`super::letter_layout_blocks`]; this file keeps the per-line scanning loop
//! that ties every zone together.

use anyhow::Result;
use docx_rs::*;

use super::super::{
    docx_renderer::*,
    parser::strip_md,
    templates::Template,
    types::{GenerationMeta, LetterRender},
};
use super::letter_layout_blocks::{
    render_layout_header_name, render_layout_subject_lines, HeaderLook,
};
use super::letter_style::{band_tint_hex, LetterDocxStyle};
use super::page_size_dxa;

pub(super) fn generate_cover_letter_docx_layout(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    req: LetterRender<'_>,
) -> Result<Docx> {
    debug_assert!(
        !matches!(req.layout, crate::export::types::LetterLayout::Classic),
        "generate_cover_letter_docx_layout serves every non-Classic layout"
    );
    let sty = LetterDocxStyle::for_layout(req.layout);
    // Decorative tints, dropped together with the `.typ` side's under ATS mode.
    // Derived once here rather than `&& !ats` at each call site, so a future
    // decoration cannot be added to only one of them.
    let show_band = sty.header_band && !req.ats;
    let show_device = sty.monogram_device && !req.ats;

    let mut docx = Docx::new();

    let profile_contact_md: Option<String> = contact
        .filter(|p| !p.is_effectively_empty())
        .map(|p| p.header_markdown(req.lang));

    let page_margin = PageMargin::new()
        .top(inch_to_dxa(1.0))
        .bottom(inch_to_dxa(1.0))
        .left(inch_to_dxa(template.margin_in + 0.15))
        .right(inch_to_dxa(template.margin_in + 0.15));

    let colors = setup_colors(template);
    let body_family = template.fonts.body_family;
    let name_family = template.fonts.name_family;
    let accent_hex = rgb_to_hex(template.accent_color);
    let rule_hex = rgb_to_hex(template.rule_color);
    let band_hex = band_tint_hex(template.accent_color);

    // Market convention — consulted here only for Refined's reference-line
    // label, so it matches the same market convention `letter_refined.typ`
    // reads from `data.opts.subject_line_label` (DOCX's scanner is otherwise
    // market-agnostic, same as Classic).
    let subj_label = crate::locale::letter::conventions(req.market)
        .subject_line
        .label
        .clone();

    let lines: Vec<&str> = text.lines().collect();
    let mut header_done = false;
    let mut in_body = false;
    // Tracks "have we processed the first non-blank line yet" — NOT the same
    // as `docx.document.children.is_empty()`, which the header block used to
    // rely on. That check silently broke once contact could be emitted
    // without a name: a contact-only header still adds a paragraph, so the
    // "first line" gate would have gone false starting on line 2 for the
    // WRONG reason.
    let mut header_line_seen = false;

    for raw_line in &lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let clean = strip_md(trimmed);

        let is_salutation = crate::locale::letter::is_salutation(&clean);
        let is_signoff = crate::locale::letter::is_signoff(&clean);
        let is_subject_line = crate::locale::letter::is_subject_line(&clean);

        let is_first_line = !header_line_seen;
        header_line_seen = true;

        // First line is name — but ONLY a real letterhead name, not a letter
        // that opens straight at the salutation, sign-off, or subject/
        // reference line (e.g. German "Betreff: …"), and not a date either
        // ("12 March 2025", the fourth opening kind) — shared with the PDF
        // parser's identical guard via `is_letterhead_name` (see its doc
        // comment) so the two formats can never disagree about which
        // openings are not names.
        if is_first_line && !header_done {
            let is_header_name_line = !is_salutation && !is_signoff && !is_subject_line;
            // Shared with the PDF parser via `resolve_letterhead_candidate`:
            // an empty-string `candidate_name: Some("")` must fall through to
            // `clean` exactly like `None` does. `meta...unwrap_or(&clean)`
            // alone does NOT do that — `Some("")` is not `None`, so the plain
            // `unwrap_or` chain returned `""` and suppressed a REAL name
            // sitting right there on the letter's own first line.
            let name_text = crate::export::typst_engine::resolve_letterhead_candidate(
                meta.and_then(|m| m.candidate_name.as_deref()),
                || clean.as_str(),
            );
            let renders_name =
                is_header_name_line && crate::export::typst_engine::is_letterhead_name(name_text);

            if renders_name {
                let look = HeaderLook {
                    colors: &colors,
                    name_family,
                    body_family,
                    accent_hex: &accent_hex,
                    band_hex: &band_hex,
                    show_device,
                    show_band,
                };
                for p in render_layout_header_name(name_text, meta, &sty, template, &look) {
                    docx = docx.add_paragraph(p);
                }
            }

            // Contact renders INDEPENDENT of whether the name did — it comes
            // from a separately-attached `ContactProfile`, not from parsing
            // this line, exactly like the PDF parser's `contact_md` (built
            // unconditionally, before any line is classified). Nesting this
            // inside the name branch was a regression a review round caught:
            // a nameless, date/salutation-opening letter with a real profile
            // attached lost the user's contact info entirely, not just the
            // fabricated name — strictly worse than the pre-guard behaviour,
            // where a garbage name at least kept the contact line alive.
            if let Some(md) = &profile_contact_md {
                let mut contact_para =
                    super::super::docx_renderer::render_contact_line(md, template, &colors)
                        .align(sty.contact_align)
                        .line_spacing(LineSpacing::new().after(sty.contact_space_after));
                if sty.header_rule {
                    // Full-width rule under the header — see approximation
                    // note. For Navy this is the letterhead rule, which is
                    // why it sits here (after name + title + contact)
                    // rather than on the name — and still runs when there is
                    // no name, closing off a contact-only header the same
                    // way.
                    contact_para.property = contact_para
                        .property
                        .set_borders(bottom_rule_border(&rule_hex, 6));
                }
                docx = docx.add_paragraph(contact_para);
            }

            if renders_name {
                continue;
            }
            // else: this line (a date, the salutation itself, a subject
            // line, or any other non-name opening) was NOT consumed as a
            // header — fall through so the block below (contact/address) or
            // a later branch (salutation/subject/body) classifies it
            // normally.
        }

        // Contact/address lines (only reached when no ContactProfile was
        // supplied — mirrors Classic's fallback).
        if !header_done
            && (clean.contains('@')
                || clean.contains('|')
                || clean.contains('·')
                || clean.chars().filter(|c| c.is_numeric()).count() > 5)
        {
            if profile_contact_md.is_some() {
                continue;
            }
            let mut run = Run::new()
                .add_text(&clean)
                .size(pt_to_half_points(9.0))
                .color(&colors.date)
                .fonts(docx_run_fonts(body_family));
            if sty.bolds_addressing {
                // Banded bolds date/address-ish lines, mirroring
                // `letter_banded.typ`'s bold `emit-date-block`. Navy does not —
                // its `emit-date-block` is regular weight.
                run = run.bold();
            }
            // Same style source as the profile-backed contact path above. These
            // two used to disagree: this fallback right-aligned Navy with no
            // rule while the other centred it and ruled it, so the SAME letter
            // rendered differently depending on whether a ContactProfile
            // happened to be attached.
            let mut para = Paragraph::new()
                .add_run(run)
                .align(sty.contact_align)
                .line_spacing(LineSpacing::new().after(40));
            if sty.header_rule {
                para.property = para.property.set_borders(bottom_rule_border(&rule_hex, 6));
            }
            docx = docx.add_paragraph(para);
            continue;
        }

        // Salutation — unchanged styling in both layouts (mirrors `.typ`,
        // which never bolds or recolors the salutation).
        if is_salutation {
            header_done = true;
            in_body = true;
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_half_points(template.body_pt))
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().before(160).after(160)),
            );
            continue;
        }

        // Sign-off.
        if is_signoff {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_half_points(template.body_pt))
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(
                        LineSpacing::new()
                            .before(240)
                            .after(sty.signoff_space_after),
                    ),
            );
            if sty.signature_gap {
                // Extra empty paragraphs to leave room for a handwritten
                // signature — approximates the `.typ`'s `#v(34pt)` gap
                // (docx-rs has no direct vertical-space primitive outside
                // paragraph spacing/empty paragraphs).
                for _ in 0..2 {
                    docx = docx.add_paragraph(
                        Paragraph::new().add_run(
                            Run::new()
                                .add_text("")
                                .size(pt_to_half_points(template.body_pt)),
                        ),
                    );
                }
            }
            continue;
        }

        // Subject / reference line (Betreff/Objet/Re/…) — before the salutation.
        if !in_body && is_subject_line {
            for p in render_layout_subject_lines(
                &clean,
                &sty,
                template,
                &colors,
                body_family,
                &accent_hex,
                &subj_label,
            ) {
                docx = docx.add_paragraph(p);
            }
            continue;
        }

        // Addressee block (before salutation).
        if !in_body {
            let mut run = Run::new()
                .add_text(&clean)
                .size(pt_to_half_points(template.body_pt))
                .color(&colors.date)
                .fonts(docx_run_fonts(body_family));
            if sty.bolds_addressing {
                // Banded bolds the recipient block, mirroring
                // `letter_banded.typ`'s bold `emit-recipient-block`. Navy's is
                // regular weight.
                run = run.bold();
            }
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(run)
                    .line_spacing(LineSpacing::new().after(40)),
            );
            continue;
        }

        // Body paragraphs — unchanged in both layouts. `trimmed`, not `clean` —
        // see the identical fix + comment in `generate_cover_letter_docx_classic`
        // above; `clean` has already had every `**` stripped, which silently
        // dropped bold formatting in every non-Classic letter layout too.
        let para = render_cover_letter_paragraph(trimmed, template, &colors, body_family);
        docx = docx.add_paragraph(para);
    }

    if sty.footer_rule {
        // Banded's short rule footer — see approximation note above. Navy's
        // rule is under the letterhead, not at the foot, so it is excluded.
        let mut footer = Paragraph::new()
            .add_run(
                Run::new()
                    .add_text("")
                    .size(pt_to_half_points(template.body_pt)),
            )
            .line_spacing(LineSpacing::new().before(160));
        footer.property = footer
            .property
            .set_borders(bottom_rule_border(&accent_hex, 18));
        docx = docx.add_paragraph(footer);
    }

    let (page_w, page_h) = page_size_dxa();
    docx = docx.page_size(page_w, page_h).page_margin(page_margin);
    Ok(docx)
}

/// A single full-width bottom-border rule (docx-rs's `ParagraphBorders::default()`
/// pre-fills all four sides — `with_empty()` avoids drawing an unwanted box).
fn bottom_rule_border(color: &str, size: usize) -> ParagraphBorders {
    ParagraphBorders::with_empty().set(
        ParagraphBorder::new(ParagraphBorderPosition::Bottom)
            .val(BorderType::Single)
            .size(size)
            .color(color),
    )
}

/// Strip a leading "`<label>`[:]" prefix from a subject line (case-insensitive),
/// mirroring `letter_refined.typ`'s `strip-subject-label`. Labels are ASCII, so
/// slicing by the label's byte length removes exactly the prefix.
pub(super) fn strip_market_label(s: &str, label: &str) -> String {
    let t = s.trim();
    if !label.is_empty() && t.to_lowercase().starts_with(&label.to_lowercase()) {
        let rest = t[label.len().min(t.len())..].trim_start();
        let rest = rest.strip_prefix(':').unwrap_or(rest).trim_start();
        rest.to_string()
    } else {
        t.to_string()
    }
}
