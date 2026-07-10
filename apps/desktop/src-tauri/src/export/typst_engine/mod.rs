//! Typst-based PDF rendering engine — Cutover-1: live for all eight templates.
//!
//! This module is named `typst_engine` (not `typst`) to avoid shadowing the
//! extern `typst` crate.
//!
//! Public surface:
//! - [`render_pdf`] — compile a `DocumentModel` + `TypstTemplate` + `RenderOpts`
//!   + optional `&Template` styling → raw PDF bytes.
//! - [`render_pdf_with_photo`] — photo-aware variant of [`render_pdf`].
//! - [`render_letter_pdf`] — compile a finished cover-letter text → PDF bytes.
//! - [`render_resume_svg_pages`] / [`render_resume_svg_pages_with_photo`] /
//!   [`render_letter_svg_pages`] — live-preview siblings of the PDF render fns,
//!   emitting one SVG string per page (same model + same world; only the emit
//!   differs).
//! - [`render_pdf_from_source`] — compile a raw Typst source string (smoke test /
//!   debugging only).
//! - [`RenderOpts`] — page geometry, accent colour, language, ATS flag.
//! - [`TypstTemplate`] — which Typst template to use.

mod engine;
mod letter;
mod photo;
mod render;
mod world;

#[cfg(test)]
mod test;

pub use engine::{
    render_letter_pdf, render_letter_svg_pages, render_pdf, render_pdf_with_photo,
    render_resume_svg_pages, render_resume_svg_pages_with_photo, TypstTemplate,
};
// `render_pdf_from_source` is only used in tests (smoke tests and debugging).
#[cfg(test)]
pub use engine::render_pdf_from_source;
pub use photo::resolve_photo;
pub use render::RenderOpts;
// Single source of truth for document-accent hex validation, reused by the
// DOCX / cover-letter accent-override path in `export::templates`.
pub(crate) use render::normalise_accent;
