use anyhow::Context;
use printpdf::*;
use crate::export::{
    links::{split_urls, Span},
    templates::{CoverLetterHeader, SectionStyle, Template},
    types::{FontFamily, GenerationMeta, TextSegment},
};

// ─── Font loading ─────────────────────────────────────────────────────────────

/// Per-family font IDs (regular + bold + optional italic).
#[derive(Clone)]
pub struct FamilyFonts {
    pub regular: FontId,
    pub bold: FontId,
    pub italic: Option<FontId>,
}

/// All template fonts embedded at compile time regardless of which template the
/// user selects. This keeps the renderer stateless and avoids runtime font-file IO
/// at the cost of ~4.2 MB binary size. Switch to a lazy font registry keyed by
/// FontFamily if bundle size becomes a concern.
pub struct LoadedFontSet {
    pub calibri: FamilyFonts,
    pub inter: FamilyFonts,
    pub source_serif4: FamilyFonts,
    pub manrope: FamilyFonts,
    pub jetbrains_mono: FamilyFonts,
    pub playfair_display: FamilyFonts,
}

impl LoadedFontSet {
    pub fn family(&self, fam: FontFamily) -> &FamilyFonts {
        match fam {
            FontFamily::Calibri => &self.calibri,
            FontFamily::Inter => &self.inter,
            FontFamily::SourceSerif4 => &self.source_serif4,
            FontFamily::Manrope => &self.manrope,
            FontFamily::JetBrainsMono => &self.jetbrains_mono,
            FontFamily::PlayfairDisplay => &self.playfair_display,
        }
    }
}

fn parse_font(bytes: &[u8], doc: &mut PdfDocument) -> anyhow::Result<FontId> {
    let mut w = Vec::new();
    let f = ParsedFont::from_bytes(bytes, 0, &mut w).context("Failed to parse font")?;
    Ok(doc.add_font(&f))
}

/// Load all template fonts into the PDF document.
/// Returns LoadedFontSet ready to use across all render functions.
pub fn load_all_fonts(doc: &mut PdfDocument) -> anyhow::Result<LoadedFontSet> {
    // Calibri (existing — bundled separately)
    let cal_reg = include_bytes!("../../../fonts/calibri.ttf");
    let cal_bol = include_bytes!("../../../fonts/calibrib.ttf");

    // Inter
    let int_reg = include_bytes!("../../../fonts/inter_regular.ttf");
    let int_bol = include_bytes!("../../../fonts/inter_bold.ttf");

    // Source Serif 4
    let ss4_reg = include_bytes!("../../../fonts/source_serif4_regular.ttf");
    let ss4_bol = include_bytes!("../../../fonts/source_serif4_bold.ttf");
    let ss4_ita = include_bytes!("../../../fonts/source_serif4_italic.ttf");

    // Manrope
    let man_reg = include_bytes!("../../../fonts/manrope_regular.ttf");
    let man_bol = include_bytes!("../../../fonts/manrope_bold.ttf");

    // JetBrains Mono
    let jbm_reg = include_bytes!("../../../fonts/jetbrains_mono_regular.ttf");
    let jbm_bol = include_bytes!("../../../fonts/jetbrains_mono_bold.ttf");

    // Playfair Display
    let pfd_reg = include_bytes!("../../../fonts/playfair_display_regular.ttf");
    let pfd_bol = include_bytes!("../../../fonts/playfair_display_bold.ttf");

    Ok(LoadedFontSet {
        calibri: FamilyFonts {
            regular: parse_font(cal_reg, doc)?,
            bold: parse_font(cal_bol, doc)?,
            italic: None,
        },
        inter: FamilyFonts {
            regular: parse_font(int_reg, doc)?,
            bold: parse_font(int_bol, doc)?,
            italic: None,
        },
        source_serif4: FamilyFonts {
            regular: parse_font(ss4_reg, doc)?,
            bold: parse_font(ss4_bol, doc)?,
            italic: Some(parse_font(ss4_ita, doc)?),
        },
        manrope: FamilyFonts {
            regular: parse_font(man_reg, doc)?,
            bold: parse_font(man_bol, doc)?,
            italic: None,
        },
        jetbrains_mono: FamilyFonts {
            regular: parse_font(jbm_reg, doc)?,
            bold: parse_font(jbm_bol, doc)?,
            italic: None,
        },
        playfair_display: FamilyFonts {
            regular: parse_font(pfd_reg, doc)?,
            bold: parse_font(pfd_bol, doc)?,
            italic: None,
        },
    })
}

/// Resolve the (regular, bold, italic_opt) font IDs for a given family.
pub fn resolve_fonts(
    set: &LoadedFontSet,
    fam: FontFamily,
) -> (&FontId, &FontId, Option<&FontId>) {
    let f = set.family(fam);
    (&f.regular, &f.bold, f.italic.as_ref())
}

/// Bundles the four common render parameters so functions stay under the arg limit.
pub struct RenderCtx<'a> {
    pub template: &'a Template,
    pub layout: &'a LayoutConfig,
    pub colors: &'a ColorPalette,
    pub fonts: &'a LoadedFontSet,
}

// ─── Page state (multi-page support) ─────────────────────────────────────────

/// Tracks the current rendering position across multiple pages.
pub struct PageState {
    /// Completed pages (each is a Vec<Op> ready for PdfPage).
    pub pages: Vec<Vec<Op>>,
    /// Ops for the page currently being built.
    pub current_ops: Vec<Op>,
    /// Current Y position in mm (decreases as content is added).
    pub y: f32,
    /// Bottom margin in mm — content below this triggers a page break.
    pub bottom_margin: f32,
    /// Y position at the top of a fresh page.
    pub top_y: f32,
}

impl PageState {
    pub fn new(layout: &LayoutConfig) -> Self {
        let top_y = layout.page_height - pt_to_mm(layout.top_margin_pt);
        Self {
            pages: Vec::new(),
            current_ops: Vec::new(),
            y: top_y,
            bottom_margin: layout.margin_in * 25.4,
            top_y,
        }
    }

    /// Push all current ops to the completed pages list and reset y.
    pub fn new_page(&mut self) {
        let page_ops = std::mem::take(&mut self.current_ops);
        self.pages.push(page_ops);
        self.y = self.top_y;
    }

    /// True when y has fallen below the bottom margin.
    pub fn needs_break(&self) -> bool {
        self.y < self.bottom_margin
    }

    /// Consume the state and return all pages including the current in-progress page.
    pub fn finish(mut self) -> Vec<Vec<Op>> {
        self.pages.push(self.current_ops);
        self.pages
    }
}

/// Break before the next element if it (plus any required minimum tail) would not fit.
pub fn maybe_break_before(
    page_state: &mut PageState,
    estimated_height_mm: f32,
    min_remaining_mm: f32,
) {
    if page_state.y - estimated_height_mm - min_remaining_mm < page_state.bottom_margin {
        page_state.new_page();
    }
}

// ─── Color palette ────────────────────────────────────────────────────────────

pub struct ColorPalette {
    pub name: Color,
    pub section: Color,
    pub body: Color,
    pub date: Color,
    pub emphasis: Color,
    pub rule: Color,
}

/// Layout configuration
#[derive(Clone)]
pub struct LayoutConfig {
    pub margin_left: f32,
    pub margin_right: f32,
    pub page_width: f32,
    pub page_height: f32,
    pub line_height: f32,
    pub top_margin_pt: f32,
    pub margin_in: f32,
    // Two-column fields (0.0 = single-column)
    pub sidebar_width: f32,
    pub main_x: f32,
    pub main_width: f32,
}

/// Convert RGB tuple to printpdf Color.
pub fn rgb_to_color(rgb: (u8, u8, u8)) -> Color {
    Color::Rgb(Rgb::new(
        rgb.0 as f32 / 255.0,
        rgb.1 as f32 / 255.0,
        rgb.2 as f32 / 255.0,
        None,
    ))
}

/// Convert points to millimeters.
pub fn pt_to_mm(pt: f32) -> f32 {
    pt / 2.834_645_7
}

/// Setup color palette from template.
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

/// Setup layout configuration from template.
pub fn setup_layout(template: &Template) -> LayoutConfig {
    const MM_PER_INCH: f32 = 25.4;
    const PT_PER_MM: f32 = 2.834_645_7;

    let margin_left = template.margin_in * MM_PER_INCH;
    let margin_right = template.margin_in * MM_PER_INCH;
    let line_height = (template.body_pt / PT_PER_MM) * template.line_spacing;

    let (sidebar_width, main_x, main_width) = if let Some(tc) = &template.two_column {
        let content_w = 210.0 - margin_left - margin_right;
        let sb = content_w * tc.sidebar_width_ratio;
        let gap = 5.0; // mm gap between columns
        let mx = margin_left + sb + gap;
        let mw = 210.0 - margin_right - mx;
        (sb, mx, mw)
    } else {
        (0.0, margin_left, 210.0 - margin_left - margin_right)
    };

    LayoutConfig {
        margin_left,
        margin_right,
        page_width: 210.0,
        page_height: 297.0,
        line_height,
        top_margin_pt: 54.0, // 0.75 inch
        margin_in: template.margin_in,
        sidebar_width,
        main_x,
        main_width,
    }
}

// ─── Low-level drawing helpers ────────────────────────────────────────────────

pub fn build_line(x1: f32, y1: f32, x2: f32, color: Color, thickness: f32) -> Vec<Op> {
    if thickness <= 0.0 {
        return Vec::new();
    }
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

/// Draw the sidebar background rectangle from header_bottom_y to the bottom margin.
pub fn draw_sidebar_bg(layout: &LayoutConfig, bg_color: (u8, u8, u8), header_bottom_y: f32, bottom_margin: f32) -> Vec<Op> {
    let color = rgb_to_color(bg_color);
    let rect_x = layout.margin_left;
    let rect_y = bottom_margin;
    let rect_w = layout.sidebar_width;
    let rect_h = header_bottom_y - bottom_margin;
    if rect_w <= 0.0 || rect_h <= 0.0 {
        return Vec::new();
    }
    let polygon = Polygon {
        rings: vec![PolygonRing { points: vec![
            LinePoint { p: Point::new(Mm(rect_x), Mm(rect_y)), bezier: false },
            LinePoint { p: Point::new(Mm(rect_x + rect_w), Mm(rect_y)), bezier: false },
            LinePoint { p: Point::new(Mm(rect_x + rect_w), Mm(rect_y + rect_h)), bezier: false },
            LinePoint { p: Point::new(Mm(rect_x), Mm(rect_y + rect_h)), bezier: false },
        ]}],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::EvenOdd,
    };
    vec![
        Op::SetFillColor { col: color },
        Op::DrawPolygon { polygon },
    ]
}

// ─── Text building helpers ────────────────────────────────────────────────────

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
    let text_pos = Point { x: Mm(config.x).into(), y: Mm(config.y).into() };
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor { pos: text_pos });

    for segment in segments {
        let font_id = if segment.bold { config.font_bold.clone() } else { config.font_regular.clone() };
        let color = if segment.bold { config.bold_color.clone() } else { config.normal_color.clone() };

        ops.push(Op::SetFillColor { col: color });
        ops.push(Op::SetFont { font: PdfFontHandle::External(font_id), size: Pt(config.font_size) });
        ops.push(Op::ShowText { items: vec![TextItem::Text(segment.text.clone())] });
    }
    ops.push(Op::EndTextSection);
    ops
}

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

// ─── Shared letterhead helper ─────────────────────────────────────────────────

/// Render the name + contact + accent rule header — shared between resume and cover letter.
/// When both documents use the same template the top 1.5" is pixel-identical.
pub fn render_letterhead(
    ctx: &RenderCtx<'_>,
    name: &str,
    contact_line: &str,
    style: CoverLetterHeader,
    y: f32,
) -> (Vec<Op>, f32) {
    let (_, name_bold, _) = resolve_fonts(ctx.fonts, ctx.template.fonts.name_family);
    let (body_reg, _, _) = resolve_fonts(ctx.fonts, ctx.template.fonts.body_family);

    let mut ops = Vec::new();
    let mut current_y = y;

    let template = ctx.template;
    let layout = ctx.layout;
    let colors = ctx.colors;

    match style {
        CoverLetterHeader::Compact => {
            // Name only — single bold line, no contact, no rule
            let x = if template.name_centered {
                layout.page_width / 2.0 - (name.len() as f32 * pt_to_mm(template.name_pt) * 0.3)
            } else {
                layout.margin_left
            };
            let text_pos = Point { x: Mm(x).into(), y: Mm(current_y).into() };
            ops.extend([
                Op::StartTextSection,
                Op::SetFillColor { col: colors.name.clone() },
                Op::SetTextCursor { pos: text_pos },
                Op::SetFont { font: PdfFontHandle::External(name_bold.clone()), size: Pt(template.name_pt) },
                Op::ShowText { items: vec![TextItem::Text(name.to_string())] },
                Op::EndTextSection,
            ]);
            current_y -= pt_to_mm(template.name_pt) * 1.2;
        }

        CoverLetterHeader::Centered => {
            // Name centered in display type, no rule
            let x = layout.page_width / 2.0 - (name.len() as f32 * pt_to_mm(template.name_pt) * 0.3);
            let text_pos = Point { x: Mm(x).into(), y: Mm(current_y).into() };
            ops.extend([
                Op::StartTextSection,
                Op::SetFillColor { col: colors.name.clone() },
                Op::SetTextCursor { pos: text_pos },
                Op::SetFont { font: PdfFontHandle::External(name_bold.clone()), size: Pt(template.name_pt) },
                Op::ShowText { items: vec![TextItem::Text(name.to_string())] },
                Op::EndTextSection,
            ]);
            current_y -= pt_to_mm(template.name_pt) * 1.2;
        }

        CoverLetterHeader::Matched | CoverLetterHeader::Letterhead => {
            // Full block: name + contact line + hairline rule
            let name_x = if template.name_centered {
                layout.page_width / 2.0 - (name.len() as f32 * pt_to_mm(template.name_pt) * 0.3)
            } else {
                layout.margin_left
            };
            let text_pos = Point { x: Mm(name_x).into(), y: Mm(current_y).into() };
            ops.extend([
                Op::StartTextSection,
                Op::SetFillColor { col: colors.name.clone() },
                Op::SetTextCursor { pos: text_pos },
                Op::SetFont { font: PdfFontHandle::External(name_bold.clone()), size: Pt(template.name_pt) },
                Op::ShowText { items: vec![TextItem::Text(name.to_string())] },
                Op::EndTextSection,
            ]);
            current_y -= pt_to_mm(template.name_pt) * 1.2;

            if !contact_line.is_empty() {
                let contact_x = if template.name_centered {
                    layout.page_width / 2.0 - (contact_line.len() as f32 * pt_to_mm(9.0) * 0.3)
                } else {
                    layout.margin_left
                };
                let contact_pos = Point { x: Mm(contact_x).into(), y: Mm(current_y).into() };
                ops.extend([
                    Op::StartTextSection,
                    Op::SetFillColor { col: colors.date.clone() },
                    Op::SetTextCursor { pos: contact_pos },
                    Op::SetFont { font: PdfFontHandle::External(body_reg.clone()), size: Pt(9.0) },
                    Op::ShowText { items: vec![TextItem::Text(contact_line.to_string())] },
                    Op::EndTextSection,
                ]);
                current_y -= pt_to_mm(9.0) * 1.2;
            }

            // Hairline rule — thickness from template (e.g. 0.25 pt for Editorial Serif)
            let rule_thickness = if template.rule_thickness > 0.0 { template.rule_thickness } else { 0.5 };
            ops.extend(build_line(
                layout.margin_left,
                current_y,
                layout.page_width - layout.margin_right,
                colors.rule.clone(),
                rule_thickness,
            ));
            current_y -= pt_to_mm(14.0);
        }
    }

    (ops, current_y)
}

// ─── Signature block ──────────────────────────────────────────────────────────

pub fn render_signature(
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFontSet,
    name: &str,
    title: Option<&str>,
    y: f32,
) -> (Vec<Op>, f32) {
    use crate::export::templates::SignatureStyle;
    let (body_reg, body_bold, _) = resolve_fonts(fonts, template.fonts.body_family);
    let (name_reg, _, name_italic_opt) = resolve_fonts(fonts, template.fonts.name_family);

    let mut ops = Vec::new();
    let mut current_y = y;

    match template.cover_letter.signature_block {
        SignatureStyle::TypedOnly => {
            let pos = Point { x: Mm(layout.margin_left).into(), y: Mm(current_y).into() };
            ops.extend([
                Op::StartTextSection,
                Op::SetFillColor { col: colors.body.clone() },
                Op::SetTextCursor { pos },
                Op::SetFont { font: PdfFontHandle::External(body_reg.clone()), size: Pt(template.body_pt) },
                Op::ShowText { items: vec![TextItem::Text(name.to_string())] },
                Op::EndTextSection,
            ]);
            current_y -= pt_to_mm(template.body_pt) * 1.2;
        }

        SignatureStyle::NameAndTitle => {
            let pos = Point { x: Mm(layout.margin_left).into(), y: Mm(current_y).into() };
            ops.extend([
                Op::StartTextSection,
                Op::SetFillColor { col: colors.body.clone() },
                Op::SetTextCursor { pos },
                Op::SetFont { font: PdfFontHandle::External(body_bold.clone()), size: Pt(template.body_pt) },
                Op::ShowText { items: vec![TextItem::Text(name.to_string())] },
                Op::EndTextSection,
            ]);
            current_y -= pt_to_mm(template.body_pt) * 1.2;
            if let Some(t) = title {
                let tpos = Point { x: Mm(layout.margin_left).into(), y: Mm(current_y).into() };
                ops.extend([
                    Op::StartTextSection,
                    Op::SetFillColor { col: colors.date.clone() },
                    Op::SetTextCursor { pos: tpos },
                    Op::SetFont { font: PdfFontHandle::External(body_reg.clone()), size: Pt(9.0) },
                    Op::ShowText { items: vec![TextItem::Text(t.to_string())] },
                    Op::EndTextSection,
                ]);
                current_y -= pt_to_mm(9.0) * 1.2;
            }
        }

        SignatureStyle::ScriptStyle => {
            // Use name_family italic if available, else fall back to TypedOnly
            let font_id = name_italic_opt.unwrap_or(name_reg);
            let pos = Point { x: Mm(layout.margin_left).into(), y: Mm(current_y).into() };
            ops.extend([
                Op::StartTextSection,
                Op::SetFillColor { col: colors.body.clone() },
                Op::SetTextCursor { pos },
                Op::SetFont { font: PdfFontHandle::External(font_id.clone()), size: Pt(template.body_pt + 2.0) },
                Op::ShowText { items: vec![TextItem::Text(name.to_string())] },
                Op::EndTextSection,
            ]);
            current_y -= pt_to_mm(template.body_pt + 2.0) * 1.2;
            if let Some(t) = title {
                let tpos = Point { x: Mm(layout.margin_left).into(), y: Mm(current_y).into() };
                ops.extend([
                    Op::StartTextSection,
                    Op::SetFillColor { col: colors.date.clone() },
                    Op::SetTextCursor { pos: tpos },
                    Op::SetFont { font: PdfFontHandle::External(body_reg.clone()), size: Pt(9.0) },
                    Op::ShowText { items: vec![TextItem::Text(t.to_string())] },
                    Op::EndTextSection,
                ]);
                current_y -= pt_to_mm(9.0) * 1.2;
            }
        }
    }

    (ops, current_y)
}

// ─── Resume render helpers ────────────────────────────────────────────────────

/// Render name line using template's name_family.
pub fn render_name_line(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFontSet,
    y: f32,
) -> (Vec<Op>, f32) {
    let (_, bold_id, _) = resolve_fonts(fonts, template.fonts.name_family);
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
        Op::SetFont { font: PdfFontHandle::External(bold_id.clone()), size: Pt(template.name_pt) },
        Op::ShowText { items: vec![TextItem::Text(name_text.to_string())] },
        Op::EndTextSection,
    ];

    let new_y = y - pt_to_mm(template.name_pt) * 1.2;
    (ops, new_y)
}

/// Render contact line with separator rule.
/// URLs are rendered in hyperlink blue and annotated with clickable link rectangles.
pub fn render_contact_line(
    text: &str,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFontSet,
    y: f32,
) -> (Vec<Op>, f32) {
    let (reg_id, _, _) = resolve_fonts(fonts, template.fonts.body_family);
    let font_size = 9.0_f32;
    // Approximate character width for this font size
    let char_w = pt_to_mm(font_size) * 0.52;

    let x_start = if template.name_centered {
        layout.page_width / 2.0 - (text.len() as f32 * char_w * 0.5)
    } else {
        layout.margin_left
    };

    let spans = split_urls(text);
    let mut ops = Vec::new();
    let mut cursor_x = x_start;

    for span in &spans {
        match span {
            Span::Text(t) => {
                if t.is_empty() { continue; }
                let pos = Point { x: Mm(cursor_x).into(), y: Mm(y).into() };
                ops.extend([
                    Op::StartTextSection,
                    Op::SetFillColor { col: colors.date.clone() },
                    Op::SetTextCursor { pos },
                    Op::SetFont { font: PdfFontHandle::External(reg_id.clone()), size: Pt(font_size) },
                    Op::ShowText { items: vec![TextItem::Text(t.clone())] },
                    Op::EndTextSection,
                ]);
                cursor_x += t.len() as f32 * char_w;
            }
            Span::Link { label, url } => {
                let link_w = label.len() as f32 * char_w;
                let pos = Point { x: Mm(cursor_x).into(), y: Mm(y).into() };
                ops.extend([
                    Op::StartTextSection,
                    Op::SetFillColor { col: colors.date.clone() },
                    Op::SetTextCursor { pos },
                    Op::SetFont { font: PdfFontHandle::External(reg_id.clone()), size: Pt(font_size) },
                    Op::ShowText { items: vec![TextItem::Text(label.clone())] },
                    Op::EndTextSection,
                ]);
                // Annotation rect in PDF points (bottom-left origin)
                // PDF y=0 is bottom; our y is mm from top of page
                let page_h_pt = layout.page_height * 2.834_645_7;
                let rect_y_bottom = page_h_pt - (y * 2.834_645_7);
                let rect_y_top = rect_y_bottom + font_size * 1.1;
                let rect_x_left = cursor_x * 2.834_645_7;
                let rect_x_right = rect_x_left + link_w * 2.834_645_7;
                let rect = Rect {
                    x: Pt(rect_x_left),
                    y: Pt(rect_y_bottom),
                    width: Pt(rect_x_right - rect_x_left),
                    height: Pt(rect_y_top - rect_y_bottom),
                    mode: None,
                    winding_order: None,
                };
                ops.push(Op::LinkAnnotation {
                    link: LinkAnnotation::new(
                        rect,
                        Actions::Uri(url.clone()),
                        Some(BorderArray::Solid([0.0, 0.0, 0.0])),
                        Some(ColorArray::Transparent),
                        None,
                    ),
                });
                cursor_x += link_w;
            }
        }
    }

    let y_after_text = y - pt_to_mm(font_size) * 1.2;
    let rule_thickness = if template.rule_thickness > 0.0 { template.rule_thickness } else { 0.5 };
    ops.extend(build_line(
        layout.margin_left,
        y_after_text,
        layout.page_width - layout.margin_right,
        colors.rule.clone(),
        rule_thickness,
    ));

    let new_y = y_after_text - pt_to_mm(5.0);
    (ops, new_y)
}

/// Render section header. Respects section_small_caps and rule_thickness.
pub fn render_section_header(
    text: &str,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFontSet,
    y: f32,
) -> (Vec<Op>, f32) {
    let (_, bold_id, _) = resolve_fonts(fonts, template.fonts.heading_family);

    let y_before = y - pt_to_mm(4.0);

    let (header_text, effective_pt) = if template.section_small_caps {
        (text.to_uppercase(), template.section_pt * 0.85)
    } else if template.section_all_caps {
        (text.to_uppercase(), template.section_pt)
    } else {
        (text.to_string(), template.section_pt)
    };

    let text_pos = Point { x: Mm(layout.margin_left).into(), y: Mm(y_before).into() };
    let mut ops = vec![
        Op::StartTextSection,
        Op::SetFillColor { col: colors.section.clone() },
        Op::SetTextCursor { pos: text_pos },
        Op::SetFont { font: PdfFontHandle::External(bold_id.clone()), size: Pt(effective_pt) },
        Op::ShowText { items: vec![TextItem::Text(header_text)] },
        Op::EndTextSection,
    ];

    let y_after_header = y_before - pt_to_mm(effective_pt) * 1.2;

    match template.section_style {
        SectionStyle::RuledBottom | SectionStyle::Underline => {
            let thickness = if template.rule_thickness > 0.0 { template.rule_thickness } else { 0.5 };
            ops.extend(build_line(
                layout.margin_left,
                y_after_header,
                layout.page_width - layout.margin_right,
                colors.rule.clone(),
                thickness,
            ));
        }
        SectionStyle::BoldOnly => {}
    }

    let new_y = y_after_header - pt_to_mm(10.0);
    (ops, new_y)
}

/// Render job entry with optional right-aligned date.
pub fn render_job_entry(
    segments: &[TextSegment],
    date: Option<&str>,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFontSet,
    y: f32,
) -> (Vec<Op>, f32) {
    let (reg_id, bold_id, _) = resolve_fonts(fonts, template.fonts.body_family);

    let mut ops = build_text_ops(
        segments,
        TextOpsConfig {
            x: layout.main_x,
            y,
            font_regular: reg_id.clone(),
            font_bold: bold_id.clone(),
            font_size: template.body_pt,
            normal_color: colors.body.clone(),
            bold_color: colors.emphasis.clone(),
        },
    );

    if let Some(date) = date {
        let date_x = layout.page_width - layout.margin_right - (date.len() as f32 * pt_to_mm(9.0) * 0.3);
        let text_pos = Point { x: Mm(date_x).into(), y: Mm(y).into() };
        ops.extend([
            Op::StartTextSection,
            Op::SetFillColor { col: colors.date.clone() },
            Op::SetTextCursor { pos: text_pos },
            Op::SetFont { font: PdfFontHandle::External(reg_id.clone()), size: Pt(9.0) },
            Op::ShowText { items: vec![TextItem::Text(date.to_string())] },
            Op::EndTextSection,
        ]);
    }

    let new_y = y - layout.line_height + pt_to_mm(2.0);
    (ops, new_y)
}

/// Render job title, using italic if available and `job_title_italic` is set.
pub fn render_job_title(
    text: &str,
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFontSet,
    y: f32,
) -> (Vec<Op>, f32) {
    let (reg_id, _, italic_opt) = resolve_fonts(fonts, template.fonts.body_family);
    let font_id = if template.job_title_italic { italic_opt.unwrap_or(reg_id) } else { reg_id };

    let text_pos = Point { x: Mm(layout.main_x).into(), y: Mm(y).into() };
    let ops = vec![
        Op::StartTextSection,
        Op::SetFillColor { col: colors.date.clone() },
        Op::SetTextCursor { pos: text_pos },
        Op::SetFont { font: PdfFontHandle::External(font_id.clone()), size: Pt(template.body_pt - 0.5) },
        Op::ShowText { items: vec![TextItem::Text(text.to_string())] },
        Op::EndTextSection,
    ];

    (ops, y - layout.line_height)
}

/// Render bullet point with wrapped text.
pub fn render_bullet_line(
    segments: &[TextSegment],
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFontSet,
    y: f32,
) -> (Vec<Op>, f32) {
    let (reg_id, bold_id, _) = resolve_fonts(fonts, template.fonts.body_family);

    let mut ops = Vec::new();

    let bullet_pos = Point { x: Mm(layout.main_x + 1.3).into(), y: Mm(y).into() };
    ops.extend([
        Op::StartTextSection,
        Op::SetFillColor { col: colors.body.clone() },
        Op::SetTextCursor { pos: bullet_pos },
        Op::SetFont { font: PdfFontHandle::External(reg_id.clone()), size: Pt(template.body_pt) },
        Op::ShowText { items: vec![TextItem::Text("•".to_string())] },
        Op::EndTextSection,
    ]);

    let bullet_content_width = layout.main_width - 4.0;
    let mut current_y = y;
    for seg_line in wrap_segments(segments, bullet_content_width, template.body_pt) {
        ops.extend(build_text_ops(
            &seg_line,
            TextOpsConfig {
                x: layout.main_x + 4.0,
                y: current_y,
                font_regular: reg_id.clone(),
                font_bold: bold_id.clone(),
                font_size: template.body_pt,
                normal_color: colors.body.clone(),
                bold_color: colors.emphasis.clone(),
            },
        ));
        current_y -= layout.line_height;
    }

    (ops, current_y)
}

/// Render plain text with wrapping.
pub fn render_text_line(
    segments: &[TextSegment],
    template: &Template,
    layout: &LayoutConfig,
    colors: &ColorPalette,
    fonts: &LoadedFontSet,
    y: f32,
) -> (Vec<Op>, f32) {
    let (reg_id, bold_id, _) = resolve_fonts(fonts, template.fonts.body_family);

    let mut ops = Vec::new();
    let mut current_y = y;

    for seg_line in wrap_segments(segments, layout.main_width, template.body_pt) {
        ops.extend(build_text_ops(
            &seg_line,
            TextOpsConfig {
                x: layout.main_x,
                y: current_y,
                font_regular: reg_id.clone(),
                font_bold: bold_id.clone(),
                font_size: template.body_pt,
                normal_color: colors.body.clone(),
                bold_color: colors.emphasis.clone(),
            },
        ));
        current_y -= layout.line_height;
    }

    (ops, current_y)
}

// ─── Cover-letter paragraph rendering ────────────────────────────────────────

/// Estimate how many mm a wrapped paragraph will occupy.
pub fn estimate_paragraph_height(
    text: &str,
    content_width: f32,
    font_size_pt: f32,
    line_height: f32,
    first_line_indent_mm: f32,
) -> f32 {
    let segs = vec![TextSegment { text: text.to_string(), bold: false }];
    let wrapped_width = if first_line_indent_mm > 0.0 { content_width - first_line_indent_mm } else { content_width };
    // First line at reduced width, rest at full width
    let wrapped = wrap_segments(&segs, content_width.min(wrapped_width), font_size_pt);
    wrapped.len() as f32 * line_height
}

/// Render a cover-letter paragraph with the template's paragraph_indent style.
/// Returns (ops, new_y).
pub fn render_cover_letter_paragraph(
    ctx: &RenderCtx<'_>,
    text: &str,
    y: f32,
    content_width: f32,
    x_offset: f32,
) -> (Vec<Op>, f32) {
    use crate::export::templates::ParagraphIndent;
    let template = ctx.template;
    let layout = ctx.layout;
    let colors = ctx.colors;
    let (reg_id, bold_id, _) = resolve_fonts(ctx.fonts, template.fonts.body_family);
    let segs = crate::export::parser::parse_inline_md(text);

    let mut ops = Vec::new();
    let mut current_y = y;

    let first_line_indent = match template.cover_letter.paragraph_indent {
        ParagraphIndent::FirstLine => 6.35, // 0.25 in
        ParagraphIndent::BlockNoIndent => 0.0,
    };

    let wrapped = wrap_segments(&segs, content_width, template.body_pt);
    let last_idx = wrapped.len().saturating_sub(1);

    for (i, seg_line) in wrapped.into_iter().enumerate() {
        let line_x = if i == 0 { x_offset + first_line_indent } else { x_offset };
        ops.extend(build_text_ops(
            &seg_line,
            TextOpsConfig {
                x: line_x,
                y: current_y,
                font_regular: reg_id.clone(),
                font_bold: bold_id.clone(),
                font_size: template.body_pt,
                normal_color: colors.body.clone(),
                bold_color: colors.emphasis.clone(),
            },
        ));

        let extra = if i == last_idx {
            match template.cover_letter.paragraph_indent {
                ParagraphIndent::BlockNoIndent => pt_to_mm(template.cover_letter.paragraph_spacing_pt),
                ParagraphIndent::FirstLine => 0.0,
            }
        } else {
            0.0
        };
        current_y -= layout.line_height + extra;
    }

    (ops, current_y)
}


#[cfg(test)]
mod test;
