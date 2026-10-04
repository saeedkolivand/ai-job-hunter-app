//! Two-column body: a borderless, single-row two-cell table (shaded sidebar
//! cell + main cell), section placement decided by `theme::placement_for`.

use docx_rs::*;

use crate::export::docx_renderer::{rgb_to_hex, DocxColors};
use crate::export::templates::Template;
use crate::locale::PageGeometry;
use crate::model::document::{DocumentModel, Placement};
use crate::theme;

use super::blocks::section_paragraphs;
use super::{content_width_dxa, Ctx};

pub(super) fn add_two_column_body(
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
        match theme::placement_for(template.id, &section.id) {
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
