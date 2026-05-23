use anyhow::{Context, Result};
use docx_rs::*;

use super::{
    parser::{parse_resume, strip_md},
    templates::{calculate_spacing, Template},
    types::{DocumentType, ExportRequest, GenerationMeta, LineKind},
};

use super::docx_renderer::*;

/// Generate DOCX for resume
fn generate_resume_docx(text: &str, meta: Option<&GenerationMeta>, template: &Template) -> Result<Docx> {
    let parsed = parse_resume(text);
    let mut docx = Docx::new();

    // Page setup
    let page_margin = PageMargin::new()
        .top(inch_to_dxa(0.9))
        .bottom(inch_to_dxa(0.9))
        .left(inch_to_dxa(template.margin_in))
        .right(inch_to_dxa(template.margin_in));

    // Setup colors
    let colors = setup_colors(template);

    let mut name_written = false;
    let mut previous_kind: Option<LineKind> = None;

    for line in &parsed.lines {
        let spacing = calculate_spacing(&line.kind, previous_kind.as_ref());
        let _spacing_before = pt_to_dxa(spacing.0);
        let _spacing_after = pt_to_dxa(spacing.1);

        match line.kind {
            LineKind::Blank => {
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(Run::new())
                );
            }

            LineKind::Name => {
                name_written = true;
                let para = render_name_line(&line.text, meta, template, &colors);
                docx = docx.add_paragraph(para);
            }

            LineKind::Contact => {
                let para = render_contact_line(&line.text, template, &colors);
                docx = docx.add_paragraph(para);
                // Add spacing after contact
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(Run::new())
                );
            }

            LineKind::SectionHeader => {
                let para = render_section_header(&line.text, template, &colors);
                docx = docx.add_paragraph(para);
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
                docx = docx.add_paragraph(para);
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

    // Add name at the beginning if not written
    if !name_written {
        if let Some(meta) = meta {
            if let Some(name) = &meta.candidate_name {
                let _para = render_name_line(name, Some(meta), template, &colors);
                // Insert at beginning (would need to rebuild, so we'll skip for now)
            }
        }
    }

    // Add bullet numbering definition
    let (abstract_num, num) = create_bullet_numbering();

    docx = docx
        .add_abstract_numbering(abstract_num)
        .add_numbering(num)
        .page_margin(page_margin);

    Ok(docx)
}

/// Generate DOCX for cover letter
fn generate_cover_letter_docx(text: &str, meta: Option<&GenerationMeta>, template: &Template) -> Result<Docx> {
    let mut docx = Docx::new();

    // Page setup with slightly larger margins
    let page_margin = PageMargin::new()
        .top(inch_to_dxa(1.0))
        .bottom(inch_to_dxa(1.0))
        .left(inch_to_dxa(template.margin_in + 0.15))
        .right(inch_to_dxa(template.margin_in + 0.15));

    // Setup colors
    let colors = setup_colors(template);

    let lines: Vec<&str> = text.lines().collect();
    let mut header_done = false;
    let mut in_body = false;

    for raw_line in lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(Run::new())
            );
            continue;
        }

        let clean = strip_md(trimmed);
        let segments = super::parser::parse_inline_md(trimmed);

        // Detect salutation and signoff
        let is_salutation = clean.starts_with("Dear") || clean.starts_with("Sehr geehrte");
        let is_signoff = clean.starts_with("Kind regards")
            || clean.starts_with("Sincerely")
            || clean.starts_with("Best regards")
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
                            .size(pt_to_dxa(template.name_pt - 2.0))
                            .bold()
                            .color(&colors.name)
                            .fonts(RunFonts::new().ascii("Calibri")),
                    ),
            );
            continue;
        }

        // Contact/address lines
        if !header_done && (clean.contains('@') || clean.contains('|') || clean.chars().filter(|c| c.is_numeric()).count() > 5) {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_dxa(template.body_pt - 1.0))
                            .color(&colors.date)
                            .fonts(RunFonts::new().ascii("Calibri")),
                    ),
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
                            .size(pt_to_dxa(template.body_pt + 0.5))
                            .bold()
                            .color(&colors.body)
                            .fonts(RunFonts::new().ascii("Calibri")),
                    ),
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
                            .fonts(RunFonts::new().ascii("Calibri")),
                    ),
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
                            .size(pt_to_dxa(template.body_pt - 1.0))
                            .color(&colors.date)
                            .fonts(RunFonts::new().ascii("Calibri")),
                    ),
            );
            continue;
        }

        // Body paragraphs with inline bold
        let runs = create_runs(
            &segments,
            pt_to_dxa(template.body_pt + 0.5),
            &colors.body,
            Some(&colors.emphasis),
        );

        let mut para = Paragraph::new();

        for run in runs {
            para = para.add_run(run);
        }

        docx = docx.add_paragraph(para);
    }

    docx = docx.page_margin(page_margin);

    Ok(docx)
}

/// Extract a section from the full AI output between two markers
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

/// Main export function
pub fn generate_docx(request: &ExportRequest) -> Result<Vec<u8>> {
    let template = Template::get(request.template_id);

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

    // Convert to bytes
    let mut buffer = std::io::Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).context("Failed to pack DOCX")?;
    Ok(buffer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::types::TemplateId;

    #[test]
    fn test_generate_simple_resume() {
        let request = ExportRequest {
            text: "John Doe\njohn@example.com\n\nEXPERIENCE\nSoftware Engineer  2020-2023".to_string(),
            format: super::super::types::ExportFormat::Docx,
            document_type: DocumentType::Resume,
            template_id: TemplateId::Modern,
            meta: None,
        };

        let result = generate_docx(&request);
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }
}
