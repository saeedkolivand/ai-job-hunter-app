//! Typst compilation and PDF / SVG rendering.
//!
//! **Isolation boundary**: this is the ONLY file (alongside `render.rs`) that
//! imports the `typst`, `typst_pdf`, and `typst_svg` crates directly. No `typst`,
//! `typst_pdf`, or `typst_svg` types may appear in any function signature that is
//! `pub` outside this module — callers only see `AppResult<Vec<u8>>` (PDF) or
//! `AppResult<Vec<String>>` (one SVG string per page).
//!
//! Public API (Phase 1a / Phase 2):
//! - [`render_pdf`] — compile a [`DocumentModel`] through a named template →
//!   raw PDF bytes.  This is the real entry point; the model-based path.
//! - [`render_resume_svg_pages`] — the live-preview sibling of [`render_pdf`]:
//!   the SAME model + SAME world, emitting one SVG string per page instead of
//!   a single PDF blob. Fidelity is identical to export; only the final emit
//!   differs.
//! - [`render_letter_svg_pages`] — the live-preview sibling of
//!   [`render_letter_pdf`].
//! - [`render_pdf_from_source`] — compile a raw Typst source string (retained
//!   for the smoke test and low-level debugging; not wired into the live export
//!   flow).

use typst::layout::PagedDocument;
use typst_pdf::{pdf, PdfOptions};

use crate::contact_profile::ContactProfile;
use crate::error::{AppError, AppResult};
use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::model::document::DocumentModel;

use super::letter::{parse_cover_letter, style_from_template as letter_style_from_template};
use super::render::{prepare, prepare_with_photo, PreparedRender, RenderOpts};
use super::world::ResumeWorld;

// ── Shared spacing scale (embedded at compile time) ───────────────────────────

/// Centralized house spacing scale — prepended to EVERY template source.
///
/// This is the ONLY place the scale constants are declared. Individual `.typ`
/// templates must NOT redeclare them. By prepending this string, `engine.rs`
/// acts as the single owner of the scale lock.
const SCALE_TYP: &str = include_str!("templates/_scale.typ");

// ── Template sources (embedded at compile time) ───────────────────────────────

/// Classic template source embedded so no disk access is needed at runtime.
const CLASSIC_TYP: &str = include_str!("templates/classic.typ");

/// Atelier two-column premium template source embedded at compile time.
const ATELIER_TYP: &str = include_str!("templates/atelier.typ");

/// Parametric single-column template driven by `data.style`.
/// Serves Modern, SwissMinimal, and Academic (and Classic after migration).
const SINGLE_COLUMN_TYP: &str = include_str!("templates/single_column.typ");

/// Meridian — header-band premium single-column template (Phase 3a).
const MERIDIAN_TYP: &str = include_str!("templates/meridian.typ");

/// Throughline — timeline spine premium single-column template (Phase 3a).
const THROUGHLINE_TYP: &str = include_str!("templates/throughline.typ");

/// Portrait — circular photo top-left, name/title right, accent keyline, two-column.
const PORTRAIT_TYP: &str = include_str!("templates/portrait.typ");

/// Lebenslauf — DACH DIN-style tabular CV with photo top-right.
const LEBENSLAUF_TYP: &str = include_str!("templates/lebenslauf.typ");

/// Parametric cover-letter template driven by `data.opts` + `data.style`.
/// A4 or Letter; themed from the chosen resume template's accent + fonts.
const LETTER_TYP: &str = include_str!("templates/letter.typ");

/// Test-only accessor for the embedded `(SCALE_TYP, LETTER_TYP)` sources so the
/// offline `generate_cover_template_previews` test can build the exact same
/// cover-letter Typst world as [`render_letter_pdf`] without duplicating the
/// `include_str!` paths (the two consts stay private to production code).
#[cfg(test)]
pub(super) const fn letter_template_sources() -> (&'static str, &'static str) {
    (SCALE_TYP, LETTER_TYP)
}

// ── Template enum (Typst-side) ────────────────────────────────────────────────

/// Which Typst template to use for rendering.
///
/// Phase 1a ships `Classic`; Phase 1b adds `Atelier` (two-column premium).
/// Phase 2 adds `SingleColumn` — a parametric renderer driven by `data.style`.
/// Phase 3a adds `Meridian`, `Throughline` — original premium single-column
/// templates, each with its own `.typ` file.
/// Phase 3b-i adds `Portrait` and `Lebenslauf` — two photo templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypstTemplate {
    Classic,
    /// Phase 1b — two-column premium sidebar template.
    Atelier,
    /// Phase 2 — parametric single-column driven by `data.style`.
    /// Used for Modern, SwissMinimal, Academic (and Classic after migration).
    SingleColumn,
    /// Phase 3a — header-band premium single-column template.
    Meridian,
    /// Phase 3a — timeline-spine premium single-column template.
    Throughline,
    /// Phase 3b-i — circular photo top-left, two-column, accent keyline.
    Portrait,
    /// Phase 3b-i — DACH DIN-style tabular CV, photo top-right.
    Lebenslauf,
}

impl TypstTemplate {
    /// Return the Typst source for this template (WITHOUT the scale preamble).
    /// The preamble is prepended in [`TypstTemplate::source_with_scale`].
    fn raw_source(self) -> &'static str {
        match self {
            TypstTemplate::Classic => CLASSIC_TYP,
            TypstTemplate::Atelier => ATELIER_TYP,
            TypstTemplate::SingleColumn => SINGLE_COLUMN_TYP,
            TypstTemplate::Meridian => MERIDIAN_TYP,
            TypstTemplate::Throughline => THROUGHLINE_TYP,
            TypstTemplate::Portrait => PORTRAIT_TYP,
            TypstTemplate::Lebenslauf => LEBENSLAUF_TYP,
        }
    }

    /// Return the full Typst source with the shared scale preamble prepended.
    ///
    /// This is what gets passed to the World as `/main.typ`.  The scale
    /// constants declared in `_scale.typ` are therefore always in scope.
    ///
    /// `pub(super)` so the sibling `test` module can call it for the showcase
    /// banner generator without exposing the raw source outside this module.
    pub(super) fn source_with_scale(self) -> String {
        format!("{}\n{}", SCALE_TYP, self.raw_source())
    }

    /// Derive the Typst template from an existing [`Template`] configuration.
    /// All nine live template IDs are handled exhaustively; no fallback needed.
    pub fn from_template(t: &Template) -> Self {
        match t.id {
            TemplateId::Classic => TypstTemplate::Classic,
            TemplateId::Atelier => TypstTemplate::Atelier,
            // Modern, SwissMinimal, Academic → parametric SingleColumn renderer.
            TemplateId::Modern | TemplateId::SwissMinimal | TemplateId::Academic => {
                TypstTemplate::SingleColumn
            }
            // Phase 3a: two premium single-column templates.
            TemplateId::Meridian => TypstTemplate::Meridian,
            TemplateId::Throughline => TypstTemplate::Throughline,
            // Phase 3b-i: two photo templates.
            TemplateId::Portrait => TypstTemplate::Portrait,
            TemplateId::Lebenslauf => TypstTemplate::Lebenslauf,
        }
    }
}

// ── Internal compile helpers ──────────────────────────────────────────────────

/// Compile a [`ResumeWorld`] to a paged document, surfacing warnings as debug
/// logs and mapping compile diagnostics to [`AppError::Parse`].
///
/// Shared by the PDF emit ([`compile_and_export`]) and the SVG emit
/// ([`compile_and_svg`]) so both paths compile the EXACT same world — preview
/// fidelity is guaranteed identical to export because only the final emit step
/// differs.
fn compile_world(world: &ResumeWorld) -> AppResult<PagedDocument> {
    let warned = typst::compile::<PagedDocument>(world);

    // Surface any warnings as a debug log (non-fatal).
    for w in &warned.warnings {
        log::debug!("typst warning: {}", w.message);
    }

    warned.output.map_err(|diags| {
        let msg = diags
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        AppError::Parse(format!("Typst compile error: {msg}"))
    })
}

fn compile_and_export(world: &ResumeWorld) -> AppResult<Vec<u8>> {
    let document = compile_world(world)?;

    let options = PdfOptions::default();
    let pdf_bytes = pdf(&document, &options).map_err(|diags| {
        let msg = diags
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        AppError::Parse(format!("Typst PDF export error: {msg}"))
    })?;

    Ok(pdf_bytes)
}

/// Compile a [`ResumeWorld`] and emit one SVG string per page (vector — no
/// rasteriser, no `image` crate). Mirrors [`compile_and_export`] but for the
/// live-preview path. Guards against a zero-page document so callers always get
/// at least one renderable page.
fn compile_and_svg(world: &ResumeWorld) -> AppResult<Vec<String>> {
    let document = compile_world(world)?;

    if document.pages.is_empty() {
        return Err(AppError::Parse(
            "Typst SVG export error: document produced zero pages".to_string(),
        ));
    }

    let pages: Vec<String> = document.pages.iter().map(typst_svg::svg).collect();
    Ok(pages)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Compile a [`DocumentModel`] through a Typst template and render to PDF bytes.
///
/// When `template` is [`TypstTemplate::SingleColumn`] the caller must supply
/// the matching [`Template`] registry entry so that `data.style` is populated
/// with colors/fonts/section-style. For Classic and Atelier the template manages
/// its own styling internally (for Classic, back-compat; for Atelier, complex
/// two-column logic). Passing `Some(t)` for Classic/Atelier is harmless — the
/// style object is serialized into JSON but the template source ignores it.
///
/// Data is injected as an in-memory `data.json` (no string interpolation of
/// user content into markup). The offline hard-wall in [`ResumeWorld`] ensures
/// only `/main.typ` and `/data.json` are ever served — no disk or network access.
///
/// This IS the live résumé render path as of Cutover-1 — called by
/// `generate_pdf` in `export/pdf/mod.rs` for all non-photo templates.
pub fn render_pdf(
    model: &DocumentModel,
    template: TypstTemplate,
    opts: &RenderOpts,
    style: Option<&Template>,
) -> AppResult<Vec<u8>> {
    let world = build_resume_world(model, template, opts, style)?;
    compile_and_export(&world)
}

/// Build the [`ResumeWorld`] for a résumé render.
///
/// Single source of truth for résumé world construction — shared by the PDF
/// emit ([`render_pdf`]) and the SVG live-preview emit
/// ([`render_resume_svg_pages`]) so both compile byte-identical inputs. Behaviour
/// is preserved exactly from the prior inline body of [`render_pdf`].
fn build_resume_world(
    model: &DocumentModel,
    template: TypstTemplate,
    opts: &RenderOpts,
    style: Option<&Template>,
) -> AppResult<ResumeWorld> {
    let source = template.source_with_scale();
    let PreparedRender { source, data_json } = prepare(model, &source, opts, style)?;
    Ok(ResumeWorld::with_data(&source, Some(data_json)))
}

/// Live-preview sibling of [`render_pdf`]: compile the SAME [`DocumentModel`]
/// through the SAME Typst world, emitting one SVG string per page instead of a
/// single PDF blob.
///
/// Fidelity is identical to the PDF export — same model, same template, same
/// world — because only the final emit step differs (`typst_svg::svg` per page
/// vs `typst_pdf::pdf`). Returns at least one page; an empty document is an
/// error (guarded in [`compile_and_svg`]).
///
/// No `typst` / `typst_svg` types appear in the public signature (offline
/// hard-wall preserved); callers only see `AppResult<Vec<String>>`.
pub fn render_resume_svg_pages(
    model: &DocumentModel,
    template: TypstTemplate,
    opts: &RenderOpts,
    style: Option<&Template>,
) -> AppResult<Vec<String>> {
    let world = build_resume_world(model, template, opts, style)?;
    compile_and_svg(&world)
}

/// Compile a [`DocumentModel`] through a photo-capable Typst template.
///
/// This is the photo-aware variant of [`render_pdf`].  `photo_png` must be the
/// clean, sanitised PNG bytes returned by [`super::photo::resolve_photo`] — or
/// `None` when no photo is available (the template falls back to a text-only
/// header).  The bytes are served as the virtual file `/photo.png` inside
/// [`ResumeWorld`]; the `data.opts.has_photo` flag tells the template whether
/// the file is present.
///
/// All other behaviour matches [`render_pdf`].
pub fn render_pdf_with_photo(
    model: &DocumentModel,
    template: TypstTemplate,
    opts: &RenderOpts,
    style: Option<&Template>,
    photo_png: Option<Vec<u8>>,
) -> AppResult<Vec<u8>> {
    let world = build_resume_world_with_photo(model, template, opts, style, photo_png)?;
    compile_and_export(&world)
}

/// Build the photo-aware [`ResumeWorld`] for a résumé render.
///
/// Single source of truth shared by [`render_pdf_with_photo`] (PDF) and
/// [`render_resume_svg_pages_with_photo`] (SVG live-preview). Behaviour is
/// preserved exactly from the prior inline body of [`render_pdf_with_photo`].
fn build_resume_world_with_photo(
    model: &DocumentModel,
    template: TypstTemplate,
    opts: &RenderOpts,
    style: Option<&Template>,
    photo_png: Option<Vec<u8>>,
) -> AppResult<ResumeWorld> {
    let has_photo = photo_png.is_some();
    let source = template.source_with_scale();
    let PreparedRender { source, data_json } =
        prepare_with_photo(model, &source, opts, style, has_photo)?;
    Ok(ResumeWorld::with_data_and_photo(
        &source,
        Some(data_json),
        photo_png,
    ))
}

/// Photo-aware live-preview sibling of [`render_resume_svg_pages`].
///
/// Mirrors [`render_pdf_with_photo`] but emits one SVG string per page. Same
/// model, same photo bytes (`/photo.png` virtual file), same world — only the
/// final emit differs. Returns at least one page.
pub fn render_resume_svg_pages_with_photo(
    model: &DocumentModel,
    template: TypstTemplate,
    opts: &RenderOpts,
    style: Option<&Template>,
    photo_png: Option<Vec<u8>>,
) -> AppResult<Vec<String>> {
    let world = build_resume_world_with_photo(model, template, opts, style, photo_png)?;
    compile_and_svg(&world)
}

/// Compile a raw Typst source string to PDF bytes (no data injection).
///
/// Retained for the smoke test and low-level debugging. Not called from the
/// live export flow; the model-based [`render_pdf`] is the real entry point.
#[cfg_attr(not(test), allow(dead_code))]
pub fn render_pdf_from_source(source: &str) -> AppResult<Vec<u8>> {
    let world = ResumeWorld::new(source);
    compile_and_export(&world)
}

/// Render a finished cover-letter text to PDF bytes via the Typst letter template.
///
/// This IS the live cover-letter render path as of Cutover-1 — called by
/// `generate_pdf` in `export/pdf/mod.rs` for `DocumentType::CoverLetter`.
/// This function is the parallel entry point to [`render_pdf`] for résumés.
///
/// Parameters:
///
/// - `text` — the full finished letter text (as produced by the AI generator).
/// - `template` — the resume [`Template`] whose accent + fonts theme the letter.
/// - `contact` — optional [`ContactProfile`] (preferred header source; falls back
///   to scraping the text when absent or effectively empty).
/// - `meta_name` — optional candidate name from generation metadata.
/// - `market` — resolved job-market id (`"us"`, `"de"`, …); drives date position,
///   subject-line, and page size per locale conventions.
/// - `lang` — BCP-47 language tag for font stack selection.
///
/// All `typst`/`typst_pdf` types remain inside this file and `render.rs`/`world.rs`.
/// No typst types appear in the public signature.
pub fn render_letter_pdf(
    text: &str,
    template: &Template,
    contact: Option<&ContactProfile>,
    meta_name: Option<&str>,
    market: &str,
    lang: &str,
) -> AppResult<Vec<u8>> {
    let world = build_letter_world(text, template, contact, meta_name, market, lang)?;
    compile_and_export(&world)
}

/// Build the [`ResumeWorld`] for a cover-letter render.
///
/// Single source of truth for cover-letter world construction — shared by the
/// PDF emit ([`render_letter_pdf`]) and the SVG live-preview emit
/// ([`render_letter_svg_pages`]). Behaviour is preserved exactly from the prior
/// inline body of [`render_letter_pdf`] (same `parse_cover_letter`, same source
/// preamble, same `data.json` serialisation).
fn build_letter_world(
    text: &str,
    template: &Template,
    contact: Option<&ContactProfile>,
    meta_name: Option<&str>,
    market: &str,
    lang: &str,
) -> AppResult<ResumeWorld> {
    let style = letter_style_from_template(template);
    let model = parse_cover_letter(text, contact, meta_name, market, lang, style);

    let data_json = serde_json::to_vec(&model).map_err(|e| {
        AppError::Parse(format!(
            "typst_engine letter: JSON serialisation failed: {e}"
        ))
    })?;

    // Prepend the shared spacing scale then the letter template source.
    let source = format!(
        "// Auto-generated cover-letter entry — do not edit.\n\
         #let data = json(\"data.json\")\n\
         {SCALE_TYP}\n\
         {LETTER_TYP}"
    );

    Ok(ResumeWorld::with_data(&source, Some(data_json)))
}

/// Live-preview sibling of [`render_letter_pdf`]: compile the SAME finished
/// cover-letter text through the SAME Typst world, emitting one SVG string per
/// page instead of a single PDF blob.
///
/// Fidelity is identical to the PDF export — same `LetterModel`, same template
/// style, same world — because only the final emit step differs. Returns at
/// least one page; an empty document is an error (guarded in
/// [`compile_and_svg`]). No `typst` / `typst_svg` types appear in the public
/// signature.
pub fn render_letter_svg_pages(
    text: &str,
    template: &Template,
    contact: Option<&ContactProfile>,
    meta_name: Option<&str>,
    market: &str,
    lang: &str,
) -> AppResult<Vec<String>> {
    let world = build_letter_world(text, template, contact, meta_name, market, lang)?;
    compile_and_svg(&world)
}
