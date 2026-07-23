use anyhow::{Context, Result};
use docx_rs::*;

use super::{
    parser::strip_md,
    templates::Template,
    types::{DocumentType, ExportRequest, GenerationMeta, LetterLayout},
};
use crate::locale::LocaleProfile;

use super::docx_renderer::*;

/// Page size (in DOCX `dxa`) for the active locale. Defaults to the `en` profile
/// (A4), keeping DOCX on the same page geometry as the PDF backends. Set
/// explicitly rather than relying on the docx-rs default so the source of truth
/// is `LocaleProfile`/`PageGeometry`; per-request locale sizing arrives in a
/// later phase.
fn page_size_dxa() -> (u32, u32) {
    let geom = LocaleProfile::default().page_geometry();
    (mm_to_dxa(geom.width_mm), mm_to_dxa(geom.height_mm))
}

// ─── Cover letter DOCX ────────────────────────────────────────────────────────

/// Dispatch to the requested [`LetterLayout`]'s DOCX renderer. `Classic` keeps
/// calling the original, unmodified renderer (`_classic`) so its output stays
/// byte-identical to the pre-layout-picker DOCX — the Refined/Banded arm is
/// entirely new code, never touched by a Classic request.
fn generate_cover_letter_docx(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
    market: &str,
    layout: LetterLayout,
) -> Result<Docx> {
    match layout {
        LetterLayout::Classic => {
            generate_cover_letter_docx_classic(text, meta, template, contact, lang)
        }
        LetterLayout::Refined | LetterLayout::Banded => {
            generate_cover_letter_docx_layout(text, meta, template, contact, lang, market, layout)
        }
    }
}

/// The original cover-letter DOCX renderer (pre-PR5). Unmodified — this is the
/// `LetterLayout::Classic` path, kept byte-identical.
fn generate_cover_letter_docx_classic(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
) -> Result<Docx> {
    let mut docx = Docx::new();

    // Named contact profile is the source of truth for the header contact line
    // (shared with the résumé), emitted as real hyperlinks. When present, the
    // scraped contact lines from the generated text are skipped.
    let profile_contact_md: Option<String> = contact
        .filter(|p| !p.is_effectively_empty())
        .map(|p| p.header_markdown(lang));

    let page_margin = PageMargin::new()
        .top(inch_to_dxa(1.0))
        .bottom(inch_to_dxa(1.0))
        .left(inch_to_dxa(template.margin_in + 0.15))
        .right(inch_to_dxa(template.margin_in + 0.15));

    let colors = setup_colors(template);
    let body_family = template.fonts.body_family;
    let name_family = template.fonts.name_family;

    let lines: Vec<&str> = text.lines().collect();
    let mut header_done = false;
    let mut in_body = false;

    for raw_line in &lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            // Blank lines — DOCX flow handles spacing; skip blank para emission
            continue;
        }

        let clean = strip_md(trimmed);
        let _segments = super::parser::parse_inline_md(trimmed);

        // Locale-aware: recognize salutations/sign-offs across every supported
        // market (was English/German only).
        let is_salutation = crate::locale::letter::is_salutation(&clean);
        let is_signoff = crate::locale::letter::is_signoff(&clean);

        // First line is name — but ONLY a real letterhead name, not a letter that
        // opens straight at the salutation. Without this guard a letterhead-less
        // letter's "Dear …" was consumed as the name (replaced with the candidate
        // name), the salutation arm never ran, `in_body` was never set, and the
        // whole body rendered in the muted addressee style.
        if !header_done && docx.document.children.is_empty() && !is_salutation && !is_signoff {
            let name_text = meta
                .and_then(|m| m.candidate_name.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(&clean);

            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(name_text)
                            .size(pt_to_dxa(template.name_pt))
                            .bold()
                            .color(&colors.name)
                            .fonts(docx_run_fonts(name_family)),
                    )
                    .line_spacing(LineSpacing::new().after(60)),
            );
            // Emit the profile-derived contact line right after the name, in place
            // of the scraped contact lines. `render_contact_line` runs `split_urls`,
            // so `[LinkedIn](url)` markdown and bare emails become real hyperlinks.
            if let Some(md) = &profile_contact_md {
                docx = docx.add_paragraph(
                    super::docx_renderer::render_contact_line(md, template, &colors)
                        .line_spacing(LineSpacing::new().after(40)),
                );
            }
            continue;
        }

        // Contact/address lines
        if !header_done
            && (clean.contains('@')
                || clean.contains('|')
                || clean.contains('·')
                || clean.chars().filter(|c| c.is_numeric()).count() > 5)
        {
            // Profile is the source of truth — drop the scraped contact line.
            if profile_contact_md.is_some() {
                continue;
            }
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_dxa(9.0))
                            .color(&colors.date)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().after(40)),
            );
            continue;
        }

        // Salutation
        if is_salutation {
            header_done = true;
            in_body = true;
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_dxa(template.body_pt))
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().before(160).after(160)),
            );
            continue;
        }

        // Signoff
        if is_signoff {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_dxa(template.body_pt))
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().before(240).after(480)),
            );
            continue;
        }

        // Subject line (Betreff/Objet/Oggetto/…) — bold, before the salutation.
        if !in_body && crate::locale::letter::is_subject_line(&clean) {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_dxa(template.body_pt))
                            .bold()
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().before(120).after(120)),
            );
            continue;
        }

        // Addressee block (before salutation)
        if !in_body {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_dxa(template.body_pt))
                            .color(&colors.date)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().after(40)),
            );
            continue;
        }

        // Body paragraphs — use proper spacing via pPr, no blank-paragraph spacers
        let para = render_cover_letter_paragraph(&clean, template, &colors, body_family);
        docx = docx.add_paragraph(para);
    }

    let (page_w, page_h) = page_size_dxa();
    docx = docx.page_size(page_w, page_h).page_margin(page_margin);
    Ok(docx)
}

// ─── Refined / Banded cover-letter DOCX (PR5) ──────────────────────────────────
//
// Shares the Classic renderer's line-by-line scan (same salutation/signoff/
// subject/date-ish detection — DOCX has no structured `LetterModel`, so this
// stays the single source of truth for that classification) but restyles each
// recognized zone per the chosen layout. Word/docx-rs has no angled-polygon or
// width-limited-rule primitive, so several `.typ` arrangement details are
// **documented approximations** here rather than faithful reproductions:
//
// - Refined's full-width horizontal rule under the header → a paragraph
//   BOTTOM BORDER (docx-rs borders are always full-paragraph-width, which
//   already matches "full-width" — no approximation needed there).
// - Refined's role line reads the `.typ`'s parsed `signature_title` (only
//   known after the sign-off, in a single forward pass over the model). DOCX's
//   flat scanner doesn't have that lookahead, so it uses `meta.job_title` —
//   the immediately-available equivalent — as the role text instead.
// - Banded's angled accent-tint polygon is NOT reproducible in DOCX at all;
//   approximated as a full-width PARAGRAPH SHADING band (lightened accent)
//   behind the (uppercased) name paragraph.
// - Banded's short (~28%-width) rule footer → docx-rs paragraph borders can't
//   be width-limited without a table, so it's approximated as a full-width
//   bottom border on the final paragraph.
fn generate_cover_letter_docx_layout(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
    market: &str,
    layout: LetterLayout,
) -> Result<Docx> {
    debug_assert!(
        !matches!(layout, LetterLayout::Classic),
        "generate_cover_letter_docx_layout is only for Refined/Banded"
    );
    let is_refined = matches!(layout, LetterLayout::Refined);

    let mut docx = Docx::new();

    let profile_contact_md: Option<String> = contact
        .filter(|p| !p.is_effectively_empty())
        .map(|p| p.header_markdown(lang));

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
    let band_hex = rgb_to_hex(lighten_rgb(template.accent_color, 0.85));

    // Market convention — consulted here only for Refined's reference-line
    // label, so it matches the same market convention `letter_refined.typ`
    // reads from `data.opts.subject_line_label` (DOCX's scanner is otherwise
    // market-agnostic, same as Classic).
    let subj_label = crate::locale::letter::conventions(market)
        .subject_line
        .label
        .clone();

    // A single full-width bottom-border rule, built fresh per call (docx-rs's
    // `ParagraphBorders::default()` pre-fills all four sides — `with_empty()`
    // avoids drawing an unwanted box).
    let bottom_rule = |color: &str, size: usize| {
        ParagraphBorders::with_empty().set(
            ParagraphBorder::new(ParagraphBorderPosition::Bottom)
                .val(BorderType::Single)
                .size(size)
                .color(color),
        )
    };

    let lines: Vec<&str> = text.lines().collect();
    let mut header_done = false;
    let mut in_body = false;

    for raw_line in &lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let clean = strip_md(trimmed);

        let is_salutation = crate::locale::letter::is_salutation(&clean);
        let is_signoff = crate::locale::letter::is_signoff(&clean);

        // First line is name — but ONLY a real letterhead name, not a letter that
        // opens straight at the salutation. Without this guard a letterhead-less
        // letter's "Dear …" was consumed as the name (replaced with the candidate
        // name), the salutation arm never ran, `in_body` was never set, and the
        // whole body rendered in the muted addressee style.
        if !header_done && docx.document.children.is_empty() && !is_salutation && !is_signoff {
            let name_text = meta
                .and_then(|m| m.candidate_name.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(&clean);
            // Banded: uppercase name — the same small-caps→uppercase precedent
            // `render_section_header` uses for the résumé DOCX path.
            let display_name = if is_refined {
                name_text.to_string()
            } else {
                name_text.to_uppercase()
            };
            let name_pt = if is_refined {
                template.name_pt + 4.0
            } else {
                template.name_pt
            };

            let mut name_para = Paragraph::new()
                .add_run(
                    Run::new()
                        .add_text(&display_name)
                        .size(pt_to_dxa(name_pt))
                        .bold()
                        .color(&colors.name)
                        .fonts(docx_run_fonts(name_family)),
                )
                .line_spacing(LineSpacing::new().after(60));
            if !is_refined {
                // Banded band approximation — see module-level doc comment.
                name_para.property = name_para.property.shading(
                    Shading::new()
                        .shd_type(ShdType::Clear)
                        .color("auto")
                        .fill(&band_hex),
                );
            }
            docx = docx.add_paragraph(name_para);

            // Refined: role line right after the name (see approximation note
            // above re: `meta.job_title` vs the `.typ`'s `signature_title`).
            if is_refined {
                if let Some(job_title) = meta.and_then(|m| m.job_title.as_deref()) {
                    docx = docx.add_paragraph(
                        Paragraph::new()
                            .add_run(
                                Run::new()
                                    .add_text(job_title.to_uppercase())
                                    .size(pt_to_dxa(template.body_pt))
                                    .color(&accent_hex)
                                    .character_spacing(24)
                                    .fonts(docx_run_fonts(body_family)),
                            )
                            .line_spacing(LineSpacing::new().after(40)),
                    );
                }
            }

            if let Some(md) = &profile_contact_md {
                let mut contact_para =
                    super::docx_renderer::render_contact_line(md, template, &colors)
                        .align(AlignmentType::Right)
                        .line_spacing(LineSpacing::new().after(if is_refined { 80 } else { 40 }));
                if is_refined {
                    // Full-width rule under the header — see approximation note.
                    contact_para.property =
                        contact_para.property.set_borders(bottom_rule(&rule_hex, 6));
                }
                docx = docx.add_paragraph(contact_para);
            }
            continue;
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
                .size(pt_to_dxa(9.0))
                .color(&colors.date)
                .fonts(docx_run_fonts(body_family));
            if !is_refined {
                // Banded bolds date/address-ish lines, mirroring
                // `letter_banded.typ`'s bold `emit-date-block`.
                run = run.bold();
            }
            let mut para = Paragraph::new()
                .add_run(run)
                .align(AlignmentType::Right)
                .line_spacing(LineSpacing::new().after(40));
            if is_refined {
                para.property = para.property.set_borders(bottom_rule(&rule_hex, 6));
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
                            .size(pt_to_dxa(template.body_pt))
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
                            .size(pt_to_dxa(template.body_pt))
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().before(240).after(if is_refined {
                        40
                    } else {
                        480
                    })),
            );
            if is_refined {
                // Extra empty paragraphs to leave room for a handwritten
                // signature — approximates the `.typ`'s `#v(34pt)` gap
                // (docx-rs has no direct vertical-space primitive outside
                // paragraph spacing/empty paragraphs).
                for _ in 0..2 {
                    docx = docx.add_paragraph(
                        Paragraph::new()
                            .add_run(Run::new().add_text("").size(pt_to_dxa(template.body_pt))),
                    );
                }
            }
            continue;
        }

        // Subject / reference line (Betreff/Objet/Re/…) — before the salutation.
        if !in_body && crate::locale::letter::is_subject_line(&clean) {
            if is_refined {
                // The always-on JOB REFERENCE line: caption suppressed when the
                // subject already opens with the market's own label or "Re:" —
                // same content rule as `letter_refined.typ`'s
                // `strip-subject-label` + `has-own-label` check.
                let subj_body = strip_market_label(&clean, &subj_label);
                let subj_body_lower = subj_body.to_lowercase();
                let has_own_label = (!subj_label.is_empty()
                    && subj_body_lower.starts_with(&subj_label.to_lowercase()))
                    || subj_body_lower.starts_with("re:");

                if !has_own_label {
                    let caption = if subj_label.is_empty() {
                        "Subject".to_string()
                    } else {
                        subj_label.clone()
                    };
                    docx = docx.add_paragraph(
                        Paragraph::new()
                            .add_run(
                                Run::new()
                                    .add_text(caption.to_uppercase())
                                    .size(pt_to_dxa((template.body_pt - 1.5).max(6.0)))
                                    .bold()
                                    .color(&accent_hex)
                                    .character_spacing(24)
                                    .fonts(docx_run_fonts(body_family)),
                            )
                            .line_spacing(LineSpacing::new().after(20)),
                    );
                }
                docx = docx.add_paragraph(
                    Paragraph::new()
                        .add_run(
                            Run::new()
                                .add_text(&subj_body)
                                .size(pt_to_dxa(template.body_pt))
                                .bold()
                                .color(&colors.body)
                                .fonts(docx_run_fonts(body_family)),
                        )
                        .line_spacing(LineSpacing::new().before(120).after(120)),
                );
            } else {
                // Banded: bold + accent color, unprocessed text — `.typ` has no
                // caption/suppression logic for this layout, only Refined does.
                docx = docx.add_paragraph(
                    Paragraph::new()
                        .add_run(
                            Run::new()
                                .add_text(&clean)
                                .size(pt_to_dxa(template.body_pt))
                                .bold()
                                .color(&accent_hex)
                                .fonts(docx_run_fonts(body_family)),
                        )
                        .line_spacing(LineSpacing::new().before(120).after(120)),
                );
            }
            continue;
        }

        // Addressee block (before salutation).
        if !in_body {
            let mut run = Run::new()
                .add_text(&clean)
                .size(pt_to_dxa(template.body_pt))
                .color(&colors.date)
                .fonts(docx_run_fonts(body_family));
            if !is_refined {
                // Banded bolds the recipient block, mirroring
                // `letter_banded.typ`'s bold `emit-recipient-block`.
                run = run.bold();
            }
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(run)
                    .line_spacing(LineSpacing::new().after(40)),
            );
            continue;
        }

        // Body paragraphs — unchanged in both layouts.
        let para = render_cover_letter_paragraph(&clean, template, &colors, body_family);
        docx = docx.add_paragraph(para);
    }

    if !is_refined {
        // Banded's short rule footer — see approximation note above.
        let mut footer = Paragraph::new()
            .add_run(Run::new().add_text("").size(pt_to_dxa(template.body_pt)))
            .line_spacing(LineSpacing::new().before(160));
        footer.property = footer.property.set_borders(bottom_rule(&accent_hex, 18));
        docx = docx.add_paragraph(footer);
    }

    let (page_w, page_h) = page_size_dxa();
    docx = docx.page_size(page_w, page_h).page_margin(page_margin);
    Ok(docx)
}

/// Lighten an RGB colour toward white by `amount` (0.0..=1.0), mirroring
/// Typst's `color.lighten(pct)` used for `letter_banded.typ`'s band tint, so
/// the DOCX approximation matches the same pale accent the PDF renders.
fn lighten_rgb(rgb: (u8, u8, u8), amount: f32) -> (u8, u8, u8) {
    let blend = |c: u8| -> u8 {
        let c = c as f32;
        (c + (255.0 - c) * amount).round().clamp(0.0, 255.0) as u8
    };
    (blend(rgb.0), blend(rgb.1), blend(rgb.2))
}

/// Strip a leading "`<label>`[:]" prefix from a subject line (case-insensitive),
/// mirroring `letter_refined.typ`'s `strip-subject-label`. Labels are ASCII, so
/// slicing by the label's byte length removes exactly the prefix.
fn strip_market_label(s: &str, label: &str) -> String {
    let t = s.trim();
    if !label.is_empty() && t.to_lowercase().starts_with(&label.to_lowercase()) {
        let rest = t[label.len().min(t.len())..].trim_start();
        let rest = rest.strip_prefix(':').unwrap_or(rest).trim_start();
        rest.to_string()
    } else {
        t.to_string()
    }
}

// ─── Extract section helper ───────────────────────────────────────────────────

fn extract_section<'a>(text: &'a str, start_marker: &str, end_marker: Option<&str>) -> &'a str {
    let start = if let Some(idx) = text.find(start_marker) {
        let after = &text[idx + start_marker.len()..];
        after
            .find('\n')
            .map(|i| idx + start_marker.len() + i + 1)
            .unwrap_or(idx + start_marker.len())
    } else {
        return text;
    };
    let end = if let Some(em) = end_marker {
        text[start..]
            .find(em)
            .map(|i| start + i)
            .unwrap_or(text.len())
    } else {
        text.len()
    };
    text[start..end].trim()
}

// ─── Public entry point ───────────────────────────────────────────────────────

pub fn generate_docx(request: &ExportRequest) -> Result<Vec<u8>> {
    // Document accent (ADR 0004): recolor the template's accent-derived fields
    // when a valid override is present. `setup_colors` reads `emphasis_color`, so
    // the accent surfaces on emphasized runs for both the résumé and cover-letter
    // DOCX paths (both derive from this `template`). No-op when absent/malformed.
    let template =
        Template::get(request.template_id).with_accent_override(request.accent.as_deref());

    // The DOCX path collapses two-column templates to a single column since
    // DOCX doesn't replicate the sidebar layout.
    let single_column = || {
        let mut t = template.clone();
        if crate::theme::is_two_column(request.template_id) {
            t.two_column = None;
            t.margin_in = 1.0;
        }
        t
    };

    let docx = match request.document_type {
        DocumentType::Resume => {
            let text = extract_section(
                &request.text,
                "### CANDIDATE RESUME ###",
                Some("### JOB ADVERTISEMENT ###"),
            );
            let text = if text.is_empty() {
                request.text.as_str()
            } else {
                text
            };
            // model_docx is the sole résumé-DOCX path. The legacy flat-parser arm
            // has been removed — it was only reachable with `--no-default-features`
            // and diverged from the model path.
            crate::export::model_docx::generate_resume_docx_in(
                text,
                request.meta.as_ref(),
                &template,
                request.ats_mode,
                request.page_geometry(),
                request.contact.as_ref(),
                &request.target_lang(),
            )
            .context("Failed to generate resume DOCX")?
        }
        DocumentType::CoverLetter => {
            let text = extract_section(&request.text, "### COMPLETE COVER LETTER ###", None);
            let text = if text.is_empty() {
                request.text.as_str()
            } else {
                text
            };
            // `market` drives only the Refined reference-line label (see
            // `generate_cover_letter_docx_layout`'s doc comment); mirrors the
            // PDF cover-letter path's `market` computation in `pdf/mod.rs`.
            let market = request.locale.as_deref().unwrap_or("intl");
            generate_cover_letter_docx(
                text,
                request.meta.as_ref(),
                &single_column(),
                request.contact.as_ref(),
                &request.target_lang(),
                market,
                request.letter_layout,
            )
            .context("Failed to generate cover letter DOCX")?
        }
    };

    let mut buffer = std::io::Cursor::new(Vec::new());
    docx.build()
        .pack(&mut buffer)
        .context("Failed to pack DOCX")?;
    Ok(buffer.into_inner())
}

#[cfg(test)]
mod test;
