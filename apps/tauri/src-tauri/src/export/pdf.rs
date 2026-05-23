use anyhow::{Context, Result};
use printpdf::*;

use super::{
    parser::{parse_resume, strip_md},
    templates::{calculate_spacing, Template},
    types::{DocumentType, ExportRequest, GenerationMeta, LineKind, TextSegment},
};

const MM_PER_INCH: f32 = 25.4;
const PT_PER_MM: f32 = 2.834_645_7;

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

/// Build text operations for mixed bold/normal text
struct TextOpsConfig {
    x: f32,
    y: f32,
    font_regular: FontId,
    font_bold: FontId,
    font_size: f32,
    normal_color: Color,
    bold_color: Color,
}

fn build_text_ops(segments: &[TextSegment], config: TextOpsConfig) -> Vec<Op> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut ops = Vec::new();
    let text_pos = Point {
        x: Mm(config.x).into(),
        y: Mm(config.y).into(),
    };
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor { pos: text_pos });

    for segment in segments {
        let font_id = if segment.bold { config.font_bold.clone() } else { config.font_regular.clone() };
        let color = if segment.bold { config.bold_color.clone() } else { config.normal_color.clone() };

        ops.push(Op::SetFillColor { col: color });
        ops.push(Op::SetFont { 
            font: PdfFontHandle::External(font_id), 
            size: Pt(config.font_size) 
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text(segment.text.clone())],
        });
    }
    ops.push(Op::EndTextSection);

    ops
}

/// Merge a word (with optional leading space) into the last segment if same boldness, else push new
fn push_word_to_segs(segs: &mut Vec<super::types::TextSegment>, word: &str, bold: bool, space_before: bool) {
    let is_punct_start = word.starts_with([',', '.', '!', '?', ';', ':', '-']);
    let text = if space_before && !is_punct_start { format!(" {}", word) } else { word.to_string() };
    if let Some(last) = segs.last_mut() {
        if last.bold == bold {
            last.text.push_str(&text);
            return;
        }
    }
    segs.push(super::types::TextSegment { text, bold });
}

/// Wrap text segments into lines fitting max_width_mm
fn wrap_segments(
    segments: &[super::types::TextSegment],
    max_width_mm: f32,
    font_size_pt: f32,
) -> Vec<Vec<super::types::TextSegment>> {
    let avg_char_width = pt_to_mm(font_size_pt) * 0.5;
    let chars_per_line = ((max_width_mm / avg_char_width) as usize).max(20);

    let mut words: Vec<(String, bool)> = Vec::new();
    for seg in segments {
        for word in seg.text.split_whitespace() {
            words.push((word.to_string(), seg.bold));
        }
    }

    if words.is_empty() {
        return vec![segments.to_vec()];
    }

    let mut lines: Vec<Vec<super::types::TextSegment>> = Vec::new();
    let mut current: Vec<super::types::TextSegment> = Vec::new();
    let mut current_len = 0usize;

    for (word, bold) in &words {
        let wl = word.len();
        if current_len == 0 {
            push_word_to_segs(&mut current, word, *bold, false);
            current_len = wl;
        } else if current_len + 1 + wl <= chars_per_line {
            push_word_to_segs(&mut current, word, *bold, true);
            current_len += 1 + wl;
        } else {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            push_word_to_segs(&mut current, word, *bold, false);
            current_len = wl;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(segments.to_vec());
    }

    lines
}

/// Build a horizontal line
fn build_line(x1: f32, y1: f32, x2: f32, _y2: f32, color: Color, thickness: f32) -> Vec<Op> {
    let line = Line {
        points: vec![
            LinePoint { p: Point::new(Mm(x1), Mm(y1)), bezier: false },
            LinePoint { p: Point::new(Mm(x2), Mm(y1)), bezier: false },
        ],
        is_closed: false,
    };
    vec![
        Op::SetOutlineColor { col: color },
        Op::SetOutlineThickness { pt: Pt(thickness) },
        Op::DrawLine { line },
    ]
}

/// Generate PDF for resume
fn generate_resume_pdf(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
) -> Result<Vec<u8>> {
    let parsed = parse_resume(text);
    let mut doc = PdfDocument::new("Resume");

    // Load fonts
    let font_data_regular = include_bytes!("../../fonts/calibri.ttf");
    let font_data_bold = include_bytes!("../../fonts/calibrib.ttf");

    let mut warnings = Vec::new();
    let font_regular = ParsedFont::from_bytes(font_data_regular, 0, &mut warnings)
        .context("Failed to parse regular font")?;
    let font_bold = ParsedFont::from_bytes(font_data_bold, 0, &mut warnings)
        .context("Failed to parse bold font")?;

    let font_regular_id = doc.add_font(&font_regular);
    let font_bold_id = doc.add_font(&font_bold);

    // Margins
    let margin_left = inch_to_mm(template.margin_in);
    let margin_right = inch_to_mm(template.margin_in);
    let margin_top = inch_to_mm(0.9);

    let page_width = 210.0;
    let page_height = 297.0;

    let mut y = page_height - margin_top;
    let line_height = pt_to_mm(template.body_pt) * template.line_spacing;

    // Colors
    let name_color = rgb_to_color(template.name_color);
    let section_color = rgb_to_color(template.section_color);
    let body_color = rgb_to_color(template.body_color);
    let date_color = rgb_to_color(template.date_color);
    let emphasis_color = rgb_to_color(template.emphasis_color);
    let rule_color = rgb_to_color(template.rule_color);

    let mut ops: Vec<Op> = Vec::new();
    let mut previous_kind: Option<LineKind> = None;

    for line in &parsed.lines {
        let spacing = calculate_spacing(&line.kind, previous_kind.as_ref());
        y -= pt_to_mm(spacing.0);

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

                let text_pos = Point { x: Mm(x).into(), y: Mm(y).into() };
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFillColor { col: name_color.clone() });
                ops.push(Op::SetTextCursor { pos: text_pos });
                ops.push(Op::SetFont { font: PdfFontHandle::External(font_bold_id.clone()), size: Pt(template.name_pt) });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(name_text.to_string())],
                });
                ops.push(Op::EndTextSection);

                y -= pt_to_mm(template.name_pt) * 1.2;
            }

            LineKind::Contact => {
                let x = if template.name_centered {
                    page_width / 2.0 - (line.text.len() as f32 * pt_to_mm(9.0) * 0.3)
                } else {
                    margin_left
                };

                let text_pos = Point { x: Mm(x).into(), y: Mm(y).into() };
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFillColor { col: date_color.clone() });
                ops.push(Op::SetTextCursor { pos: text_pos });
                ops.push(Op::SetFont { font: PdfFontHandle::External(font_regular_id.clone()), size: Pt(9.0) });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(line.text.clone())],
                });
                ops.push(Op::EndTextSection);

                y -= pt_to_mm(9.0) * 1.2;

                // Draw line
                ops.extend(build_line(margin_left, y, page_width - margin_right, y, rule_color.clone(), 0.5));

                y -= pt_to_mm(5.0);
            }

            LineKind::SectionHeader => {
                y -= pt_to_mm(4.0);

                let header_text = if template.section_all_caps {
                    line.text.to_uppercase()
                } else {
                    line.text.clone()
                };

                let text_pos = Point { x: Mm(margin_left).into(), y: Mm(y).into() };
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFillColor { col: section_color.clone() });
                ops.push(Op::SetTextCursor { pos: text_pos });
                ops.push(Op::SetFont { font: PdfFontHandle::External(font_bold_id.clone()), size: Pt(template.section_pt) });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(header_text)],
                });
                ops.push(Op::EndTextSection);

                y -= pt_to_mm(template.section_pt) * 1.2;

                // Draw section line if needed
                match template.section_style {
                    super::templates::SectionStyle::RuledBottom => {
                        ops.extend(build_line(margin_left, y, page_width - margin_right, y, section_color.clone(), 1.0));
                        y -= pt_to_mm(2.0);
                    }
                    super::templates::SectionStyle::Underline => {
                        ops.extend(build_line(margin_left, y, page_width - margin_right, y, section_color.clone(), 0.5));
                        y -= pt_to_mm(2.0);
                    }
                    _ => {}
                }

                y -= pt_to_mm(4.0);
            }

            LineKind::JobEntry => {
                ops.extend(build_text_ops(
                    &line.segments,
                    TextOpsConfig {
                        x: margin_left,
                        y,
                        font_regular: font_regular_id.clone(),
                        font_bold: font_bold_id.clone(),
                        font_size: template.body_pt,
                        normal_color: body_color.clone(),
                        bold_color: emphasis_color.clone(),
                    },
                ));

                // Date on right
                if let Some(date) = &line.right_text {
                    let date_x = page_width - margin_right - (date.len() as f32 * pt_to_mm(9.0) * 0.3);
                    let text_pos = Point { x: Mm(date_x).into(), y: Mm(y).into() };
                    ops.push(Op::StartTextSection);
                    ops.push(Op::SetFillColor { col: date_color.clone() });
                    ops.push(Op::SetTextCursor { pos: text_pos });
                    ops.push(Op::SetFont { font: PdfFontHandle::External(font_regular_id.clone()), size: Pt(9.0) });
                    ops.push(Op::ShowText {
                        items: vec![TextItem::Text(date.clone())],
                    });
                    ops.push(Op::EndTextSection);
                }

                y -= line_height + pt_to_mm(2.0);
            }

            LineKind::JobTitle => {
                let text_pos = Point { x: Mm(margin_left).into(), y: Mm(y).into() };
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFillColor { col: date_color.clone() });
                ops.push(Op::SetTextCursor { pos: text_pos });
                ops.push(Op::SetFont { font: PdfFontHandle::External(font_regular_id.clone()), size: Pt(template.body_pt - 0.5) });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(line.text.clone())],
                });
                ops.push(Op::EndTextSection);

                y -= line_height;
            }

            LineKind::Bullet => {
                // Draw bullet point
                let bullet_pos = Point { x: Mm(margin_left + 1.3).into(), y: Mm(y).into() };
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFillColor { col: body_color.clone() });
                ops.push(Op::SetTextCursor { pos: bullet_pos });
                ops.push(Op::SetFont { font: PdfFontHandle::External(font_regular_id.clone()), size: Pt(template.body_pt) });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text("•".to_string())],
                });
                ops.push(Op::EndTextSection);

                // Draw wrapped bullet text
                let bullet_content_width = page_width - margin_left - margin_right - 4.0;
                for seg_line in wrap_segments(&line.segments, bullet_content_width, template.body_pt) {
                    ops.extend(build_text_ops(
                        &seg_line,
                        TextOpsConfig {
                            x: margin_left + 4.0,
                            y,
                            font_regular: font_regular_id.clone(),
                            font_bold: font_bold_id.clone(),
                            font_size: template.body_pt,
                            normal_color: body_color.clone(),
                            bold_color: emphasis_color.clone(),
                        },
                    ));
                    y -= line_height;
                }
            }

            LineKind::Text => {
                let content_width = page_width - margin_left - margin_right;
                for seg_line in wrap_segments(&line.segments, content_width, template.body_pt) {
                    ops.extend(build_text_ops(
                        &seg_line,
                        TextOpsConfig {
                            x: margin_left,
                            y,
                            font_regular: font_regular_id.clone(),
                            font_bold: font_bold_id.clone(),
                            font_size: template.body_pt,
                            normal_color: body_color.clone(),
                            bold_color: emphasis_color.clone(),
                        },
                    ));
                    y -= line_height;
                }
            }
        }

        y -= pt_to_mm(spacing.1);
        previous_kind = Some(line.kind);
    }

    let page = PdfPage::new(Mm(page_width), Mm(page_height), ops);
    let mut save_warnings = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut save_warnings);

    Ok(pdf_bytes)
}

/// Generate PDF for cover letter
fn generate_cover_letter_pdf(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::new("Cover Letter");

    // Load fonts
    let font_data_regular = include_bytes!("../../fonts/calibri.ttf");
    let font_data_bold = include_bytes!("../../fonts/calibrib.ttf");

    let mut warnings = Vec::new();
    let font_regular = ParsedFont::from_bytes(font_data_regular, 0, &mut warnings)
        .context("Failed to parse regular font")?;
    let font_bold = ParsedFont::from_bytes(font_data_bold, 0, &mut warnings)
        .context("Failed to parse bold font")?;

    let font_regular_id = doc.add_font(&font_regular);
    let font_bold_id = doc.add_font(&font_bold);

    let margin_left = 30.0_f32;    // ~1.18" — generous symmetric margins
    let margin_right = 30.0_f32;
    let page_width = 210.0_f32;
    let page_height = 297.0_f32;
    let content_width = page_width - margin_left - margin_right; // ~150 mm

    let mut y = page_height - 25.4_f32; // 1" top margin
    let line_height = pt_to_mm(template.body_pt) * 1.55; // relaxed line spacing

    let name_color = rgb_to_color(template.name_color);
    let body_color = rgb_to_color(template.body_color);
    let date_color = rgb_to_color(template.date_color);
    let emphasis_color = rgb_to_color(template.emphasis_color);

    let lines: Vec<&str> = text.lines().collect();
    let mut header_done = false;
    let mut in_body = false;
    let mut ops: Vec<Op> = Vec::new();
    let initial_y = y;

    for raw_line in lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            y -= pt_to_mm(5.0);
            continue;
        }

        let clean = strip_md(trimmed);
        // Strip # heading markers but keep ** intact for inline bold detection
        let text_no_hash = trimmed.trim_start_matches('#').trim_start().trim_end_matches('#').trim_end();
        let segments = super::parser::parse_inline_md(text_no_hash);

        let is_salutation = clean.starts_with("Dear") || clean.starts_with("Sehr geehrte");
        let is_signoff = clean.starts_with("Kind regards")
            || clean.starts_with("Sincerely")
            || clean.starts_with("Mit freundlichen Grüßen")
            || clean.starts_with("Hochachtungsvoll")
            || clean.starts_with("Viele Grüße");

        // First line is name
        if !header_done && (y - initial_y).abs() < 0.1 {
            let name_text = meta
                .and_then(|m| m.candidate_name.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(&clean);

            let text_pos = Point { x: Mm(margin_left).into(), y: Mm(y).into() };
            ops.push(Op::StartTextSection);
            ops.push(Op::SetFillColor { col: name_color.clone() });
            ops.push(Op::SetTextCursor { pos: text_pos });
            ops.push(Op::SetFont { font: PdfFontHandle::External(font_bold_id.clone()), size: Pt(template.name_pt - 2.0) });
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(name_text.to_string())],
            });
            ops.push(Op::EndTextSection);

            y -= pt_to_mm(template.name_pt - 2.0) * 1.2 + pt_to_mm(1.5);
            continue;
        }

        // Contact/address — split on | and render each item on its own line
        if !header_done && (clean.contains('@') || clean.contains('|')) {
            let contact_font_size = template.body_pt - 0.5;
            let contact_line_h = pt_to_mm(contact_font_size) * 1.4;
            for item in clean.split('|').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let text_pos = Point { x: Mm(margin_left).into(), y: Mm(y).into() };
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFillColor { col: date_color.clone() });
                ops.push(Op::SetTextCursor { pos: text_pos });
                ops.push(Op::SetFont { font: PdfFontHandle::External(font_regular_id.clone()), size: Pt(contact_font_size) });
                ops.push(Op::ShowText { items: vec![TextItem::Text(item.to_string())] });
                ops.push(Op::EndTextSection);
                y -= contact_line_h;
            }
            y -= pt_to_mm(2.0);
            continue;
        }

        // Salutation — add extra gap between header block and letter body
        if is_salutation {
            if !header_done {
                y -= pt_to_mm(10.0); // professional gap before greeting
            }
            header_done = true;
            in_body = true;
            let text_pos = Point { x: Mm(margin_left).into(), y: Mm(y).into() };
            ops.push(Op::StartTextSection);
            ops.push(Op::SetFillColor { col: body_color.clone() });
            ops.push(Op::SetTextCursor { pos: text_pos });
            ops.push(Op::SetFont { font: PdfFontHandle::External(font_regular_id.clone()), size: Pt(template.body_pt) });
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(clean.clone())],
            });
            ops.push(Op::EndTextSection);
            y -= line_height + pt_to_mm(4.0);
            continue;
        }

        // Signoff
        if is_signoff {
            y -= pt_to_mm(4.0);
            let text_pos = Point { x: Mm(margin_left).into(), y: Mm(y).into() };
            ops.push(Op::StartTextSection);
            ops.push(Op::SetFillColor { col: body_color.clone() });
            ops.push(Op::SetTextCursor { pos: text_pos });
            ops.push(Op::SetFont { font: PdfFontHandle::External(font_regular_id.clone()), size: Pt(template.body_pt) });
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(clean.clone())],
            });
            ops.push(Op::EndTextSection);
            y -= line_height + pt_to_mm(6.0); // space for handwritten signature
            continue;
        }

        // Addressee block (wrapped)
        if !in_body {
            let addr_segs = super::parser::parse_inline_md(&clean);
            let addr_wrapped = wrap_segments(&addr_segs, content_width, template.body_pt);
            let addr_last = addr_wrapped.len().saturating_sub(1);
            for (i, seg_line) in addr_wrapped.into_iter().enumerate() {
                let text_pos = Point { x: Mm(margin_left).into(), y: Mm(y).into() };
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFillColor { col: body_color.clone() });
                ops.push(Op::SetTextCursor { pos: text_pos });
                ops.push(Op::SetFont { font: PdfFontHandle::External(font_regular_id.clone()), size: Pt(template.body_pt) });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(seg_line.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(""))],
                });
                ops.push(Op::EndTextSection);
                y -= if i == addr_last { line_height + pt_to_mm(1.0) } else { line_height };
            }
            continue;
        }

        // Body paragraphs (wrapped)
        let para_lines = wrap_segments(&segments, content_width, template.body_pt);
        let last_idx = para_lines.len().saturating_sub(1);
        for (i, seg_line) in para_lines.into_iter().enumerate() {
            ops.extend(build_text_ops(
                &seg_line,
                TextOpsConfig {
                    x: margin_left,
                    y,
                    font_regular: font_regular_id.clone(),
                    font_bold: font_bold_id.clone(),
                    font_size: template.body_pt,
                    normal_color: body_color.clone(),
                    bold_color: emphasis_color.clone(),
                },
            ));
            y -= if i == last_idx { line_height + pt_to_mm(4.5) } else { line_height };
        }
    }

    let page = PdfPage::new(Mm(page_width), Mm(page_height), ops);
    let mut save_warnings = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut save_warnings);

    Ok(pdf_bytes)
}

/// Extract the section between two markers, or the full text if not found
fn extract_section<'a>(text: &'a str, start_marker: &str, end_marker: Option<&str>) -> &'a str {
    let start = if let Some(idx) = text.find(start_marker) {
        let after = &text[idx + start_marker.len()..];
        // skip the marker line itself
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
pub fn generate_pdf(request: &ExportRequest) -> Result<Vec<u8>> {
    let template = Template::get(request.template_id);

    match request.document_type {
        DocumentType::Resume => {
            let text = extract_section(
                &request.text,
                "### CANDIDATE RESUME ###",
                Some("### JOB ADVERTISEMENT ###"),
            );
            let text = if text.is_empty() { &request.text } else { text };
            generate_resume_pdf(text, request.meta.as_ref(), &template)
                .context("Failed to generate resume PDF")
        }
        DocumentType::CoverLetter => {
            let text = extract_section(
                &request.text,
                "### COMPLETE COVER LETTER ###",
                None,
            );
            let text = if text.is_empty() { &request.text } else { text };
            generate_cover_letter_pdf(text, request.meta.as_ref(), &template)
                .context("Failed to generate cover letter PDF")
        }
    }
}
