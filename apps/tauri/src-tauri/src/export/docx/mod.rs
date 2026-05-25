use anyhow::{Context, Result};
use docx_rs::*;

use super::{
    parser::{parse_resume, strip_md},
    templates::{calculate_spacing, Template},
    types::{DocumentType, ExportRequest, GenerationMeta, LineKind, TemplateId},
};

use super::docx_renderer::*;

// ─── Resume DOCX ──────────────────────────────────────────────────────────────

fn generate_resume_docx(text: &str, meta: Option<&GenerationMeta>, template: &Template) -> Result<Docx> {
    let parsed = parse_resume(text);
    let mut docx = Docx::new();

    let page_margin = PageMargin::new()
        .top(inch_to_dxa(0.9))
        .bottom(inch_to_dxa(0.9))
        .left(inch_to_dxa(template.margin_in))
        .right(inch_to_dxa(template.margin_in));

    let colors = setup_colors(template);
    let _font_name = docx_font_name(template.fonts.body_family);

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
                docx = docx.add_paragraph(
                    para.line_spacing(LineSpacing::new().after(120))
                );
            }

            LineKind::SectionHeader => {
                let para = render_section_header(&line.text, template, &colors);
                docx = docx.add_paragraph(
                    para.line_spacing(
                        LineSpacing::new()
                            .before(pt_to_dxa(template.section_spacing_before) as u32)
                            .after(60),
                    )
                );
            }

            LineKind::JobEntry => {
                let para = render_job_entry(&line.segments, line.right_text.as_deref(), template, &colors);
                docx = docx.add_paragraph(para);
            }

            LineKind::JobTitle => {
                let para = render_job_title(&line.segments, template, &colors);
                docx = docx.add_paragraph(
                    para.line_spacing(LineSpacing::new().after(60))
                );
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

    docx = docx
        .add_abstract_numbering(abstract_num)
        .add_numbering(num)
        .page_margin(page_margin);

    Ok(docx)
}

// ─── Cover letter DOCX ────────────────────────────────────────────────────────

fn generate_cover_letter_docx(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
) -> Result<Docx> {
    let mut docx = Docx::new();

    let page_margin = PageMargin::new()
        .top(inch_to_dxa(1.0))
        .bottom(inch_to_dxa(1.0))
        .left(inch_to_dxa(template.margin_in + 0.15))
        .right(inch_to_dxa(template.margin_in + 0.15));

    let colors = setup_colors(template);
    let font_name = docx_font_name(template.fonts.body_family);
    let name_font = docx_font_name(template.fonts.name_family);

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

        let is_salutation = clean.starts_with("Dear") || clean.starts_with("Sehr geehrte");
        let is_signoff = clean.starts_with("Kind regards")
            || clean.starts_with("Sincerely")
            || clean.starts_with("Best regards")
            || clean.starts_with("Best,")
            || clean.starts_with("Regards")
            || clean.starts_with("Mit freundlichen");

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
                            .fonts(RunFonts::new().ascii(name_font)),
                    )
                    .line_spacing(LineSpacing::new().after(60)),
            );
            continue;
        }

        // Contact/address lines
        if !header_done && (clean.contains('@') || clean.contains('|') || clean.contains('·')
            || clean.chars().filter(|c| c.is_numeric()).count() > 5)
        {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_dxa(9.0))
                            .color(&colors.date)
                            .fonts(RunFonts::new().ascii(font_name)),
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
                            .fonts(RunFonts::new().ascii(font_name)),
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
                            .fonts(RunFonts::new().ascii(font_name)),
                    )
                    .line_spacing(LineSpacing::new().before(240).after(480)),
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
                            .fonts(RunFonts::new().ascii(font_name)),
                    )
                    .line_spacing(LineSpacing::new().after(40)),
            );
            continue;
        }

        // Body paragraphs — use proper spacing via pPr, no blank-paragraph spacers
        let para = render_cover_letter_paragraph(&clean, template, &colors, font_name);
        docx = docx.add_paragraph(para);
    }

    docx = docx.page_margin(page_margin);
    Ok(docx)
}

// ─── Extract section helper ───────────────────────────────────────────────────

fn extract_section<'a>(text: &'a str, start_marker: &str, end_marker: Option<&str>) -> &'a str {
    let start = if let Some(idx) = text.find(start_marker) {
        let after = &text[idx + start_marker.len()..];
        after.find('\n').map(|i| idx + start_marker.len() + i + 1).unwrap_or(idx + start_marker.len())
    } else {
        return text;
    };
    let end = if let Some(em) = end_marker {
        text[start..].find(em).map(|i| start + i).unwrap_or(text.len())
    } else {
        text.len()
    };
    text[start..end].trim()
}

// ─── Public entry point ───────────────────────────────────────────────────────

pub fn generate_docx(request: &ExportRequest) -> Result<Vec<u8>> {
    // Two-column is PDF-only — DOCX always linearizes to single column.
    let effective_id = request.template_id;
    let mut template = Template::get(effective_id);
    if matches!(effective_id, TemplateId::TwoColumn) {
        template.two_column = None;
        template.margin_in = 1.0;
    }

    let docx = match request.document_type {
        DocumentType::Resume => {
            let text = extract_section(&request.text, "### CANDIDATE RESUME ###", Some("### JOB ADVERTISEMENT ###"));
            let text = if text.is_empty() { request.text.as_str() } else { text };
            generate_resume_docx(text, request.meta.as_ref(), &template)
                .context("Failed to generate resume DOCX")?
        }
        DocumentType::CoverLetter => {
            let text = extract_section(&request.text, "### COMPLETE COVER LETTER ###", None);
            let text = if text.is_empty() { request.text.as_str() } else { text };
            generate_cover_letter_docx(text, request.meta.as_ref(), &template)
                .context("Failed to generate cover letter DOCX")?
        }
    };

    let mut buffer = std::io::Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).context("Failed to pack DOCX")?;
    Ok(buffer.into_inner())
}

#[cfg(test)]
mod test;
