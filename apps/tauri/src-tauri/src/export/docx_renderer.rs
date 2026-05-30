use docx_rs::*;
use crate::export::{
    links::{split_urls, Span},
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

/// Convert millimetres to twentieths of a point (the DOCX `dxa` unit used for
/// page geometry). `1 in = 25.4 mm = 1440 dxa`, so A4's 210 × 297 mm becomes
/// 11906 × 16838 dxa — the same page size Word writes for A4.
pub fn mm_to_dxa(mm: f32) -> u32 {
    (mm * 1440.0 / 25.4).round() as u32
}

/// Convert RGB tuple to hex string.
pub fn rgb_to_hex(rgb: (u8, u8, u8)) -> String {
    format!("{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2)
}

/// Map a bundled [`FontFamily`] to a widely-available system font.
///
/// The bundled TTFs (Inter, Source Serif 4, Manrope, …) are **not** embedded in
/// the DOCX yet — true font embedding is deferred to Phase 5 — so a reader that
/// lacks them would silently substitute an arbitrary face. Referencing these
/// common fallbacks instead (present on Windows/Office, with close cross-platform
/// equivalents) keeps the output predictable everywhere.
pub fn docx_fallback_font(family: FontFamily) -> &'static str {
    match family {
        FontFamily::Calibri => "Calibri",
        FontFamily::Inter => "Calibri",
        FontFamily::Manrope => "Calibri",
        FontFamily::SourceSerif4 => "Georgia",
        FontFamily::PlayfairDisplay => "Cambria",
        FontFamily::JetBrainsMono => "Consolas",
    }
}

/// Run fonts for a bundled family, resolved to its predictable DOCX fallback and
/// applied to both the ASCII and high-ANSI ranges so accented Latin characters
/// (common in DACH names) render in the same face, not the reader's default.
pub fn docx_run_fonts(family: FontFamily) -> RunFonts {
    let name = docx_fallback_font(family);
    RunFonts::new().ascii(name).hi_ansi(name)
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
/// Uses the predictable DOCX fallback for the given bundled family.
pub fn create_runs(
    segments: &[TextSegment],
    font_size: usize,
    color: &str,
    bold_color: Option<&str>,
    family: FontFamily,
) -> Vec<Run> {
    segments
        .iter()
        .map(|seg| {
            let mut run = Run::new()
                .add_text(&seg.text)
                .size(font_size)
                .fonts(docx_run_fonts(family));

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

    let mut para = Paragraph::new()
        .add_run(
            Run::new()
                .add_text(name_text)
                .size(pt_to_dxa(template.name_pt))
                .bold()
                .color(&colors.name)
                .fonts(docx_run_fonts(template.fonts.name_family)),
        );

    if template.name_centered {
        para = para.align(AlignmentType::Center);
    }

    para
}

/// Render contact line, converting URLs to clickable hyperlinks with friendly labels.
pub fn render_contact_line(
    text: &str,
    template: &Template,
    colors: &DocxColors,
) -> Paragraph {
    let family = template.fonts.body_family;
    let spans = split_urls(text);

    let mut para = Paragraph::new();

    for span in spans {
        match span {
            Span::Text(t) => {
                para = para.add_run(
                    Run::new()
                        .add_text(&t)
                        .size(pt_to_dxa(9.0))
                        .color(&colors.date)
                        .fonts(docx_run_fonts(family)),
                );
            }
            Span::Link { label, url } => {
                let link_run = Run::new()
                    .add_text(&label)
                    .size(pt_to_dxa(9.0))
                    .color(&colors.date)
                    .fonts(docx_run_fonts(family));
                let hyperlink = Hyperlink::new(&url, HyperlinkType::External).add_run(link_run);
                para = para.add_hyperlink(hyperlink);
            }
        }
    }

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
                .fonts(docx_run_fonts(template.fonts.heading_family))
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
    let family = template.fonts.body_family;
    let runs = create_runs(
        segments,
        pt_to_dxa(template.body_pt),
        &colors.body,
        Some(&colors.emphasis),
        family,
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
                    .fonts(docx_run_fonts(family)),
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
    let family = template.fonts.body_family;
    let runs = create_runs(
        segments,
        pt_to_dxa(template.body_pt - 0.5),
        &colors.date,
        Some(&colors.emphasis),
        family,
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
    let family = template.fonts.body_family;
    let runs = create_runs(
        segments,
        pt_to_dxa(template.body_pt),
        &colors.body,
        Some(&colors.emphasis),
        family,
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
    let family = template.fonts.body_family;
    let runs = create_runs(
        segments,
        pt_to_dxa(template.body_pt),
        &colors.body,
        Some(&colors.emphasis),
        family,
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
    family: FontFamily,
) -> Paragraph {
    use crate::export::templates::ParagraphIndent;

    let segments = crate::export::parser::parse_inline_md(text);
    let runs = create_runs(
        &segments,
        pt_to_dxa(template.body_pt),
        &colors.body,
        Some(&colors.emphasis),
        family,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mm_to_dxa_matches_word_a4_and_letter() {
        // Word writes A4 as 11906 × 16838 dxa and US Letter as 12240 × 15840.
        assert_eq!(mm_to_dxa(210.0), 11906);
        assert_eq!(mm_to_dxa(297.0), 16838);
        assert_eq!(mm_to_dxa(215.9), 12240);
        assert_eq!(mm_to_dxa(279.4), 15840);
    }

    #[test]
    fn fallback_fonts_are_common_system_faces() {
        // Every bundled family resolves to a face present on Windows/Office so the
        // reader never has to silently substitute an un-embedded bundled font.
        assert_eq!(docx_fallback_font(FontFamily::Calibri), "Calibri");
        assert_eq!(docx_fallback_font(FontFamily::Inter), "Calibri");
        assert_eq!(docx_fallback_font(FontFamily::Manrope), "Calibri");
        assert_eq!(docx_fallback_font(FontFamily::SourceSerif4), "Georgia");
        assert_eq!(docx_fallback_font(FontFamily::PlayfairDisplay), "Cambria");
        assert_eq!(docx_fallback_font(FontFamily::JetBrainsMono), "Consolas");
    }
}
