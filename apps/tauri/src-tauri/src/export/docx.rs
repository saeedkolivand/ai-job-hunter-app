use anyhow::{Context, Result};
use docx_rs::*;

use super::{
    parser::{parse_resume, strip_md},
    templates::{calculate_spacing, Template},
    types::{DocumentType, ExportRequest, GenerationMeta, LineKind, TextSegment},
};

/// Convert points to twentieths of a point (DOCX unit)
fn pt_to_dxa(pt: f32) -> usize {
    (pt * 20.0) as usize
}

/// Convert inches to twentieths of a point
fn inch_to_dxa(inch: f32) -> i32 {
    (inch * 1440.0) as i32
}

/// Convert RGB tuple to hex string
fn rgb_to_hex(rgb: (u8, u8, u8)) -> String {
    format!("{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2)
}

/// Create text runs from segments with bold formatting
fn create_runs(segments: &[TextSegment], font_size: usize, color: &str, bold_color: Option<&str>) -> Vec<Run> {
    segments
        .iter()
        .map(|seg| {
            let mut run = Run::new()
                .add_text(&seg.text)
                .size(font_size)
                .fonts(RunFonts::new().ascii("Calibri"));

            if seg.bold {
                run = run.bold();
                if let Some(bc) = bold_color {
                    run = run.color(bc);
                } else {
                    run = run.color(color);
                }
            } else {
                run = run.color(color);
            }

            run
        })
        .collect()
}

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

    // Colors
    let name_color = rgb_to_hex(template.name_color);
    let section_color = rgb_to_hex(template.section_color);
    let body_color = rgb_to_hex(template.body_color);
    let date_color = rgb_to_hex(template.date_color);
    let emphasis_color = rgb_to_hex(template.emphasis_color);
    let _rule_color = rgb_to_hex(template.rule_color);

    let mut name_written = false;
    let mut previous_kind: Option<LineKind> = None;

    for line in &parsed.lines {
        let spacing = calculate_spacing(&line.kind, previous_kind.as_ref());
        let _spacing_before = pt_to_dxa(spacing.0);
        let _spacing_after = pt_to_dxa(spacing.1);

        match line.kind {
            LineKind::Blank => {
                // Add small spacing
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(Run::new())
                );
            }

            LineKind::Name => {
                name_written = true;
                let name_text = meta
                    .and_then(|m| m.candidate_name.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or(&line.text);

                let mut para = Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(name_text)
                            .size(pt_to_dxa(template.name_pt))
                            .bold()
                            .color(&name_color)
                            .fonts(RunFonts::new().ascii("Calibri")),
                    );

                if template.name_centered {
                    para = para.align(AlignmentType::Center);
                }

                docx = docx.add_paragraph(para);
            }

            LineKind::Contact => {
                let mut para = Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&line.text)
                            .size(pt_to_dxa(9.0))
                            .color(&date_color)
                            .fonts(RunFonts::new().ascii("Calibri")),
                    );

                if template.name_centered {
                    para = para.align(AlignmentType::Center);
                }

                // Note: Paragraph borders removed - not available in docx-rs 0.4.20 Paragraph API
                // Use horizontal rule or table borders if needed

                docx = docx.add_paragraph(para);
                
                // Add spacing after contact
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(Run::new())
                );
            }

            LineKind::SectionHeader => {
                let header_text = if template.section_all_caps {
                    line.text.to_uppercase()
                } else {
                    line.text.clone()
                };

                let para = Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&header_text)
                            .size(pt_to_dxa(template.section_pt))
                            .bold()
                            .color(&section_color)
                            .fonts(RunFonts::new().ascii("Calibri"))
                            .character_spacing(if template.section_all_caps { 30 } else { 0 }),
                    );

                // Note: Section borders removed - not available in docx-rs 0.4.20 Paragraph API
                // Styling is handled via bold text and spacing
                let _ = &template.section_style; // suppress unused warning

                docx = docx.add_paragraph(para);
            }

            LineKind::JobEntry => {
                let runs = create_runs(
                    &line.segments,
                    pt_to_dxa(template.body_pt),
                    &body_color,
                    Some(&emphasis_color),
                );

                let mut para = Paragraph::new();

                // Add company name runs (bold)
                for run in runs {
                    para = para.add_run(run.bold());
                }

                // Add tab and date
                if let Some(date) = &line.right_text {
                    para = para
                        .add_run(Run::new().add_tab())
                        .add_run(
                            Run::new()
                                .add_text(date)
                                .size(pt_to_dxa(9.5))
                                .color(&date_color)
                                .fonts(RunFonts::new().ascii("Calibri")),
                        );
                }

                // Add tab stop for right-aligned date
                para = para.add_tab(
                    Tab::new()
                        .val(TabValueType::Right)
                        .pos(inch_to_dxa(6.27) as usize),
                );

                docx = docx.add_paragraph(para);
            }

            LineKind::JobTitle => {
                let runs = create_runs(
                    &line.segments,
                    pt_to_dxa(template.body_pt - 0.5),
                    &date_color,
                    Some(&emphasis_color),
                );

                let mut para = Paragraph::new();

                for run in runs {
                    para = para.add_run(run.italic());
                }

                docx = docx.add_paragraph(para);
            }

            LineKind::Bullet => {
                let runs = create_runs(
                    &line.segments,
                    pt_to_dxa(template.body_pt),
                    &body_color,
                    Some(&emphasis_color),
                );

                let mut para = Paragraph::new()
                    .indent(
                        Some(inch_to_dxa(0.2)),
                        Some(SpecialIndentType::Hanging(inch_to_dxa(0.2))),
                        None,
                        None,
                    );

                for run in runs {
                    para = para.add_run(run);
                }

                // Add bullet numbering
                para = para.numbering(NumberingId::new(1), IndentLevel::new(0));

                docx = docx.add_paragraph(para);
            }

            LineKind::Text => {
                let runs = create_runs(
                    &line.segments,
                    pt_to_dxa(template.body_pt),
                    &body_color,
                    Some(&emphasis_color),
                );

                let mut para = Paragraph::new();

                for run in runs {
                    para = para.add_run(run);
                }

                docx = docx.add_paragraph(para);
            }
        }

        previous_kind = Some(line.kind);
    }

    // Add name at the beginning if not written
    if !name_written {
        if let Some(meta) = meta {
            if let Some(name) = &meta.candidate_name {
                let _para = Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(name)
                            .size(pt_to_dxa(template.name_pt))
                            .bold()
                            .color(&name_color)
                            .fonts(RunFonts::new().ascii("Calibri")),
                    )
;

                // Insert at beginning (would need to rebuild, so we'll skip for now)
            }
        }
    }

    // Add bullet numbering definition
    let abstract_num = AbstractNumbering::new(1)
        .add_level(
            Level::new(
                0,
                Start::new(1),
                NumberFormat::new("bullet"),
                LevelText::new("•"),
                LevelJc::new("left"),
            )
            .indent(Some(inch_to_dxa(0.2)), None, None, None),
        );

    let num = Numbering::new(1, 1);

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

    // Colors
    let name_color = rgb_to_hex(template.name_color);
    let body_color = rgb_to_hex(template.body_color);
    let date_color = rgb_to_hex(template.date_color);
    let emphasis_color = rgb_to_hex(template.emphasis_color);

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
                            .color(&name_color)
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
                            .color(&date_color)
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
                            .color(&body_color)
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
                            .color(&body_color)
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
                            .color(&date_color)
                            .fonts(RunFonts::new().ascii("Calibri")),
                    ),
            );
            continue;
        }

        // Body paragraphs with inline bold
        let runs = create_runs(
            &segments,
            pt_to_dxa(template.body_pt + 0.5),
            &body_color,
            Some(&emphasis_color),
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
