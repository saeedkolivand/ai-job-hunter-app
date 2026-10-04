//! Section / block → paragraphs: the walk from a [`Section`]'s headings,
//! paragraphs, bullets and entries to the `docx-rs` `Paragraph`s that make up
//! a flow (full page width, or one table cell).

use docx_rs::*;

use crate::export::docx_renderer::{docx_run_fonts, inch_to_dxa, pt_to_dxa, pt_to_half_points};
use crate::model::document::{Block, EntryBlock, Section};
use crate::model::rich::RichText;

use super::rich::{add_rich, RunOpts};
use super::Ctx;

pub(super) fn section_paragraphs(section: &Section, ctx: &Ctx) -> Vec<Paragraph> {
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
                    .size(pt_to_half_points(pt))
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
        // Italic (not bold) — matches the PDF path (`single_column.typ`'s
        // date-str run): the date/duration reads as a distinct, fast-to-scan
        // element next to the bold entry title, without a heavier weight that
        // would compete with it. Font-variant only, ATS-safe.
        if ctx.right_align_date {
            title = title
                .add_run(Run::new().add_tab())
                .add_run(
                    Run::new()
                        .add_text(date)
                        .size(pt_to_half_points(9.5))
                        .color(ctx.colors.date.as_str())
                        .fonts(docx_run_fonts(t.fonts.body_family))
                        .italic(),
                )
                .add_tab(Tab::new().val(TabValueType::Right).pos(ctx.width_dxa));
        } else {
            title = title.add_run(
                Run::new()
                    .add_text(format!("  ·  {date}"))
                    .size(pt_to_half_points(9.5))
                    .color(ctx.colors.date.as_str())
                    .fonts(docx_run_fonts(t.fonts.body_family))
                    .italic(),
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
        // `before` matches `_scale.typ`'s `sp-subtitle-gap` (3pt, #28) so the
        // subtitle doesn't sit crammed against the title the way the PDF used
        // to before that fix — DOCX had no "before" spacing here at all.
        .line_spacing(LineSpacing::new().before(pt_to_dxa(3.0) as u32).after(60))
        .keep_next(true);
        out.push(para);
    }

    for bullet in &e.bullets {
        out.push(bullet_paragraph(bullet, ctx));
    }

    out
}
