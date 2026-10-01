//! Cover-letter structured data model (serialised into `data.json`) + page-geometry
//! constants, split out of `letter.rs` to stay under the R8 module-size cap.

use serde::Serialize;

use crate::locale::letter::conventions;
use crate::model::rich::TextRun;

// ── Serialisable rich-text run (mirrors render::JsonTextRun) ──────────────────
//
// Duplicated here so the letter module is self-contained and does NOT depend
// on `render.rs` types (avoids circular module dependencies while keeping the
// JSON shape identical).

#[derive(Debug, Clone, Serialize)]
pub(in crate::export::typst_engine) struct LetterRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl From<&TextRun> for LetterRun {
    fn from(r: &TextRun) -> Self {
        Self {
            text: r.text.clone(),
            bold: r.bold,
            italic: r.italic,
            link: r.link.clone(),
        }
    }
}

// ── LetterModel ───────────────────────────────────────────────────────────────

/// Structured cover-letter model ready for JSON serialisation into `data.json`.
///
/// All fields that the template may not need are `Option`; only `body` and
/// `opts` are always present. Missing parts degrade gracefully — the template
/// guards every optional key with `"k" in d`.
#[derive(Debug, Serialize)]
pub(in crate::export::typst_engine) struct LetterModel {
    pub opts: LetterOpts,
    pub style: LetterStyle,
    pub letterhead: LetterHead,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub recipient_lines: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salutation: Option<String>,
    /// Body paragraphs as rich-text runs so **bold** survives.
    pub body: Vec<Vec<LetterRun>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signoff: Option<String>,
    pub signature_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_title: Option<String>,
}

/// Page + locale options passed through to the template.
#[derive(Debug, Serialize)]
pub(in crate::export::typst_engine) struct LetterOpts {
    pub page_width_mm: f32,
    pub page_height_mm: f32,
    pub lang: String,
    /// `"top-right"` | `"below-header"` | `"above-salutation"`.
    pub date_position: String,
    /// `"top"` | `"bottom"`.
    pub sender_position: String,
    /// Horizontal-placement hint from the shared fixture (`"left"` | `"top-right"`),
    /// passed through verbatim. Not currently read by any `.typ` template — every
    /// letter layout (Classic/Refined/Banded) renders the recipient block
    /// unconditionally, left-aligned, right after the date. (fr's true top-right
    /// recipient layout is not implemented — a real top-right variant is a
    /// possible follow-up; kept in the data contract for that future use.)
    pub recipient_position: String,
    pub subject_line_used: bool,
    pub subject_line_label: String,
    /// ATS mode — the request's `ats_mode`, threaded through so a layout can
    /// drop its **decorative, non-semantic** elements (Sidebar's tinted rail,
    /// Monogram's initials device, Banded's accent band) while keeping every
    /// word of the letter.
    ///
    /// This is the letter-side counterpart of the résumé's `RenderOpts.ats` and
    /// `theme::has_header_band`: "both renderers drop it in ATS mode". Layouts
    /// gate on THIS, never on the layout id — the id picks the arrangement, the
    /// opts decide which parts of it are legal for this export.
    pub ats: bool,
}

/// Letterhead block: candidate name + tokenised contact runs.
#[derive(Debug, Serialize)]
pub(in crate::export::typst_engine) struct LetterHead {
    pub name: String,
    /// Rich-text runs for the contact line (links first-class).
    pub contact: Vec<LetterRun>,
    /// Up to two uppercase initials derived from [`Self::name`] — see
    /// `letterhead::monogram_initials`. Part of the shared model (any layout
    /// may use it); only `letter_monogram.typ` renders it today, and only
    /// outside ATS mode.
    pub initials: String,
}

/// Styling pulled from the chosen resume [`Template`] so the letter visually
/// matches the resume family.
#[derive(Debug, Serialize)]
pub(in crate::export::typst_engine) struct LetterStyle {
    /// Validated accent hex (`#RRGGBB`).
    pub c_accent: String,
    /// Body colour hex.
    pub c_body: String,
    /// Name colour hex.
    pub c_name: String,
    /// Date / muted colour hex.
    pub c_date: String,
    /// Rule colour hex.
    pub c_rule: String,
    /// Typst font family name for the heading / name.
    pub font_name: String,
    /// Typst font family name for the body.
    pub font_body: String,
    pub name_pt: f32,
    pub body_pt: f32,
}

// ── Page geometry constants ───────────────────────────────────────────────────

/// A4 dimensions in mm.
pub(super) const A4_W: f32 = 210.0;
pub(super) const A4_H: f32 = 297.0;
/// US Letter dimensions in mm.
pub(super) const LETTER_W: f32 = 215.9;
pub(super) const LETTER_H: f32 = 279.4;

// ── Parser ────────────────────────────────────────────────────────────────────

/// Resolve page geometry from the market's page field (`"a4"` or `"letter"`).
pub(super) fn page_dims(market: &str) -> (f32, f32) {
    let conv = conventions(market);
    if conv.page == "letter" {
        (LETTER_W, LETTER_H)
    } else {
        (A4_W, A4_H)
    }
}
