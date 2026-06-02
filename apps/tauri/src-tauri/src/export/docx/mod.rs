use anyhow::{Context, Result};
use docx_rs::*;

use super::{
    parser::{parse_resume, strip_md},
    templates::{calculate_spacing, Template},
    types::{DocumentType, ExportRequest, GenerationMeta, LineKind, TemplateId},
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

// ─── Resume DOCX ──────────────────────────────────────────────────────────────

fn generate_resume_docx(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
) -> Result<Docx> {
    let parsed = parse_resume(text);
    let mut docx = Docx::new();

    let page_margin = PageMargin::new()
        .top(inch_to_dxa(0.9))
        .bottom(inch_to_dxa(0.9))
        .left(inch_to_dxa(template.margin_in))
        .right(inch_to_dxa(template.margin_in));

    let colors = setup_colors(template);

    let mut name_written = false;
    let mut previous_kind: Option<LineKind> = None;

    for line in &parsed.lines {
        let spacing = calculate_spacing(&line.kind, previous_kind.as_ref());
        let _spacing_before = pt_to_dxa(spacing.0);
        let _spacing_after = pt_to_dxa(spacing.1);

        match line.kind {
            LineKind::Blank => {
                // Blank lines in DOCX handled by paragraph spacing — skip emitting empty paras
            }

            LineKind::Name => {
                name_written = true;
                let para = render_name_line(&line.text, meta, template, &colors);
                docx = docx.add_paragraph(para);
            }

            LineKind::Contact => {
                let para = render_contact_line(&line.text, template, &colors);
                docx = docx.add_paragraph(para.line_spacing(LineSpacing::new().after(120)));
            }

            LineKind::SectionHeader => {
                let para = render_section_header(&line.text, template, &colors);
                docx = docx.add_paragraph(
                    para.line_spacing(
                        LineSpacing::new()
                            .before(pt_to_dxa(template.section_spacing_before) as u32)
                            .after(60),
                    ),
                );
            }

            LineKind::JobEntry => {
                let para = render_job_entry(
                    &line.segments,
                    line.right_text.as_deref(),
                    template,
                    &colors,
                );
                docx = docx.add_paragraph(para);
            }

            LineKind::JobTitle => {
                let para = render_job_title(&line.segments, template, &colors);
                docx = docx.add_paragraph(para.line_spacing(LineSpacing::new().after(60)));
            }

            LineKind::Bullet => {
                let para = render_bullet_line(&line.segments, template, &colors);
                docx = docx.add_paragraph(para);
            }

            LineKind::Text => {
                let para = render_text_line(&line.segments, template, &colors);
                docx = docx.add_paragraph(para);
            }
        }

        previous_kind = Some(line.kind);
    }

    if !name_written {
        if let Some(meta) = meta {
            if let Some(_name) = &meta.candidate_name {
                // Name would be prepended — skip for now (requires rebuild)
            }
        }
    }

    let (abstract_num, num) = create_bullet_numbering();

    let (page_w, page_h) = page_size_dxa();
    docx = docx
        .add_abstract_numbering(abstract_num)
        .add_numbering(num)
        .page_size(page_w, page_h)
        .page_margin(page_margin);

    Ok(docx)
}

// ─── Cover letter DOCX ────────────────────────────────────────────────────────

fn generate_cover_letter_docx(
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

        // First line is name
        if !header_done && docx.document.children.is_empty() {
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
    let template = Template::get(request.template_id);

    // The legacy DOCX path can't lay out columns, so it collapses two-column
    // templates to a single column. The model path renders a real two-column
    // table, so it keeps the config.
    let single_column = || {
        let mut t = template.clone();
        if matches!(request.template_id, TemplateId::TwoColumn) {
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
            // Strangler-fig switch: the canonical model backend renders resume DOCX
            // by default; `--no-default-features` falls back to the legacy renderer.
            // Both arms compile via `cfg!` so neither path rots.
            if cfg!(feature = "model_docx") {
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
            } else {
                generate_resume_docx(text, request.meta.as_ref(), &single_column())
                    .context("Failed to generate resume DOCX")?
            }
        }
        DocumentType::CoverLetter => {
            let text = extract_section(&request.text, "### COMPLETE COVER LETTER ###", None);
            let text = if text.is_empty() {
                request.text.as_str()
            } else {
                text
            };
            generate_cover_letter_docx(
                text,
                request.meta.as_ref(),
                &single_column(),
                request.contact.as_ref(),
                &request.target_lang(),
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
