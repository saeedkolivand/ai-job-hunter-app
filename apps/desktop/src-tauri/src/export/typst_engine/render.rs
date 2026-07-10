//! Serialize a [`DocumentModel`] + [`RenderOpts`] into the JSON payload
//! and Typst entry source for template rendering.
//!
//! **Injection safety**: model content is serialized to JSON and injected via
//! a virtual `data.json` file (`#let data = json("data.json")`). Template
//! markup is never built by string-concatenating model text, eliminating
//! Typst-markup injection hazards.
//!
//! This is the ONLY file (alongside `engine.rs`) allowed to construct the
//! data payload; callers interact through [`RenderOpts`] and the opaque
//! [`PreparedRender`] value type.

use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

use crate::export::templates::{SectionStyle, Template};
use crate::export::types::{FontFamily, TemplateId};
use crate::locale::PageGeometry;
use crate::model::document::{Block, DocumentModel, EntryBlock, Placement, SectionId};
use crate::model::rich::{RichText, TextRun};

// ── RenderOpts ────────────────────────────────────────────────────────────────

/// Options controlling the rendered output (page, accent, locale, ATS).
#[derive(Debug, Clone)]
pub struct RenderOpts {
    /// Page geometry (A4 / Letter …).  Defaults to A4.
    pub page: PageGeometry,
    /// Six-digit hex accent colour, with or without `#` prefix.
    /// If absent or invalid the template falls back to its built-in colour.
    pub accent: Option<String>,
    /// BCP-47 language tag driving the font stack (e.g. `"en"`, `"de"`, `"ru"`).
    pub lang: String,
    /// When true the render should be ATS-optimised (linear order, no floats).
    /// For single-column templates this is a no-op — the field is passed so the
    /// template can branch without an API change.
    pub ats: bool,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            page: PageGeometry {
                width_mm: 210.0,
                height_mm: 297.0,
            },
            accent: None,
            lang: "en".to_string(),
            ats: false,
        }
    }
}

// ── Accent validation ─────────────────────────────────────────────────────────

static HEX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#?([0-9a-fA-F]{6})$").unwrap());

/// Validate and normalise a hex colour to `#RRGGBB`.
/// Returns `None` when the input is absent or does not match the pattern.
///
/// The single source of truth for document-accent validation across every
/// backend — the DOCX/cover-letter path in `export::templates` reuses this via
/// the module re-export so PDF and DOCX accept exactly the same inputs.
pub(crate) fn normalise_accent(raw: Option<&str>) -> Option<String> {
    let s = raw?;
    let caps = HEX_RE.captures(s.trim())?;
    Some(format!("#{}", &caps[1]))
}

// ── JSON data model ───────────────────────────────────────────────────────────
// These types are serialised to JSON and consumed by the `.typ` template via
// `#let data = json("data.json")`. Field names intentionally use snake_case
// matching Typst's preferred convention so templates read `data.header.name`.

#[derive(Serialize)]
pub(super) struct JsonTextRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

#[derive(Serialize)]
pub(super) struct JsonEntry {
    pub title: Vec<JsonTextRun>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<Vec<JsonTextRun>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub bullets: Vec<Vec<JsonTextRun>>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum JsonBlock {
    Paragraph { runs: Vec<JsonTextRun> },
    Bullet { runs: Vec<JsonTextRun> },
    Entry(JsonEntry),
}

#[derive(Serialize)]
pub(crate) struct JsonSection {
    pub heading: String,
    pub blocks: Vec<JsonBlock>,
    /// Column placement: `"main"` or `"sidebar"`.
    /// Derived from `theme::placement_for(section.id)` so the `.typ` template can
    /// split sidebar vs main content without re-implementing placement logic.
    pub placement: String,
    /// Canonical section kind tag, serialized from the model `SectionId`.
    /// Values: `"summary"` | `"experience"` | `"education"` | `"skills"` |
    /// `"projects"` | `"certifications"` | `"languages"` | `"awards"` |
    /// `"publications"` | `"volunteer"` | `"interests"` | `"references"` | `"custom"`.
    /// Used by templates to vary rendering per section type (e.g. education de-emphasis).
    pub kind: String,
}

#[derive(Serialize)]
pub(super) struct JsonHeader {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub contact: Vec<JsonTextRun>,
}

#[derive(Serialize)]
pub(super) struct JsonOpts {
    /// Page width in millimetres.
    pub page_width_mm: f32,
    /// Page height in millimetres.
    pub page_height_mm: f32,
    /// Validated accent hex (`#RRGGBB`) or empty string when absent/invalid.
    pub accent: String,
    /// BCP-47 language tag.
    pub lang: String,
    /// ATS flag (bool).
    pub ats: bool,
    /// Whether a `/photo.png` virtual file is available in the World for
    /// embedding.  Templates that support photos check this before calling
    /// `image("photo.png", …)` so they never crash on a missing file.
    pub has_photo: bool,
}

/// Styling parameters derived from the [`Template`] registry, serialized into
/// `data.style` in `data.json`.  Templates read these instead of hardcoding
/// colors/fonts/section-style so a single `single_column.typ` source serves
/// every single-column variant.
///
/// Colors are `#RRGGBB` hex strings.  Font families are mapped to the Typst
/// family name strings that the bundled TTFs are registered under.
#[derive(Serialize)]
pub(super) struct JsonStyle {
    // Colors
    pub c_name: String,
    pub c_section: String,
    pub c_accent: String,
    pub c_body: String,
    pub c_date: String,
    pub c_rule: String,
    // Font families (Typst-registered name)
    pub font_name: String,
    pub font_heading: String,
    pub font_body: String,
    // Section presentation
    /// `"ruled-bottom"` | `"underline"` | `"bold-only"`
    pub section_style: String,
    pub name_centered: bool,
    pub section_all_caps: bool,
    pub section_small_caps: bool,
    pub job_title_italic: bool,
    // Type sizes (points as f32)
    pub name_pt: f32,
    pub section_pt: f32,
    pub body_pt: f32,
    /// When `true`, education entry titles are rendered bold (same as other entries).
    /// When `false` (the default), education entry titles are rendered at normal weight
    /// to de-emphasize them.  Only the academic template sets this to `true`.
    pub emphasize_education: bool,
    /// Extra section-heading letter-spacing (tracking) in em units. `0.0` (every
    /// pre-PR3 template) means no tracking; `single_column.typ` only emits
    /// `text(tracking: …)` when non-zero, so defaults are byte-identical.
    pub heading_tracking: f32,
    /// When `true`, hyperlinked runs are wrapped in `underline(…)`. `false` (every
    /// pre-PR3 template) leaves links un-underlined, byte-identical to prior output.
    pub link_underline: bool,
    /// Section-rule stroke thickness in pt. `single_column.typ` falls back to the
    /// house `0.5pt` when this is absent or `0.0`, so every pre-PR3 ruled template
    /// (all of which are `0.5`) renders byte-identical; only Cadence (`0.75`) sets
    /// a non-default value.
    pub rule_thickness: f32,
}

#[derive(Serialize)]
pub(super) struct JsonData {
    pub opts: JsonOpts,
    pub header: JsonHeader,
    pub sections: Vec<JsonSection>,
    /// Template-derived styling; present when rendering through a known Template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<JsonStyle>,
}

// ── Template → JsonStyle mapping ─────────────────────────────────────────────

/// Map a [`FontFamily`] variant to the Typst font family name string.
fn font_family_to_typst(f: FontFamily) -> &'static str {
    match f {
        FontFamily::Calibri => "Carlito", // metric-compatible substitute
        FontFamily::Inter => "Inter",
        FontFamily::SourceSerif4 => "Source Serif 4",
        FontFamily::Manrope => "Manrope",
    }
}

fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

/// Map a [`SectionId`] to the canonical lowercase tag serialized into `JsonSection.kind`.
pub(crate) fn section_id_to_kind(id: &SectionId) -> String {
    match id {
        SectionId::Summary => "summary",
        SectionId::Experience => "experience",
        SectionId::Education => "education",
        SectionId::Skills => "skills",
        SectionId::Projects => "projects",
        SectionId::Certifications => "certifications",
        SectionId::Languages => "languages",
        SectionId::Awards => "awards",
        SectionId::Publications => "publications",
        SectionId::Volunteer => "volunteer",
        SectionId::Interests => "interests",
        SectionId::References => "references",
        SectionId::Custom(_) => "custom",
    }
    .to_string()
}

/// Build a [`JsonStyle`] from a [`Template`] registry entry.
pub(crate) fn style_from_template(t: &Template) -> JsonStyle {
    let section_style_str = match t.section_style {
        SectionStyle::RuledBottom => "ruled-bottom",
        SectionStyle::Underline => "underline",
        SectionStyle::BoldOnly => "bold-only",
    };
    // Only the Academic template emphasizes education entries (keeps them bold).
    let emphasize_education = t.id == TemplateId::Academic;
    JsonStyle {
        c_name: rgb_to_hex(t.name_color.0, t.name_color.1, t.name_color.2),
        c_section: rgb_to_hex(t.section_color.0, t.section_color.1, t.section_color.2),
        c_accent: rgb_to_hex(t.accent_color.0, t.accent_color.1, t.accent_color.2),
        c_body: rgb_to_hex(t.body_color.0, t.body_color.1, t.body_color.2),
        c_date: rgb_to_hex(t.date_color.0, t.date_color.1, t.date_color.2),
        c_rule: rgb_to_hex(t.rule_color.0, t.rule_color.1, t.rule_color.2),
        font_name: font_family_to_typst(t.fonts.name_family).to_string(),
        font_heading: font_family_to_typst(t.fonts.heading_family).to_string(),
        font_body: font_family_to_typst(t.fonts.body_family).to_string(),
        section_style: section_style_str.to_string(),
        name_centered: t.name_centered,
        section_all_caps: t.section_all_caps,
        section_small_caps: t.section_small_caps,
        job_title_italic: t.job_title_italic,
        name_pt: t.name_pt,
        section_pt: t.section_pt,
        body_pt: t.body_pt,
        emphasize_education,
        heading_tracking: t.heading_tracking,
        link_underline: t.link_underline,
        rule_thickness: t.rule_thickness,
    }
}

// ── Conversion helpers ────────────────────────────────────────────────────────

fn convert_run(r: &TextRun) -> JsonTextRun {
    JsonTextRun {
        text: r.text.clone(),
        bold: r.bold,
        italic: r.italic,
        link: r.link.clone(),
    }
}

fn convert_rich(rt: &RichText) -> Vec<JsonTextRun> {
    rt.iter().map(convert_run).collect()
}

fn convert_entry(e: &EntryBlock) -> JsonEntry {
    JsonEntry {
        title: convert_rich(&e.title),
        subtitle: e.subtitle.as_ref().map(convert_rich),
        date: e.date.clone(),
        bullets: e.bullets.iter().map(convert_rich).collect(),
    }
}

fn convert_block(b: &Block) -> JsonBlock {
    match b {
        Block::Paragraph(rt) => JsonBlock::Paragraph {
            runs: convert_rich(rt),
        },
        Block::Bullet(rt) => JsonBlock::Bullet {
            runs: convert_rich(rt),
        },
        Block::Entry(e) => JsonBlock::Entry(convert_entry(e)),
    }
}

// ── Public surface ────────────────────────────────────────────────────────────

/// The ready-to-render bundle: Typst entry source + raw JSON bytes.
pub(super) struct PreparedRender {
    /// The Typst source that `#import`s the template and loads data.json.
    pub source: String,
    /// The `data.json` bytes to serve via the virtual file system.
    pub data_json: Vec<u8>,
}

/// Build the accessibility document-meta preamble prepended to every rendered
/// Typst source: the PDF **title** (candidate name + document kind), the
/// **author** (candidate name), and the natural-language tag screen readers use
/// to pronounce the content. These come from the document source (not
/// `PdfOptions`); this is a cheap accessibility win short of full PDF/UA.
///
/// Only the constant `doc_kind` label is baked into the markup — the name and
/// language are read from `data.json` at Typst runtime (`name_expr` is the Typst
/// field expression: `data.header.name` for résumés, `data.letterhead.name` for
/// cover letters), so no user content is concatenated into the source
/// (injection-safe, matching the `data.json` boundary this module enforces).
///
/// # `doc_kind` invariant
///
/// `doc_kind` is interpolated verbatim — WITHOUT escaping — into a Typst string
/// literal (`title: <name_expr> + " — {doc_kind}"`). It MUST therefore be a
/// static, quote- and backslash-free label (`"Résumé"`, `"Cover Letter"`) and
/// MUST NEVER carry user data: a `"` or `\` in it would break out of the string
/// literal and corrupt — or inject — Typst source. All current callers pass
/// compile-time constants; keep it that way.
pub(super) fn document_meta_preamble(name_expr: &str, doc_kind: &str) -> String {
    format!(
        "#set document(title: {name_expr} + \" — {doc_kind}\", author: {name_expr})\n\
         #set text(lang: data.opts.lang)\n"
    )
}

/// Build the [`PreparedRender`] for a model + template name + opts.
///
/// `template_source` is the full Typst source for the template (already has the
/// scale preamble prepended by `engine.rs`).  `style_template` is the optional
/// [`Template`] registry entry whose styling should be serialized into
/// `data.style` — pass `None` to omit styling (e.g. for `render_pdf_from_source`
/// smoke tests and for Atelier which manages its own colors internally).
///
/// `has_photo` is set to `true` when the caller has resolved photo bytes and will
/// supply them to [`ResumeWorld::with_data_and_photo`].  This flag is forwarded
/// into `data.opts.has_photo` so the `.typ` template can branch without crashing.
pub(super) fn prepare(
    model: &DocumentModel,
    template_source: &str,
    opts: &RenderOpts,
    style_template: Option<&Template>,
) -> crate::error::AppResult<PreparedRender> {
    prepare_with_photo(model, template_source, opts, style_template, false)
}

/// Like [`prepare`] but accepts `has_photo` to signal that `/photo.png` will be
/// available in the World.
pub(super) fn prepare_with_photo(
    model: &DocumentModel,
    template_source: &str,
    opts: &RenderOpts,
    style_template: Option<&Template>,
    has_photo: bool,
) -> crate::error::AppResult<PreparedRender> {
    let accent_str = normalise_accent(opts.accent.as_deref()).unwrap_or_default();

    // Section placement is per-template (Aria/Saffron override the default table).
    // Fall back to the default table (Classic) when no styling template is passed
    // — only the two-column photo templates ever carry an override anyway.
    let template_id = style_template.map(|t| t.id).unwrap_or_default();

    let json_data = JsonData {
        opts: JsonOpts {
            page_width_mm: opts.page.width_mm,
            page_height_mm: opts.page.height_mm,
            accent: accent_str,
            lang: opts.lang.clone(),
            ats: opts.ats,
            has_photo,
        },
        header: JsonHeader {
            name: model.header.name.clone(),
            title: model.header.title.clone(),
            contact: convert_rich(&model.header.contact),
        },
        sections: model
            .sections
            .iter()
            .map(|s| {
                let p = crate::theme::placement_for(template_id, &s.id);
                JsonSection {
                    heading: s.heading.clone(),
                    blocks: s.blocks.iter().map(convert_block).collect(),
                    placement: if p == Placement::Sidebar {
                        "sidebar".to_string()
                    } else {
                        "main".to_string()
                    },
                    kind: section_id_to_kind(&s.id),
                }
            })
            .collect(),
        style: style_template.map(style_from_template),
    };

    let data_json = serde_json::to_vec(&json_data).map_err(|e| {
        crate::error::AppError::Parse(format!("typst_engine: JSON serialisation failed: {e}"))
    })?;

    // The entry source loads data.json then delegates to the template source.
    // The template source is embedded inline via a virtual include so the world
    // only needs to serve /main.typ and /data.json. The document-meta preamble
    // (PDF title + author + language) is injected before the template so it is in
    // effect before any page content is laid out.
    let meta = document_meta_preamble("data.header.name", "Résumé");
    let source = format!(
        "// Auto-generated entry — do not edit.\n\
         #let data = json(\"data.json\")\n\
         {meta}{template_source}"
    );

    Ok(PreparedRender { source, data_json })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_accent_accepts_hash_prefix() {
        assert_eq!(
            normalise_accent(Some("#1a2b3c")),
            Some("#1a2b3c".to_string())
        );
    }

    #[test]
    fn normalise_accent_accepts_bare_hex() {
        assert_eq!(
            normalise_accent(Some("1A2B3C")),
            Some("#1A2B3C".to_string())
        );
    }

    #[test]
    fn normalise_accent_rejects_invalid() {
        assert_eq!(normalise_accent(Some("red")), None);
        assert_eq!(normalise_accent(Some("#GG0000")), None);
        assert_eq!(normalise_accent(Some("12345")), None);
    }

    #[test]
    fn normalise_accent_none_is_none() {
        assert_eq!(normalise_accent(None), None);
    }

    #[test]
    fn document_meta_preamble_sets_title_author_and_lang() {
        let meta = document_meta_preamble("data.header.name", "Résumé");
        assert!(
            meta.contains(
                "#set document(title: data.header.name + \" — Résumé\", author: data.header.name)"
            ),
            "preamble must set the PDF title + author from the candidate name; got {meta:?}"
        );
        assert!(
            meta.contains("#set text(lang: data.opts.lang)"),
            "preamble must set the document language for screen readers; got {meta:?}"
        );
    }
}
