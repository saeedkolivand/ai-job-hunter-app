use docx_rs::*;
use crate::export::{
    templates::Template,
    types::{FontFamily, GenerationMeta, TextSegment},
};

/// Convert points to twentieths of a point (DOCX unit).
pub fn pt_to_dxa(pt: f32) -> usize {
    (pt * 20.0) as usize
}

/// Convert inches to twentieths of a point.
pub fn inch_to_dxa(inch: f32) -> i32 {
    (inch * 1440.0) as i32
}

/// Convert RGB tuple to hex string.
pub fn rgb_to_hex(rgb: (u8, u8, u8)) -> String {
    format!("{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2)
}

/// Map FontFamily to the font name string used in DOCX run properties.
pub fn docx_font_name(family: FontFamily) -> &'static str {
    match family {
        FontFamily::Calibri => "Calibri",
        FontFamily::Inter => "Inter",
        FontFamily::SourceSerif4 => "Source Serif 4",
        FontFamily::Manrope => "Manrope",
        FontFamily::JetBrainsMono => "JetBrains Mono",
        FontFamily::PlayfairDisplay => "Playfair Display",
    }
}

/// Color palette for DOCX rendering.
pub struct DocxColors {
    pub name: String,
    pub section: String,
    pub body: String,
    pub date: String,
    pub emphasis: String,
}

/// Setup color palette from template.
pub fn setup_colors(template: &Template) -> DocxColors {
    DocxColors {
        name: rgb_to_hex(template.name_color),
        section: rgb_to_hex(template.section_color),
        body: rgb_to_hex(template.body_color),
        date: rgb_to_hex(template.date_color),
        emphasis: rgb_to_hex(template.emphasis_color),
    }
}

/// Create text runs from segments with bold formatting.
/// Uses the template's body_family font name.
pub fn create_runs(
    segments: &[TextSegment],
    font_size: usize,
    color: &str,
    bold_color: Option<&str>,
    font_name: &str,
) -> Vec<Run> {
    segments
        .iter()
        .map(|seg| {
            let mut run = Run::new()
                .add_text(&seg.text)
                .size(font_size)
                .fonts(RunFonts::new().ascii(font_name));

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

/// Render name line using template's name_family font.
pub fn render_name_line(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    colors: &DocxColors,
) -> Paragraph {
    let name_text = meta
        .and_then(|m| m.candidate_name.as_ref())
        .map(|s| s.as_str())
        .unwrap_or(text);

    let font_name = docx_font_name(template.fonts.name_family);

    let mut para = Paragraph::new()
        .add_run(
            Run::new()
                .add_text(name_text)
                .size(pt_to_dxa(template.name_pt))
                .bold()
                .color(&colors.name)
                .fonts(RunFonts::new().ascii(font_name)),
        );

    if template.name_centered {
        para = para.align(AlignmentType::Center);
    }

    para
}

/// Render contact line.
pub fn render_contact_line(
    text: &str,
    template: &Template,
    colors: &DocxColors,
) -> Paragraph {
    let font_name = docx_font_name(template.fonts.body_family);

    let mut para = Paragraph::new()
        .add_run(
            Run::new()
                .add_text(text)
                .size(pt_to_dxa(9.0))
                .color(&colors.date)
                .fonts(RunFonts::new().ascii(font_name)),
        );

    if template.name_centered {
        para = para.align(AlignmentType::Center);
    }

    para
}

/// Render section header using template's heading_family font.
pub fn render_section_header(
    text: &str,
    template: &Template,
    colors: &DocxColors,
) -> Paragraph {
    let font_name = docx_font_name(template.fonts.heading_family);

    let (header_text, effective_pt) = if template.section_small_caps {
        (text.to_uppercase(), template.section_pt * 0.85)
    } else if template.section_all_caps {
        (text.to_uppercase(), template.section_pt)
    } else {
        (text.to_string(), template.section_pt)
    };

    let char_spacing = if template.section_small_caps || template.section_all_caps { 30 } else { 0 };

    let para = Paragraph::new()
        .add_run(
            Run::new()
                .add_text(&header_text)
                .size(pt_to_dxa(effective_pt))
                .bold()
                .color(&colors.section)
                .fonts(RunFonts::new().ascii(font_name))
                .character_spacing(char_spacing),
        );

    let _ = &template.section_style; // suppress unused warning
    para
}

/// Render job entry with date on right.
pub fn render_job_entry(
    segments: &[TextSegment],
    date: Option<&str>,
    template: &Template,
    colors: &DocxColors,
) -> Paragraph {
    let font_name = docx_font_name(template.fonts.body_family);
    let runs = create_runs(
        segments,
        pt_to_dxa(template.body_pt),
        &colors.body,
        Some(&colors.emphasis),
        font_name,
    );

    let mut para = Paragraph::new();

    for run in runs {
        para = para.add_run(run.bold());
    }

    if let Some(date) = date {
        para = para
            .add_run(Run::new().add_tab())
            .add_run(
                Run::new()
                    .add_text(date)
                    .size(pt_to_dxa(9.5))
                    .color(&colors.date)
                    .fonts(RunFonts::new().ascii(font_name)),
            );
    }

    para = para.add_tab(
        Tab::new()
            .val(TabValueType::Right)
            .pos(inch_to_dxa(6.27) as usize),
    );

    para
}

/// Render job title (italic if supported by the body font).
pub fn render_job_title(
    segments: &[TextSegment],
    template: &Template,
    colors: &DocxColors,
) -> Paragraph {
    let font_name = docx_font_name(template.fonts.body_family);
    let runs = create_runs(
        segments,
        pt_to_dxa(template.body_pt - 0.5),
        &colors.date,
        Some(&colors.emphasis),
        font_name,
    );

    let mut para = Paragraph::new();

    for run in runs {
        let r = if template.job_title_italic { run.italic() } else { run };
        para = para.add_run(r);
    }

    para
}

/// Render bullet point.
pub fn render_bullet_line(
    segments: &[TextSegment],
    template: &Template,
    colors: &DocxColors,
) -> Paragraph {
    let font_name = docx_font_name(template.fonts.body_family);
    let runs = create_runs(
        segments,
        pt_to_dxa(template.body_pt),
        &colors.body,
        Some(&colors.emphasis),
        font_name,
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

    para = para.numbering(NumberingId::new(1), IndentLevel::new(0));
    para
}

/// Render plain text.
pub fn render_text_line(
    segments: &[TextSegment],
    template: &Template,
    colors: &DocxColors,
) -> Paragraph {
    let font_name = docx_font_name(template.fonts.body_family);
    let runs = create_runs(
        segments,
        pt_to_dxa(template.body_pt),
        &colors.body,
        Some(&colors.emphasis),
        font_name,
    );

    let mut para = Paragraph::new();

    for run in runs {
        para = para.add_run(run);
    }

    para
}

/// Render a cover-letter paragraph with proper spacing/indent via pPr.
/// Never emits blank paragraphs as spacers — uses Spacing::after instead.
pub fn render_cover_letter_paragraph(
    text: &str,
    template: &Template,
    colors: &DocxColors,
    font_name: &str,
) -> Paragraph {
    use crate::export::templates::ParagraphIndent;

    let segments = crate::export::parser::parse_inline_md(text);
    let runs = create_runs(
        &segments,
        pt_to_dxa(template.body_pt),
        &colors.body,
        Some(&colors.emphasis),
        font_name,
    );

    // spacing_after in twentieths-of-a-point (1pt = 20 dxa)
    let spacing_after = match template.cover_letter.paragraph_indent {
        ParagraphIndent::BlockNoIndent => (template.cover_letter.paragraph_spacing_pt * 20.0) as u32,
        ParagraphIndent::FirstLine => 0,
    };

    let first_line_indent = match template.cover_letter.paragraph_indent {
        ParagraphIndent::FirstLine => Some(inch_to_dxa(0.25)), // 360 dxa
        ParagraphIndent::BlockNoIndent => None,
    };

    let mut para = Paragraph::new()
        .line_spacing(LineSpacing::new().after(spacing_after));

    if let Some(indent) = first_line_indent {
        para = para.indent(Some(indent), None, None, None);
    }

    for run in runs {
        para = para.add_run(run);
    }

    para
}

/// Create bullet numbering definition.
pub fn create_bullet_numbering() -> (AbstractNumbering, Numbering) {
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

    (abstract_num, num)
}
