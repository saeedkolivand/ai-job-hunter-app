//! PDF backend for the canonical layout engine.
//!
//! Strangler-fig step: this renders a resume by building the [`DocumentModel`],
//! laying it out with [`crate::layout::layout_document`] into a backend-agnostic
//! [`LaidOutDoc`], and translating that display list to `printpdf` ops. It does
//! **no** layout itself — geometry (wrapping, centering, columns, pagination,
//! link rects) all comes from the engine.
//!
//! It is wired into [`super::pdf::generate_pdf`] behind the `layout_pdf` Cargo
//! feature, which is **on by default** now that gap-by-gap snapshot parity with
//! the legacy renderer is locked (see the parity gate in [`crate::layout`]'s
//! tests). `--no-default-features` falls back to the legacy renderer, which stays
//! as the parity reference. The two remain side-by-side so neither path rots.

use anyhow::Result;
use printpdf::*;

use crate::export::pdf_renderer::{
    build_line, load_all_fonts, resolve_fonts, rgb_to_color, LoadedFontSet,
};
use crate::export::templates::Template;
use crate::export::types::GenerationMeta;
use crate::layout::{layout_document, FillRect, LaidOutDoc, LinkRect, PlacedText, RuleLine};
use crate::locale::PageSize;
use crate::measure::FontMetrics;
use crate::model::adapter::model_from_resume_text;
use crate::model::transform;

const PT_PER_MM: f32 = 2.834_645_7;

/// Render a resume to PDF bytes via the canonical layout engine.
pub(crate) fn generate_resume_pdf(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    ats_mode: bool,
) -> Result<Vec<u8>> {
    let mut model = model_from_resume_text(text);

    // Header name override from generation metadata (mirrors the legacy renderer).
    if let Some(name) = meta.and_then(|m| m.candidate_name.as_deref()) {
        if !name.is_empty() {
            model.header.name = name.to_string();
        }
    }

    // ATS mode: collapse to a single column and put sections in ATS reading order.
    let effective_template = if ats_mode && template.two_column.is_some() {
        let mut t = template.clone();
        t.two_column = None;
        t.margin_in = 1.0;
        t
    } else {
        template.clone()
    };
    if ats_mode {
        transform::linearize(&mut model);
    }

    // Page geometry: A4, matching the legacy renderer (locale-driven in a later phase).
    let geom = PageSize::A4.geometry();
    let laid = layout_document(&model, &effective_template, geom, &FontMetrics);
    emit_pdf(&laid)
}

/// Translate a [`LaidOutDoc`] (mm from top-left) into a `printpdf` document
/// (mm from bottom-left), drawing fills, then rules, then text, then links.
fn emit_pdf(laid: &LaidOutDoc) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::new("Resume");
    let fonts = load_all_fonts(&mut doc)?;
    let page_h = laid.page_height_mm;

    let pages: Vec<PdfPage> = laid
        .pages
        .iter()
        .map(|page| {
            let mut ops: Vec<Op> = Vec::new();
            for fill in &page.fills {
                ops.extend(fill_ops(fill, page_h));
            }
            for rule in &page.rules {
                ops.extend(rule_ops(rule, page_h));
            }
            for text in &page.texts {
                ops.extend(text_ops(text, page_h, &fonts));
            }
            for link in &page.links {
                ops.push(link_op(link, page_h));
            }
            PdfPage::new(Mm(laid.page_width_mm), Mm(page_h), ops)
        })
        .collect();

    let mut warnings = Vec::new();
    Ok(doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings))
}

/// Filled rectangle. The engine gives a top-left origin + height; printpdf wants
/// bottom-up coordinates.
fn fill_ops(f: &FillRect, page_h: f32) -> Vec<Op> {
    if f.width_mm <= 0.0 || f.height_mm <= 0.0 {
        return Vec::new();
    }
    let x = f.x_mm;
    let w = f.width_mm;
    let h = f.height_mm;
    let y_bottom = page_h - (f.y_top_mm + h);
    let polygon = Polygon {
        rings: vec![PolygonRing {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(x), Mm(y_bottom)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x + w), Mm(y_bottom)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x + w), Mm(y_bottom + h)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x), Mm(y_bottom + h)),
                    bezier: false,
                },
            ],
        }],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::EvenOdd,
    };
    vec![
        Op::SetFillColor {
            col: rgb_to_color(f.color),
        },
        Op::DrawPolygon { polygon },
    ]
}

fn rule_ops(r: &RuleLine, page_h: f32) -> Vec<Op> {
    build_line(
        r.x1_mm,
        page_h - r.y_mm,
        r.x2_mm,
        rgb_to_color(r.color),
        r.thickness_pt,
    )
}

fn text_ops(t: &PlacedText, page_h: f32, fonts: &LoadedFontSet) -> Vec<Op> {
    let (reg, bold, italic) = resolve_fonts(fonts, t.family);
    let font = if t.italic {
        italic.unwrap_or(if t.bold { bold } else { reg })
    } else if t.bold {
        bold
    } else {
        reg
    };
    let pos = Point {
        x: Mm(t.x_mm).into(),
        y: Mm(page_h - t.baseline_y_mm).into(),
    };
    vec![
        Op::StartTextSection,
        Op::SetFillColor {
            col: rgb_to_color(t.color),
        },
        Op::SetTextCursor { pos },
        Op::SetFont {
            font: PdfFontHandle::External(font.clone()),
            size: Pt(t.size_pt),
        },
        Op::ShowText {
            items: vec![TextItem::Text(t.text.clone())],
        },
        Op::EndTextSection,
    ]
}

fn link_op(l: &LinkRect, page_h: f32) -> Op {
    let y_bottom_pt = (page_h - (l.y_top_mm + l.height_mm)) * PT_PER_MM;
    let rect = Rect {
        x: Pt(l.x_mm * PT_PER_MM),
        y: Pt(y_bottom_pt),
        width: Pt(l.width_mm * PT_PER_MM),
        height: Pt(l.height_mm * PT_PER_MM),
        mode: None,
        winding_order: None,
    };
    Op::LinkAnnotation {
        link: LinkAnnotation::new(
            rect,
            Actions::Uri(l.url.clone()),
            Some(BorderArray::Solid([0.0, 0.0, 0.0])),
            Some(ColorArray::Transparent),
            None,
        ),
    }
}

#[cfg(test)]
mod test;
