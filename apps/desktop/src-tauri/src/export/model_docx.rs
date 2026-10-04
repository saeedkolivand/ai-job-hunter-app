//! DOCX backend for the canonical document model (Phase 5).
//!
//! Builds a `DocumentModel` from resume text and translates it to a `docx-rs`
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
//!
//! Split (issue #1280 batch 5b) into: [`header`] (name/title/contact + the
//! decorative header band), [`two_column`] (the sidebar/main table body),
//! [`blocks`] (section → paragraph walk) and [`rich`] (rich-text → runs /
//! hyperlinks + per-context [`rich::RunOpts`]).

mod blocks;
mod header;
mod rich;
mod two_column;

use crate::error::AppResult;
use docx_rs::*;

use crate::export::docx_renderer::{inch_to_dxa, mm_to_dxa, setup_colors, DocxColors};
use crate::export::templates::Template;
use crate::export::types::GenerationMeta;
use crate::locale::PageGeometry;
use crate::model::adapter::model_from_resume_text;
use crate::model::transform;
use crate::theme::{self, LinkStyle};

use blocks::section_paragraphs;
use header::add_header;
use two_column::add_two_column_body;

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
) -> AppResult<Docx> {
    generate_resume_docx_in(
        text,
        meta,
        template,
        ats_mode,
        crate::locale::LocaleProfile::default().page_geometry(),
        None,
        "en",
        "en",
    )
}

/// Render a resume to a `Docx` on a specific page geometry (locale-driven).
///
/// `contact` (when present) is applied via `ContactProfile::apply_to_header` —
/// a FALLBACK only (H: the editor is the source of truth), filling the header
/// name/contact line ONLY when the text-derived header carries none. Shared
/// with the PDF backend (same `apply_to_header` call, same localization by
/// `lang`) so both documents' headers stay in lockstep whichever side (text
/// or profile) actually supplied them.
///
/// `market` (the request's `locale`, e.g. `"de"`) is distinct from `lang` (the
/// document's WRITTEN language, `target_lang()`) — a candidate can write an
/// English résumé for a German posting. Only `market` drives ATS section order.
pub(crate) fn generate_resume_docx_in(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    ats_mode: bool,
    geom: PageGeometry,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
    market: &str,
) -> AppResult<Docx> {
    let mut model = model_from_resume_text(text);

    // Fallback only: the text-derived header name wins whenever the document
    // already has one; metadata fills a header that has none.
    if model.header.name.trim().is_empty() {
        if let Some(name) = meta.and_then(|m| m.candidate_name.as_deref()) {
            let name = name.trim();
            if !name.is_empty() {
                model.header.name = name.to_string();
            }
        }
    }

    if let Some(profile) = contact {
        profile.apply_to_header(&mut model.header, lang);
    }

    // ATS mode collapses to a single column and reorders sections for reading.
    // theme::is_two_column is the single source of truth for two-column gating.
    // Belt-and-suspenders: also require two_column config to be present so a
    // future template classified as two-column but missing the config falls back
    // to single-column rather than reaching the expect() in add_two_column_body.
    let two_column =
        crate::theme::is_two_column(template.id) && template.two_column.is_some() && !ats_mode;
    if ats_mode {
        // Canonicalise a region-tagged locale (`de-DE`, `de_AT`) to the market
        // id `section_order_for`'s alias arm expects — see the matching
        // comment in `export/pdf/mod.rs`.
        let market = crate::locale::LocaleProfile::get(market).id;
        transform::linearize(&mut model, market);
    }

    let colors = setup_colors(template);
    let (abstract_num, num) = crate::export::docx_renderer::create_bullet_numbering();
    let mut docx = Docx::new()
        .add_abstract_numbering(abstract_num)
        .add_numbering(num);

    // Header spans the full width, above any columns. `ats_mode` reaches it
    // because the Awesome band is decorative colour the ATS toggle drops —
    // exactly as `awesome.typ` drops it behind `is-ats`.
    docx = add_header(docx, &model.header, template, &colors, ats_mode);

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

#[cfg(test)]
mod tests;
