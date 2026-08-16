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
// Letterhead name-guard family (`is_letterhead_name` / `letterhead_initials` /
// their private helpers). Split out of `letter.rs` — sibling module, not a
// public-surface change — to stay under the R8 module-size cap; see its
// module doc comment.
mod letterhead;
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
// Single source of truth for the letterhead monogram initials, shared by the
// Typst layout (via `LetterHead.initials`) and the DOCX approximation. Same
// posture as `normalise_accent` below: one derivation, so PDF and DOCX can
// never disagree about what the device says — or about which openings are not
// names at all (salutation / sign-off / subject / date). The unguarded
// `monogram_initials` is deliberately NOT exported: DOCX calling it directly
// is how the date hole survived in one format after being closed in the other.
//
// Lives in the sibling `letterhead` module (split out of `letter.rs` for the
// R8 module-size cap); the re-export path here is unchanged, so no consumer
// (`docx::mod`, `letter::parse_cover_letter`) had to change its call site.
pub(crate) use letterhead::letterhead_initials;
// The predicate behind `letterhead_initials` above, exported separately so
// callers that need "is this a name" (not "give me the initials") don't have
// to check `!letterhead_initials(s).is_empty()` — a mononym's initials are one
// character, not empty, so that would have been the wrong test. Used by both
// `parse_cover_letter` (letterhead NAME text) and `export/docx/mod.rs`'s two
// line-scanners (DOCX has no shared `LetterModel` to funnel through), so every
// format agrees on which openings are not names.
pub(crate) use letterhead::is_letterhead_name;
// The shared "prefer meta_name unless it's blank" resolution used by every
// letterhead-name call site — the PDF parser and both DOCX line-scanners —
// so an empty-string `Some("")` candidate name (the shape three renderer
// call sites actually send) can never fall through to a REAL name on the
// letter's first line in one format while the other renders it.
pub(crate) use letterhead::resolve_letterhead_candidate;
// The same date heuristic `parse_cover_letter` and `is_letterhead_name` use to
// classify a pre-salutation line, now also needed by
// `letter_shape::complete_letter_text` (sibling `export` module) so the
// completion step and the parser can never disagree about what counts as a
// date line — the two already share `is_subject_line` the same way.
pub(crate) use letterhead::looks_like_date;
pub use photo::resolve_photo;
pub use render::RenderOpts;
// Single source of truth for document-accent hex validation, reused by the
// DOCX / cover-letter accent-override path in `export::templates`.
pub(crate) use render::normalise_accent;
