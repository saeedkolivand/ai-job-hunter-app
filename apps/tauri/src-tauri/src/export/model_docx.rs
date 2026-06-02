//! DOCX backend for the canonical document model (Phase 5).
//!
//! Builds a [`DocumentModel`] from resume text and translates it to a `docx-rs`
//! document. Unlike the legacy DOCX path (which re-parses text and always
//! collapses to one column), this
//! backend renders a genuine **two-column** layout as a borderless, single-row
//! two-cell table (shaded sidebar cell + main cell) and honors `ats_mode`
//! natively by linearizing the model to a single column before rendering.
//!
//! Wired into [`super::docx::generate_docx`] behind the `model_docx` Cargo
//! feature. Reuses the Phase-3 font fallback and page-size helpers from
//! [`super::docx_renderer`]; cover letters stay on the legacy path until they are
//! modeled explicitly.

use anyhow::Result;
use docx_rs::*;

use crate::export::docx_renderer::{
    create_bullet_numbering, docx_run_fonts, inch_to_dxa, mm_to_dxa, pt_to_dxa, rgb_to_hex,
    setup_colors, DocxColors,
};
use crate::export::templates::Template;
use crate::export::types::{FontFamily, GenerationMeta};
use crate::locale::PageGeometry;
use crate::model::adapter::model_from_resume_text;
use crate::model::document::{Block, DocumentModel, EntryBlock, HeaderBlock, Placement, Section};
use crate::model::rich::RichText;
use crate::model::transform;
use crate::theme::{self, LinkStyle};

/// Per-flow rendering context (full page width, or a single table cell).
struct Ctx<'a> {
    template: &'a Template,
    colors: &'a DocxColors,
    link: LinkStyle,
    /// Width of this flow in dxa — used to right-align entry dates.
    width_dxa: usize,
    /// Right-align entry dates with a tab (false inside the narrow sidebar).
    right_align_date: bool,
}

/// Render a resume to a `Docx` via the canonical document model, on the default
/// (international A4) page geometry. A test convenience — the export command uses
/// [`generate_resume_docx_in`] with the request's locale geometry.
#[cfg(test)]
pub(crate) fn generate_resume_docx(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    ats_mode: bool,
) -> Result<Docx> {
    generate_resume_docx_in(
        text,
        meta,
        template,
        ats_mode,
        crate::locale::LocaleProfile::default().page_geometry(),
        None,
        "en",
    )
}

/// Render a resume to a `Docx` on a specific page geometry (locale-driven).
///
/// `contact` (when present) is the single source of truth for the header contact
/// line, localized by `lang` — shared with the PDF backend so both documents'
/// headers carry identical, correctly-named links.
pub(crate) fn generate_resume_docx_in(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    ats_mode: bool,
    geom: PageGeometry,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
) -> Result<Docx> {
    let mut model = model_from_resume_text(text);

    if let Some(name) = meta.and_then(|m| m.candidate_name.as_deref()) {
        if !name.trim().is_empty() {
            model.header.name = name.to_string();
        }
    }

    if let Some(profile) = contact {
        profile.apply_to_header(&mut model.header, lang);
    }

    // ATS mode collapses to a single column and reorders sections for reading.
    let two_column = template.two_column.is_some() && !ats_mode;
    if ats_mode {
        transform::linearize(&mut model);
    }

    let colors = setup_colors(template);
    let (abstract_num, num) = create_bullet_numbering();
    let mut docx = Docx::new()
        .add_abstract_numbering(abstract_num)
        .add_numbering(num);

    // Header spans the full width, above any columns.
    docx = add_header(docx, &model.header, template, &colors);

    if two_column {
        docx = add_two_column_body(docx, &model, template, &colors, geom);
    } else {
        let ctx = Ctx {
            template,
            colors: &colors,
            link: theme::link_style(template.id),
            width_dxa: content_width_dxa(template, geom),
            right_align_date: true,
        };
        for section in &model.sections {
            for para in section_paragraphs(section, &ctx) {
                docx = docx.add_paragraph(para);
            }
        }
    }

    let page_margin = PageMargin::new()
        .top(inch_to_dxa(0.9))
        .bottom(inch_to_dxa(0.9))
        .left(inch_to_dxa(template.margin_in))
        .right(inch_to_dxa(template.margin_in));

    docx = docx
        .page_size(mm_to_dxa(geom.width_mm), mm_to_dxa(geom.height_mm))
        .page_margin(page_margin);

    Ok(docx)
}

/// Printable width (page minus both margins) in dxa for the given geometry.
fn content_width_dxa(template: &Template, geom: PageGeometry) -> usize {
    let page = mm_to_dxa(geom.width_mm) as i64;
    let margin = inch_to_dxa(template.margin_in) as i64;
    (page - 2 * margin).max(1) as usize
}

// ─── Header ───────────────────────────────────────────────────────────────────

fn add_header(mut docx: Docx, header: &HeaderBlock, t: &Template, colors: &DocxColors) -> Docx {
    if !header.name.is_empty() {
        let mut p = Paragraph::new().add_run(
            Run::new()
                .add_text(&header.name)
                .size(pt_to_dxa(t.name_pt))
                .bold()
                .color(colors.name.as_str())
                .fonts(docx_run_fonts(t.fonts.name_family)),
        );
        if t.name_centered {
            p = p.align(AlignmentType::Center);
        }
        docx = docx.add_paragraph(p);
    }

    if let Some(title) = &header.title {
        let mut p = Paragraph::new().add_run(
            Run::new()
                .add_text(title)
                .size(pt_to_dxa(t.body_pt))
                .color(colors.date.as_str())
                .fonts(docx_run_fonts(t.fonts.body_family)),
        );
        if t.name_centered {
            p = p.align(AlignmentType::Center);
        }
        docx = docx.add_paragraph(p);
    }

    if !header.contact.is_empty() {
        let opts = RunOpts::contact(t, colors);
        let mut p = add_rich(Paragraph::new(), &header.contact, &opts)
            .line_spacing(LineSpacing::new().after(120));
        if t.name_centered {
            p = p.align(AlignmentType::Center);
        }
        docx = docx.add_paragraph(p);
    }

    docx
}

// ─── Two-column body (borderless shaded table) ────────────────────────────────

fn add_two_column_body(
    mut docx: Docx,
    model: &DocumentModel,
    template: &Template,
    colors: &DocxColors,
    geom: PageGeometry,
) -> Docx {
    let tc = template
        .two_column
        .as_ref()
        .expect("add_two_column_body requires a two-column config");

    let content = content_width_dxa(template, geom);
    let sidebar_w = (content as f32 * tc.sidebar_width_ratio) as usize;
    let main_w = content.saturating_sub(sidebar_w).max(1);

    let mut sidebar_paras = Vec::new();
    let mut main_paras = Vec::new();
    let sidebar_ctx = Ctx {
        template,
        colors,
        link: theme::link_style(template.id),
        width_dxa: sidebar_w,
        right_align_date: false,
    };
    let main_ctx = Ctx {
        template,
        colors,
        link: theme::link_style(template.id),
        width_dxa: main_w,
        right_align_date: true,
    };
    for section in &model.sections {
        match theme::placement_for(&section.id) {
            Placement::Sidebar => sidebar_paras.extend(section_paragraphs(section, &sidebar_ctx)),
            Placement::Main => main_paras.extend(section_paragraphs(section, &main_ctx)),
        }
    }

    // A table cell must hold at least one block-level element.
    if sidebar_paras.is_empty() {
        sidebar_paras.push(Paragraph::new());
    }
    if main_paras.is_empty() {
        main_paras.push(Paragraph::new());
    }

    let mut sidebar_cell = TableCell::new()
        .width(sidebar_w, WidthType::Dxa)
        .set_borders(TableCellBorders::new().clear_all())
        .vertical_align(VAlignType::Top)
        .shading(
            Shading::new()
                .shd_type(ShdType::Clear)
                .color("auto")
                .fill(rgb_to_hex(tc.sidebar_bg_color)),
        );
    for p in sidebar_paras {
        sidebar_cell = sidebar_cell.add_paragraph(p);
    }

    let mut main_cell = TableCell::new()
        .width(main_w, WidthType::Dxa)
        .set_borders(TableCellBorders::new().clear_all())
        .vertical_align(VAlignType::Top);
    for p in main_paras {
        main_cell = main_cell.add_paragraph(p);
    }

    let table = Table::new(vec![TableRow::new(vec![sidebar_cell, main_cell])])
        .set_grid(vec![sidebar_w, main_w])
        .layout(TableLayoutType::Fixed)
        .width(content, WidthType::Dxa)
        .clear_all_border();

    docx = docx.add_table(table);
    docx
}

// ─── Section / block → paragraphs ─────────────────────────────────────────────

fn section_paragraphs(section: &Section, ctx: &Ctx) -> Vec<Paragraph> {
    let mut out = Vec::new();
    if let Some(h) = heading_paragraph(&section.heading, ctx) {
        out.push(h);
    }
    for block in &section.blocks {
        match block {
            Block::Paragraph(rt) => out.push(body_paragraph(rt, ctx)),
            Block::Bullet(rt) => out.push(bullet_paragraph(rt, ctx)),
            Block::Entry(e) => out.extend(entry_paragraphs(e, ctx)),
        }
    }
    out
}

fn heading_paragraph(heading: &str, ctx: &Ctx) -> Option<Paragraph> {
    if heading.is_empty() {
        return None;
    }
    let t = ctx.template;
    let (text, pt) = if t.section_small_caps {
        (heading.to_uppercase(), t.section_pt * 0.85)
    } else if t.section_all_caps {
        (heading.to_uppercase(), t.section_pt)
    } else {
        (heading.to_string(), t.section_pt)
    };
    let char_spacing = if t.section_small_caps || t.section_all_caps {
        30
    } else {
        0
    };

    Some(
        Paragraph::new()
            .add_run(
                Run::new()
                    .add_text(&text)
                    .size(pt_to_dxa(pt))
                    .bold()
                    .color(ctx.colors.section.as_str())
                    .fonts(docx_run_fonts(t.fonts.heading_family))
                    .character_spacing(char_spacing),
            )
            .line_spacing(
                LineSpacing::new()
                    .before(pt_to_dxa(t.section_spacing_before) as u32)
                    .after(60),
            )
            // Keep a heading with the content that follows it (no orphaned header
            // at the foot of a page/column).
            .keep_next(true),
    )
}

fn body_paragraph(rt: &RichText, ctx: &Ctx) -> Paragraph {
    add_rich(
        Paragraph::new(),
        rt,
        &RunOpts::body(ctx.template, ctx.colors, ctx.link),
    )
    .keep_lines(true)
}

fn bullet_paragraph(rt: &RichText, ctx: &Ctx) -> Paragraph {
    let para = Paragraph::new().indent(
        Some(inch_to_dxa(0.2)),
        Some(SpecialIndentType::Hanging(inch_to_dxa(0.2))),
        None,
        None,
    );
    add_rich(para, rt, &RunOpts::body(ctx.template, ctx.colors, ctx.link))
        .numbering(NumberingId::new(1), IndentLevel::new(0))
        .keep_lines(true)
}

fn entry_paragraphs(e: &EntryBlock, ctx: &Ctx) -> Vec<Paragraph> {
    let t = ctx.template;
    let mut out = Vec::new();

    // Title line — bold — with the date either right-aligned (wide flows) or
    // appended inline (the narrow sidebar).
    let mut title = add_rich(
        Paragraph::new(),
        &e.title,
        &RunOpts::entry_title(t, ctx.colors),
    );
    if let Some(date) = &e.date {
        if ctx.right_align_date {
            title = title
                .add_run(Run::new().add_tab())
                .add_run(
                    Run::new()
                        .add_text(date)
                        .size(pt_to_dxa(9.5))
                        .color(ctx.colors.date.as_str())
                        .fonts(docx_run_fonts(t.fonts.body_family)),
                )
                .add_tab(Tab::new().val(TabValueType::Right).pos(ctx.width_dxa));
        } else {
            title = title.add_run(
                Run::new()
                    .add_text(format!("  ·  {date}"))
                    .size(pt_to_dxa(9.5))
                    .color(ctx.colors.date.as_str())
                    .fonts(docx_run_fonts(t.fonts.body_family)),
            );
        }
    }
    // Keep the entry title with its subtitle / first bullet.
    out.push(title.keep_next(true));

    if let Some(subtitle) = &e.subtitle {
        let para = add_rich(
            Paragraph::new(),
            subtitle,
            &RunOpts::subtitle(t, ctx.colors),
        )
        .line_spacing(LineSpacing::new().after(60))
        .keep_next(true);
        out.push(para);
    }

    for bullet in &e.bullets {
        out.push(bullet_paragraph(bullet, ctx));
    }

    out
}

// ─── Rich text → runs / hyperlinks ────────────────────────────────────────────

/// Styling for a run of rich text. Built per context so the same `add_rich`
/// walker handles header contact, body, bold entry titles, and italic subtitles.
struct RunOpts {
    size: usize,
    color: String,
    link_color: String,
    underline: bool,
    family: FontFamily,
    force_bold: bool,
    force_italic: bool,
}

impl RunOpts {
    fn link_color(colors: &DocxColors, link: LinkStyle) -> String {
        if link.use_accent {
            colors.emphasis.clone()
        } else {
            colors.body.clone()
        }
    }

    fn contact(t: &Template, colors: &DocxColors) -> Self {
        let link = theme::link_style(t.id);
        Self {
            size: pt_to_dxa(9.0),
            color: colors.date.clone(),
            link_color: Self::link_color(colors, link),
            underline: link.underline,
            family: t.fonts.body_family,
            force_bold: false,
            force_italic: false,
        }
    }

    fn body(t: &Template, colors: &DocxColors, link: LinkStyle) -> Self {
        Self {
            size: pt_to_dxa(t.body_pt),
            color: colors.body.clone(),
            link_color: Self::link_color(colors, link),
            underline: link.underline,
            family: t.fonts.body_family,
            force_bold: false,
            force_italic: false,
        }
    }

    fn entry_title(t: &Template, colors: &DocxColors) -> Self {
        let link = theme::link_style(t.id);
        Self {
            size: pt_to_dxa(t.body_pt),
            color: colors.body.clone(),
            link_color: Self::link_color(colors, link),
            underline: link.underline,
            family: t.fonts.body_family,
            force_bold: true,
            force_italic: false,
        }
    }

    fn subtitle(t: &Template, colors: &DocxColors) -> Self {
        let link = theme::link_style(t.id);
        Self {
            size: pt_to_dxa(t.body_pt - 0.5),
            color: colors.date.clone(),
            link_color: Self::link_color(colors, link),
            underline: link.underline,
            family: t.fonts.body_family,
            force_bold: false,
            force_italic: t.job_title_italic,
        }
    }
}

fn add_rich(mut para: Paragraph, rt: &RichText, opts: &RunOpts) -> Paragraph {
    for run in rt {
        let mut r = Run::new()
            .add_text(&run.text)
            .size(opts.size)
            .fonts(docx_run_fonts(opts.family));
        if run.bold || opts.force_bold {
            r = r.bold();
        }
        if run.italic || opts.force_italic {
            r = r.italic();
        }

        match &run.link {
            Some(url) => {
                r = r.color(opts.link_color.as_str());
                if opts.underline {
                    r = r.underline("single");
                }
                para = para
                    .add_hyperlink(Hyperlink::new(url.clone(), HyperlinkType::External).add_run(r));
            }
            None => {
                para = para.add_run(r.color(opts.color.as_str()));
            }
        }
    }
    para
}

#[cfg(test)]
mod test;
