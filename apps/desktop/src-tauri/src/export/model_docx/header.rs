//! Résumé header rendering (name / title / contact line) for the model-based
//! DOCX backend, including the Awesome-style decorative header band.

use docx_rs::*;

use crate::export::docx::band_tint_hex;
use crate::export::docx_renderer::{docx_run_fonts, pt_to_dxa, pt_to_half_points, DocxColors};
use crate::export::templates::Template;
use crate::model::document::HeaderBlock;
use crate::theme;

use super::rich::{add_rich, RunOpts};

pub(super) fn add_header(
    mut docx: Docx,
    header: &HeaderBlock,
    t: &Template,
    colors: &DocxColors,
    ats_mode: bool,
) -> Docx {
    if !header.name.is_empty() {
        let name_run = Run::new()
            .add_text(&header.name)
            .size(pt_to_half_points(t.name_pt))
            .bold()
            .color(colors.name.as_str())
            .fonts(docx_run_fonts(t.fonts.name_family));

        // 9pt (#28), matching `_scale.typ`'s `sp-name-below` — previously no
        // explicit spacing at all, leaving the name→contact gap to whatever
        // Word's own default paragraph spacing happens to be.
        let mut p = Paragraph::new()
            .add_run(name_run)
            .line_spacing(LineSpacing::new().after(pt_to_dxa(9.0) as u32));

        // A banded template's PDF (`awesome.typ`) draws a full-width
        // accent-tinted band behind the name. `docx-rs` has no page-background
        // primitive, so approximate it exactly the way the Banded cover-letter
        // layout does (`docx::mod`'s `header_band` branch): PARAGRAPH-level
        // shading, filled with the accent lightened toward white by
        // `docx::band_tint_hex`, keeping the normal dark ink. Run-level `w:shd`
        // would only tint the glyph boxes, and white ink on it disappears
        // entirely in any reader that ignores run shading — the invisible-name
        // hazard `awesome_matches_spec` guards the registry against.
        //
        // WHICH templates are banded is `theme::has_header_band`'s call (the
        // same owner as `is_two_column`/`placement_for`), so PDF and DOCX can't
        // disagree on the roster. WHETHER to draw it stays here: ATS mode drops
        // the band, matching `awesome.typ`'s `is-ats` branch (which renders a
        // plain black-on-white header).
        if theme::has_header_band(t.id) && !ats_mode {
            p.property = p.property.shading(
                Shading::new()
                    .shd_type(ShdType::Clear)
                    .color("auto")
                    .fill(band_tint_hex(t.accent_color)),
            );
        }
        if t.name_centered {
            p = p.align(AlignmentType::Center);
        }
        docx = docx.add_paragraph(p);
    }

    if let Some(title) = &header.title {
        let mut p = Paragraph::new().add_run(
            Run::new()
                .add_text(title)
                .size(pt_to_half_points(t.body_pt))
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
