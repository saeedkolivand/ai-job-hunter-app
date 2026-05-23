use anyhow::Context;
use printpdf::*;
use crate::export::{
    templates::{SectionStyle, Template},
    types::{GenerationMeta, TextSegment},
};

/// Font loading result
pub struct LoadedFonts {
    pub regular_id: FontId,
    pub bold_id: FontId,
}

/// Color palette for PDF rendering
pub struct ColorPalette {
    pub name: Color,
    pub section: Color,
    pub body: Color,
    pub date: Color,
    pub emphasis: Color,
    pub rule: Color,
}

/// Layout configuration
pub struct LayoutConfig {
    pub margin_left: f32,
    pub margin_right: f32,
    pub page_width: f32,
    pub page_height: f32,
    pub line_height: f32,
}

/// Load and parse fonts for PDF generation
pub fn load_fonts(doc: &mut PdfDocument) -> anyhow::Result<LoadedFonts> {
    let font_data_regular = include_bytes!("../../fonts/calibri.ttf");
    let font_data_bold = include_bytes!("../../fonts/calibrib.ttf");

    let mut warnings = Vec::new();
    let font_regular = ParsedFont::from_bytes(font_data_regular, 0, &mut warnings)
        .context("Failed to parse regular font")?;
    let font_bold = ParsedFont::from_bytes(font_data_bold, 0, &mut warnings)
        .context("Failed to parse bold font")?;

    let regular_id = doc.add_font(&font_regular);
    let bold_id = doc.add_font(&font_bold);

    Ok(LoadedFonts {
        regular_id,
        bold_id,
    })
}

/// Setup color palette from template
pub fn setup_colors(template: &Template) -> ColorPalette {
    ColorPalette {
        name: rgb_to_color(template.name_color),
        section: rgb_to_color(template.section_color),
        body: rgb_to_color(template.body_color),
        date: rgb_to_color(template.date_color),
        emphasis: rgb_to_color(template.emphasis_color),
        rule: rgb_to_color(template.rule_color),
    }
}

/// Setup layout configuration from template
pub fn setup_layout(template: &Template) -> LayoutConfig {
    const MM_PER_INCH: f32 = 25.4;
    const PT_PER_MM: f32 = 2.834_645_7;

    let margin_left = template.margin_in * MM_PER_INCH;
    let margin_right = template.margin_in * MM_PER_INCH;
    let line_height = (template.body_pt / PT_PER_MM) * template.line_spacing;

    LayoutConfig {
        margin_left,
        margin_right,
        page_width: 210.0,
        page_height: 297.0,
        line_height,
    }
}

/// Convert RGB tuple to printpdf Color
pub fn rgb_to_color(rgb: (u8, u8, u8)) -> Color {
    Color::Rgb(Rgb::new(
        rgb.0 as f32 / 255.0,
        rgb.1 as f32 / 255.0,
        rgb.2 as f32 / 255.0,
        None,
    ))
}

/// Convert points to millimeters
pub fn pt_to_mm(pt: f32) -> f32 {
    pt / 2.834_645_7
}

/// Build text operations for mixed bold/normal text
pub struct TextOpsConfig {
    pub x: f32,
    pub y: f32,
    pub font_regular: FontId,
    pub font_bold: FontId,
    pub font_size: f32,
    pub normal_color: Color,
    pub bold_color: Color,
}

pub fn build_text_ops(segments: &[TextSegment], config: TextOpsConfig) -> Vec<Op> {
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
fn push_word_to_segs(segs: &mut Vec<TextSegment>, word: &str, bold: bool, space_before: bool) {
    let is_punct_start = word.starts_with([',', '.', '!', '?', ';', ':', '-']);
    let text = if space_before && !is_punct_start { format!(" {}", word) } else { word.to_string() };
    if let Some(last) = segs.last_mut() {
        if last.bold == bold {
            last.text.push_str(&text);
            return;
        }
    }
    segs.push(TextSegment { text, bold });
}

/// Wrap text segments into lines fitting max_width_mm
pub fn wrap_segments(
    segments: &[TextSegment],
    max_width_mm: f32,
    font_size_pt: f32,
) -> Vec<Vec<TextSegment>> {
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

    let mut lines: Vec<Vec<TextSegment>> = Vec::new();
    let mut current: Vec<TextSegment> = Vec::new();
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
pub fn build_line(x1: f32, y1: f32, x2: f32, _y2: f32, color: Color, thickness: f32) -> Vec<Op> {
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

/// Render name line
pub fn render_name_line(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFonts,
    y: f32,
) -> (Vec<Op>, f32) {
    let name_text = meta
        .and_then(|m| m.candidate_name.as_ref())
        .map(|s| s.as_str())
        .unwrap_or(text);

    let x = if template.name_centered {
        layout.page_width / 2.0 - (name_text.len() as f32 * pt_to_mm(template.name_pt) * 0.3)
    } else {
        layout.margin_left
    };

    let text_pos = Point { x: Mm(x).into(), y: Mm(y).into() };
    let ops = vec![
        Op::StartTextSection,
        Op::SetFillColor { col: colors.name.clone() },
        Op::SetTextCursor { pos: text_pos },
        Op::SetFont { font: PdfFontHandle::External(fonts.bold_id.clone()), size: Pt(template.name_pt) },
        Op::ShowText {
            items: vec![TextItem::Text(name_text.to_string())],
        },
        Op::EndTextSection,
    ];

    let new_y = y - pt_to_mm(template.name_pt) * 1.2;
    (ops, new_y)
}

/// Render contact line with separator
pub fn render_contact_line(
    text: &str,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFonts,
    y: f32,
) -> (Vec<Op>, f32) {
    let x = if template.name_centered {
        layout.page_width / 2.0 - (text.len() as f32 * pt_to_mm(9.0) * 0.3)
    } else {
        layout.margin_left
    };

    let text_pos = Point { x: Mm(x).into(), y: Mm(y).into() };
    let mut ops = vec![
        Op::StartTextSection,
        Op::SetFillColor { col: colors.date.clone() },
        Op::SetTextCursor { pos: text_pos },
        Op::SetFont { font: PdfFontHandle::External(fonts.regular_id.clone()), size: Pt(9.0) },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ];

    let y_after_text = y - pt_to_mm(9.0) * 1.2;

    // Draw separator line
    ops.extend(build_line(
        layout.margin_left,
        y_after_text,
        layout.page_width - layout.margin_right,
        y_after_text,
        colors.rule.clone(),
        0.5,
    ));

    let new_y = y_after_text - pt_to_mm(5.0);
    (ops, new_y)
}

/// Render section header with optional underline
pub fn render_section_header(
    text: &str,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFonts,
    y: f32,
) -> (Vec<Op>, f32) {
    let y_before = y - pt_to_mm(4.0);

    let header_text = if template.section_all_caps {
        text.to_uppercase()
    } else {
        text.to_string()
    };

    let text_pos = Point { x: Mm(layout.margin_left).into(), y: Mm(y_before).into() };
    let mut ops = vec![
        Op::StartTextSection,
        Op::SetFillColor { col: colors.section.clone() },
        Op::SetTextCursor { pos: text_pos },
        Op::SetFont { font: PdfFontHandle::External(fonts.bold_id.clone()), size: Pt(template.section_pt) },
        Op::ShowText {
            items: vec![TextItem::Text(header_text)],
        },
        Op::EndTextSection,
    ];

    let y_after_header = y_before - pt_to_mm(template.section_pt) * 1.2;

    // Draw section line if needed
    match template.section_style {
        SectionStyle::RuledBottom => {
            ops.extend(build_line(
                layout.margin_left,
                y_after_header,
                layout.page_width - layout.margin_right,
                y_after_header,
                colors.section.clone(),
                1.0,
            ));
        }
        SectionStyle::Underline => {
            ops.extend(build_line(
                layout.margin_left,
                y_after_header,
                layout.page_width - layout.margin_right,
                y_after_header,
                colors.section.clone(),
                0.5,
            ));
        }
        _ => {}
    }

    let new_y = y_after_header - pt_to_mm(4.0);
    (ops, new_y)
}

/// Render job entry with date on right
pub fn render_job_entry(
    segments: &[TextSegment],
    date: Option<&str>,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFonts,
    y: f32,
) -> (Vec<Op>, f32) {
    let mut ops = build_text_ops(
        segments,
        TextOpsConfig {
            x: layout.margin_left,
            y,
            font_regular: fonts.regular_id.clone(),
            font_bold: fonts.bold_id.clone(),
            font_size: template.body_pt,
            normal_color: colors.body.clone(),
            bold_color: colors.emphasis.clone(),
        },
    );

    // Date on right
    if let Some(date) = date {
        let date_x = layout.page_width - layout.margin_right - (date.len() as f32 * pt_to_mm(9.0) * 0.3);
        let text_pos = Point { x: Mm(date_x).into(), y: Mm(y).into() };
        ops.extend(vec![
            Op::StartTextSection,
            Op::SetFillColor { col: colors.date.clone() },
            Op::SetTextCursor { pos: text_pos },
            Op::SetFont { font: PdfFontHandle::External(fonts.regular_id.clone()), size: Pt(9.0) },
            Op::ShowText {
                items: vec![TextItem::Text(date.to_string())],
            },
            Op::EndTextSection,
        ]);
    }

    let new_y = y - layout.line_height + pt_to_mm(2.0);
    (ops, new_y)
}

/// Render job title
pub fn render_job_title(
    text: &str,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFonts,
    y: f32,
) -> (Vec<Op>, f32) {
    let text_pos = Point { x: Mm(layout.margin_left).into(), y: Mm(y).into() };
    let ops = vec![
        Op::StartTextSection,
        Op::SetFillColor { col: colors.date.clone() },
        Op::SetTextCursor { pos: text_pos },
        Op::SetFont { font: PdfFontHandle::External(fonts.regular_id.clone()), size: Pt(template.body_pt - 0.5) },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ];

    let new_y = y - layout.line_height;
    (ops, new_y)
}

/// Render bullet point with wrapped text
pub fn render_bullet_line(
    segments: &[TextSegment],
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFonts,
    y: f32,
) -> (Vec<Op>, f32) {
    let mut ops = Vec::new();

    // Draw bullet point
    let bullet_pos = Point { x: Mm(layout.margin_left + 1.3).into(), y: Mm(y).into() };
    ops.extend(vec![
        Op::StartTextSection,
        Op::SetFillColor { col: colors.body.clone() },
        Op::SetTextCursor { pos: bullet_pos },
        Op::SetFont { font: PdfFontHandle::External(fonts.regular_id.clone()), size: Pt(template.body_pt) },
        Op::ShowText {
            items: vec![TextItem::Text("•".to_string())],
        },
        Op::EndTextSection,
    ]);

    // Draw wrapped bullet text
    let bullet_content_width = layout.page_width - layout.margin_left - layout.margin_right - 4.0;
    let mut current_y = y;
    for seg_line in wrap_segments(segments, bullet_content_width, template.body_pt) {
        ops.extend(build_text_ops(
            &seg_line,
            TextOpsConfig {
                x: layout.margin_left + 4.0,
                y: current_y,
                font_regular: fonts.regular_id.clone(),
                font_bold: fonts.bold_id.clone(),
                font_size: template.body_pt,
                normal_color: colors.body.clone(),
                bold_color: colors.emphasis.clone(),
            },
        ));
        current_y -= layout.line_height;
    }

    (ops, current_y)
}

/// Render plain text with wrapping
pub fn render_text_line(
    segments: &[TextSegment],
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFonts,
    y: f32,
) -> (Vec<Op>, f32) {
    let content_width = layout.page_width - layout.margin_left - layout.margin_right;
    let mut ops = Vec::new();
    let mut current_y = y;

    for seg_line in wrap_segments(segments, content_width, template.body_pt) {
        ops.extend(build_text_ops(
            &seg_line,
            TextOpsConfig {
                x: layout.margin_left,
                y: current_y,
                font_regular: fonts.regular_id.clone(),
                font_bold: fonts.bold_id.clone(),
                font_size: template.body_pt,
                normal_color: colors.body.clone(),
                bold_color: colors.emphasis.clone(),
            },
        ));
        current_y -= layout.line_height;
    }

    (ops, current_y)
}
