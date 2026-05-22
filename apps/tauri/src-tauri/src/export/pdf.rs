use anyhow::{Context, Result};
use printpdf::*;
use std::io::BufWriter;

use super::{
    parser::{parse_resume, strip_md},
    templates::{calculate_spacing, Template},
    types::{DocumentType, ExportRequest, GenerationMeta, LineKind, TextSegment},
};

const MM_PER_INCH: f32 = 25.4;
const PT_PER_MM: f32 = 2.834645669;

/// Convert inches to millimeters
fn inch_to_mm(inch: f32) -> f32 {
    inch * MM_PER_INCH
}

/// Convert points to millimeters
fn pt_to_mm(pt: f32) -> f32 {
    pt / PT_PER_MM
}

/// Convert RGB tuple to printpdf Color
fn rgb_to_color(rgb: (u8, u8, u8)) -> Color {
    Color::Rgb(Rgb::new(
        rgb.0 as f32 / 255.0,
        rgb.1 as f32 / 255.0,
        rgb.2 as f32 / 255.0,
        None,
    ))
}

/// Draw text with mixed bold/normal formatting
fn draw_mixed_text(
    layer: &PdfLayerReference,
    segments: &[TextSegment],
    x: f32,
    mut y: f32,
    max_width: f32,
    line_height: f32,
    font_regular: &IndirectFontRef,
    font_bold: &IndirectFontRef,
    font_size: f32,
    normal_color: Color,
    bold_color: Color,
) -> f32 {
    let mut current_x = x;

    for segment in segments {
        let font = if segment.bold { font_bold } else { font_regular };
        let color = if segment.bold { bold_color } else { normal_color };

        // Split into words for wrapping
        let words: Vec<&str> = segment.text.split_whitespace().collect();

        for (i, word) in words.iter().enumerate() {
            let word_with_space = if i < words.len() - 1 {
                format!("{} ", word)
            } else {
                word.to_string()
            };

            // Estimate word width (rough approximation)
            let char_width = font_size * 0.5;
            let word_width = word_with_space.len() as f32 * char_width;

            // Check if word fits on current line
            if current_x > x && current_x + word_width > x + max_width {
                // Move to next line
                y -= line_height;
                current_x = x;
            }

            // Draw word
            layer.use_text(
                &word_with_space,
                font_size,
                Mm(current_x),
                Mm(y),
                font,
            );
            layer.set_fill_color(color.clone());

            current_x += word_width;
        }
    }

    y - line_height
}

/// Generate PDF for resume
fn generate_resume_pdf(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
) -> Result<PdfDocumentReference> {
    let parsed = parse_resume(text);

    // Create document
    let (doc, page1, layer1) = PdfDocument::new(
        "Resume",
        Mm(210.0), // A4 width
        Mm(297.0), // A4 height
        "Layer 1",
    );

    // Get fonts
    let font_data_regular = include_bytes!("../../fonts/Calibri-Regular.ttf");
    let font_data_bold = include_bytes!("../../fonts/Calibri-Bold.ttf");
    
    let font_regular = doc.add_external_font(font_data_regular.as_ref())
        .context("Failed to load regular font")?;
    let font_bold = doc.add_external_font(font_data_bold.as_ref())
        .context("Failed to load bold font")?;

    let current_layer = doc.get_page(page1).get_layer(layer1);

    // Margins
    let margin_left = inch_to_mm(template.margin_in);
    let margin_right = inch_to_mm(template.margin_in);
    let margin_top = inch_to_mm(0.9);
    let margin_bottom = inch_to_mm(0.9);

    let page_width = 210.0;
    let page_height = 297.0;
    let usable_width = page_width - margin_left - margin_right;

    let mut y = page_height - margin_top;
    let line_height = pt_to_mm(template.body_pt) * template.line_spacing;

    // Colors
    let name_color = rgb_to_color(template.name_color);
    let section_color = rgb_to_color(template.section_color);
    let body_color = rgb_to_color(template.body_color);
    let date_color = rgb_to_color(template.date_color);
    let emphasis_color = rgb_to_color(template.emphasis_color);
    let rule_color = rgb_to_color(template.rule_color);

    let mut previous_kind: Option<LineKind> = None;

    for line in &parsed.lines {
        let spacing = calculate_spacing(&line.kind, previous_kind.as_ref());
        y -= pt_to_mm(spacing.0);

        // Check if we need a new page
        if y < margin_bottom + line_height * 2.0 {
            let (page, layer) = doc.add_page(Mm(page_width), Mm(page_height), "Layer 1");
            let current_layer = doc.get_page(page).get_layer(layer);
            y = page_height - margin_top;
        }

        match line.kind {
            LineKind::Blank => {
                y -= pt_to_mm(3.0);
            }

            LineKind::Name => {
                let name_text = meta
                    .and_then(|m| m.candidate_name.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or(&line.text);

                let x = if template.name_centered {
                    page_width / 2.0 - (name_text.len() as f32 * pt_to_mm(template.name_pt) * 0.3)
                } else {
                    margin_left
                };

                current_layer.use_text(
                    name_text,
                    template.name_pt,
                    Mm(x),
                    Mm(y),
                    &font_bold,
                );
                current_layer.set_fill_color(name_color.clone());

                y -= pt_to_mm(template.name_pt) * 1.2;
            }

            LineKind::Contact => {
                let x = if template.name_centered {
                    page_width / 2.0 - (line.text.len() as f32 * pt_to_mm(9.0) * 0.3)
                } else {
                    margin_left
                };

                current_layer.use_text(&line.text, 9.0, Mm(x), Mm(y), &font_regular);
                current_layer.set_fill_color(date_color.clone());

                y -= pt_to_mm(9.0) * 1.2;

                // Draw line
                let line_obj = Line {
                    points: vec![
                        (Point::new(Mm(margin_left), Mm(y)), false),
                        (Point::new(Mm(page_width - margin_right), Mm(y)), false),
                    ],
                    is_closed: false,
                };
                current_layer.set_outline_color(rule_color.clone());
                current_layer.set_outline_thickness(0.5);
                current_layer.add_shape(line_obj);

                y -= pt_to_mm(5.0);
            }

            LineKind::SectionHeader => {
                y -= pt_to_mm(4.0);

                let header_text = if template.section_all_caps {
                    line.text.to_uppercase()
                } else {
                    line.text.clone()
                };

                current_layer.use_text(
                    &header_text,
                    template.section_pt,
                    Mm(margin_left),
                    Mm(y),
                    &font_bold,
                );
                current_layer.set_fill_color(section_color.clone());

                y -= pt_to_mm(template.section_pt) * 1.2;

                // Draw section line if needed
                match template.section_style {
                    super::templates::SectionStyle::RuledBottom => {
                        let line_obj = Line {
                            points: vec![
                                (Point::new(Mm(margin_left), Mm(y)), false),
                                (Point::new(Mm(page_width - margin_right), Mm(y)), false),
                            ],
                            is_closed: false,
                        };
                        current_layer.set_outline_thickness(1.0);
                        current_layer.add_shape(line_obj);
                        y -= pt_to_mm(2.0);
                    }
                    super::templates::SectionStyle::Underline => {
                        let line_obj = Line {
                            points: vec![
                                (Point::new(Mm(margin_left), Mm(y)), false),
                                (Point::new(Mm(page_width - margin_right), Mm(y)), false),
                            ],
                            is_closed: false,
                        };
                        current_layer.set_outline_thickness(0.5);
                        current_layer.add_shape(line_obj);
                        y -= pt_to_mm(2.0);
                    }
                    _ => {}
                }

                y -= pt_to_mm(4.0);
            }

            LineKind::JobEntry => {
                // Company name (bold)
                y = draw_mixed_text(
                    &current_layer,
                    &line.segments,
                    margin_left,
                    y,
                    usable_width * 0.7,
                    line_height,
                    &font_regular,
                    &font_bold,
                    template.body_pt,
                    body_color.clone(),
                    emphasis_color.clone(),
                );

                // Date on right
                if let Some(date) = &line.right_text {
                    let date_x = page_width - margin_right - (date.len() as f32 * pt_to_mm(9.0) * 0.3);
                    current_layer.use_text(date, 9.0, Mm(date_x), Mm(y + line_height), &font_regular);
                    current_layer.set_fill_color(date_color.clone());
                }

                y -= pt_to_mm(2.0);
            }

            LineKind::JobTitle => {
                current_layer.use_text(
                    &line.text,
                    template.body_pt - 0.5,
                    Mm(margin_left),
                    Mm(y),
                    &font_regular,
                );
                current_layer.set_fill_color(date_color.clone());

                y -= line_height;
            }

            LineKind::Bullet => {
                // Draw bullet point
                current_layer.use_text("•", template.body_pt, Mm(margin_left + 1.3), Mm(y), &font_regular);
                current_layer.set_fill_color(body_color.clone());

                // Draw text
                y = draw_mixed_text(
                    &current_layer,
                    &line.segments,
                    margin_left + 4.0,
                    y,
                    usable_width - 4.0,
                    line_height,
                    &font_regular,
                    &font_bold,
                    template.body_pt,
                    body_color.clone(),
                    emphasis_color.clone(),
                );
            }

            LineKind::Text => {
                y = draw_mixed_text(
                    &current_layer,
                    &line.segments,
                    margin_left,
                    y,
                    usable_width,
                    line_height + pt_to_mm(0.5),
                    &font_regular,
                    &font_bold,
                    template.body_pt,
                    body_color.clone(),
                    emphasis_color.clone(),
                );
            }
        }

        y -= pt_to_mm(spacing.1);
        previous_kind = Some(line.kind);
    }

    Ok(doc)
}

/// Generate PDF for cover letter
fn generate_cover_letter_pdf(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
) -> Result<PdfDocumentReference> {
    let (doc, page1, layer1) = PdfDocument::new(
        "Cover Letter",
        Mm(210.0),
        Mm(297.0),
        "Layer 1",
    );

    let font_data_regular = include_bytes!("../../fonts/Calibri-Regular.ttf");
    let font_data_bold = include_bytes!("../../fonts/Calibri-Bold.ttf");
    
    let font_regular = doc.add_external_font(font_data_regular.as_ref())
        .context("Failed to load regular font")?;
    let font_bold = doc.add_external_font(font_data_bold.as_ref())
        .context("Failed to load bold font")?;

    let current_layer = doc.get_page(page1).get_layer(layer1);

    let margin_left = inch_to_mm(template.margin_in + 0.15);
    let margin_right = inch_to_mm(template.margin_in + 0.15);
    let page_width = 210.0;
    let page_height = 297.0;
    let usable_width = page_width - margin_left - margin_right;

    let mut y = page_height - inch_to_mm(1.0);
    let line_height = pt_to_mm(template.body_pt) * template.line_spacing;

    let name_color = rgb_to_color(template.name_color);
    let body_color = rgb_to_color(template.body_color);
    let date_color = rgb_to_color(template.date_color);
    let emphasis_color = rgb_to_color(template.emphasis_color);

    let lines: Vec<&str> = text.lines().collect();
    let mut header_done = false;
    let mut in_body = false;

    for raw_line in lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            y -= line_height * 0.7;
            continue;
        }

        let clean = strip_md(trimmed);
        let segments = super::parser::parse_inline_md(trimmed);

        let is_salutation = clean.starts_with("Dear") || clean.starts_with("Sehr geehrte");
        let is_signoff = clean.starts_with("Kind regards") || clean.starts_with("Sincerely");

        // First line is name
        if !header_done && y == page_height - inch_to_mm(1.0) {
            let name_text = meta
                .and_then(|m| m.candidate_name.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(&clean);

            current_layer.use_text(
                name_text,
                template.name_pt - 2.0,
                Mm(margin_left),
                Mm(y),
                &font_bold,
            );
            current_layer.set_fill_color(name_color.clone());

            y -= pt_to_mm(template.name_pt - 2.0) * 1.2 + pt_to_mm(1.5);
            continue;
        }

        // Contact/address
        if !header_done && (clean.contains('@') || clean.contains('|')) {
            current_layer.use_text(&clean, template.body_pt, Mm(margin_left), Mm(y), &font_regular);
            current_layer.set_fill_color(date_color.clone());
            y -= line_height + pt_to_mm(1.0);
            continue;
        }

        // Salutation
        if is_salutation {
            header_done = true;
            in_body = true;
            current_layer.use_text(&clean, template.body_pt + 0.5, Mm(margin_left), Mm(y), &font_bold);
            current_layer.set_fill_color(body_color.clone());
            y -= line_height + pt_to_mm(3.0);
            continue;
        }

        // Signoff
        if is_signoff {
            y -= pt_to_mm(5.0);
            current_layer.use_text(&clean, template.body_pt + 0.5, Mm(margin_left), Mm(y), &font_regular);
            current_layer.set_fill_color(body_color.clone());
            y -= line_height + pt_to_mm(12.0);
            continue;
        }

        // Addressee block
        if !in_body {
            current_layer.use_text(&clean, template.body_pt, Mm(margin_left), Mm(y), &font_regular);
            current_layer.set_fill_color(date_color.clone());
            y -= line_height + pt_to_mm(1.0);
            continue;
        }

        // Body paragraphs
        y = draw_mixed_text(
            &current_layer,
            &segments,
            margin_left,
            y,
            usable_width,
            line_height + pt_to_mm(1.0),
            &font_regular,
            &font_bold,
            template.body_pt + 0.5,
            body_color.clone(),
            emphasis_color.clone(),
        );

        y -= pt_to_mm(2.0);
    }

    Ok(doc)
}

/// Main export function
pub fn generate_pdf(request: &ExportRequest) -> Result<Vec<u8>> {
    let template = Template::get(request.template_id);

    let doc = match request.document_type {
        DocumentType::Resume => {
            generate_resume_pdf(&request.text, request.meta.as_ref(), &template)
                .context("Failed to generate resume PDF")?
        }
        DocumentType::CoverLetter => {
            generate_cover_letter_pdf(&request.text, request.meta.as_ref(), &template)
                .context("Failed to generate cover letter PDF")?
        }
    };

    // Convert to bytes
    let mut buffer = Vec::new();
    doc.save(&mut BufWriter::new(&mut buffer))
        .context("Failed to save PDF to buffer")?;

    Ok(buffer)
}
