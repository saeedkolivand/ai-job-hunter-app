use super::types::{FontFamily, TemplateId};

/// Every user-facing template, in gallery order — the one list test matrices
/// iterate so a newly added template is covered automatically instead of
/// needing a remembered edit in each of them.
///
/// Kept honest by `canonical_template_ids_are_unique_and_self_describing`:
/// [`Template::get`] matches on `TemplateId` with no wildcard arm, and that
/// test pins that every entry here really resolves to its own template.
#[cfg(test)]
pub(crate) const CANONICAL_TEMPLATE_IDS: [TemplateId; 16] = [
    TemplateId::Classic,
    TemplateId::SwissMinimal,
    TemplateId::Academic,
    TemplateId::Atelier,
    TemplateId::Meridian,
    TemplateId::Throughline,
    TemplateId::Portrait,
    TemplateId::Lebenslauf,
    TemplateId::Cadence,
    TemplateId::Regent,
    TemplateId::Aria,
    TemplateId::Saffron,
    TemplateId::CologneNavy,
    TemplateId::Jake,
    TemplateId::Awesome,
    TemplateId::Deedy,
];

// Registry: template constructors, grouped by the phase/track that introduced
// them (issue #1280 batch 5b split) — each group gets its own `impl Template`
// block; inherent impls may be split across modules in the same crate.
mod community;
mod core;
mod photo;
mod pr3_ats;
mod pr4_design;
mod premium_single_column;

// ─── Font configuration ───────────────────────────────────────────────────────

/// Which font families a template uses for its three typographic roles.
#[derive(Debug, Clone, Copy)]
pub struct TemplateFonts {
    pub name_family: FontFamily,
    pub heading_family: FontFamily,
    pub body_family: FontFamily,
}

impl Default for TemplateFonts {
    fn default() -> Self {
        Self {
            name_family: FontFamily::Calibri,
            heading_family: FontFamily::Calibri,
            body_family: FontFamily::Calibri,
        }
    }
}

// ─── Two-column configuration ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TwoColumnConfig {
    /// Fraction of the content width reserved for the sidebar (e.g. 0.30 = 30 %).
    pub sidebar_width_ratio: f32,
    /// Background tint of the sidebar column (RGB).
    pub sidebar_bg_color: (u8, u8, u8),
}

// ─── Cover letter configuration ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParagraphIndent {
    /// Classical: 0.25 in first-line indent, no extra space between paragraphs.
    FirstLine,
    /// Modern: no indent, blank-line-equivalent spacing between paragraphs.
    BlockNoIndent,
}

/// Cover-letter layout knobs still read by the DOCX letter renderer. (Date
/// placement, recipient block, and sign-off come from `LetterMarketConventions`
/// in the Typst letter path, so they're no longer template config.)
#[derive(Debug, Clone)]
pub struct CoverLetterLayout {
    pub paragraph_indent: ParagraphIndent,
    /// Extra vertical space (pt) after each block-indent paragraph.
    pub paragraph_spacing_pt: f32,
}

impl Default for CoverLetterLayout {
    fn default() -> Self {
        Self {
            paragraph_indent: ParagraphIndent::BlockNoIndent,
            paragraph_spacing_pt: 8.0,
        }
    }
}

// ─── Template styling configuration ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SectionStyle {
    RuledBottom,
    Underline,
    BoldOnly,
}

/// ATS-safe vs. design tier — metadata only, no render behavior.
///
/// Drives the frontend gallery grouping (ATS-Safe / Design sections + badge) and
/// which templates surface the ATS-mode toggle: design-tier layouts (photo /
/// two-column) drop the photo and linearize when ATS mode is on. Not serialized
/// to the renderer (`JsonStyle` is unchanged); the frontend registry mirrors it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateTier {
    /// Single-column, parser-safe layouts.
    Ats,
    /// Photo / two-column / visually rich layouts.
    Design,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Template {
    pub id: TemplateId,
    pub name: &'static str,
    /// ATS-safe vs. design tier (metadata; see [`TemplateTier`]).
    pub tier: TemplateTier,

    // Colors (RGB tuples)
    pub name_color: (u8, u8, u8),
    pub section_color: (u8, u8, u8),
    pub accent_color: (u8, u8, u8),
    pub body_color: (u8, u8, u8),
    pub date_color: (u8, u8, u8),
    pub emphasis_color: (u8, u8, u8),
    pub rule_color: (u8, u8, u8),

    // Font sizes (points)
    pub name_pt: f32,
    pub section_pt: f32,
    pub body_pt: f32,

    // Margins (inches)
    pub margin_in: f32,

    // Spacing
    pub line_spacing: f32,
    pub section_spacing_before: f32,

    // Style options (existing)
    pub name_centered: bool,
    pub section_all_caps: bool,
    pub section_style: SectionStyle,

    // New style options
    pub fonts: TemplateFonts,
    pub job_title_italic: bool,
    /// When true: wrap section heading text in Typst `smallcaps(…)` (small-caps
    /// glyph variant; the PDF text layer keeps its original case — extraction-
    /// safe) and render at 0.85 × section_pt. Read into `JsonStyle.section_small_caps`;
    /// see `single_column.typ`'s `render-section` for the actual wrap.
    pub section_small_caps: bool,
    /// Section-rule stroke thickness in pt, read by the `single_column.typ`
    /// ruled-bottom branch as `stroke: (rule_thickness * 1pt) + c-rule`.
    /// `single_column.typ` falls back to the house `0.5pt` when this is `0.0` or
    /// the style is absent — every pre-PR3 ruled template ships `0.5`, so this is
    /// byte-identical for them; only Cadence sets a real `0.75` override.
    /// NOTE: `0.0` means "default thickness", NOT "no rule" — rule *presence*
    /// is owned by `section_style` (`BoldOnly` = no rule). Don't set `0.0` on a
    /// `RuledBottom` template expecting suppression.
    pub rule_thickness: f32,
    /// Extra letter-spacing (tracking) applied to section headings, in em units.
    /// `0.0` (the default for every pre-PR3 template) leaves headings untracked —
    /// `single_column.typ` only emits `text(tracking: …)` when this is non-zero, so
    /// existing output is byte-identical. Read into `JsonStyle.heading_tracking`.
    pub heading_tracking: f32,
    /// When true, wrap hyperlinked runs in `underline(…)` in the single-column
    /// renderer. `false` (the default for every pre-PR3 template) leaves links
    /// un-underlined, byte-identical to prior output. Read into
    /// `JsonStyle.link_underline`.
    pub link_underline: bool,
    /// Extra space (pt) above every section heading, ADDED to the shared
    /// `_scale.typ` `sp-section-above`. `0.0` (every template but Deedy) keeps
    /// the house rhythm exactly as it is.
    ///
    /// This knob exists because `_scale.typ` is the single locked source of the
    /// vertical rhythm: a template that wants a wider one declares it here (the
    /// same shape as [`Template::rule_thickness`] / [`Template::heading_tracking`])
    /// rather than defining a local spacing constant in its own `.typ`.
    pub section_above_extra: f32,

    // Two-column layout (None = single column)
    pub two_column: Option<TwoColumnConfig>,

    // Cover letter paired layout
    pub cover_letter: CoverLetterLayout,
}

/// Parse a **document accent** hex (`#RRGGBB` or bare `RRGGBB`) into an RGB
/// tuple. Delegates validation to `typst_engine::normalise_accent` — the single
/// source of truth the résumé-PDF `RenderOpts.accent` path uses — so every
/// backend (PDF résumé, cover letter, DOCX) accepts exactly the same inputs.
/// Returns `None` for an absent or malformed value.
fn parse_accent_rgb(accent: Option<&str>) -> Option<(u8, u8, u8)> {
    // `normalise_accent` returns a canonical `#RRGGBB` string when valid.
    let normalized = super::typst_engine::normalise_accent(accent)?;
    let hex = &normalized[1..]; // drop the leading '#'
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

impl Template {
    /// Get template by ID.
    pub fn get(id: TemplateId) -> Self {
        match id {
            TemplateId::Classic => Self::classic(),
            TemplateId::SwissMinimal => Self::swiss_minimal(),
            TemplateId::Academic => Self::academic(),
            TemplateId::Atelier => Self::atelier(),
            TemplateId::Meridian => Self::meridian(),
            TemplateId::Throughline => Self::throughline(),
            TemplateId::Portrait => Self::portrait(),
            TemplateId::Lebenslauf => Self::lebenslauf(),
            TemplateId::Cadence => Self::cadence(),
            TemplateId::Regent => Self::regent(),
            TemplateId::CologneNavy => Self::cologne_navy(),
            TemplateId::Aria => Self::aria(),
            TemplateId::Saffron => Self::saffron(),
            TemplateId::Jake => Self::jake(),
            TemplateId::Awesome => Self::awesome(),
            TemplateId::Deedy => Self::deedy(),
        }
    }

    /// Apply a per-export **document accent** override (ADR 0004): when `accent`
    /// is a valid 6-digit hex (`#RRGGBB` or bare `RRGGBB`), recolor the
    /// accent-derived fields so the DOCX backend (`docx_renderer::setup_colors`
    /// reads `emphasis_color`) and the cover-letter style
    /// (`typst_engine::letter::style_from_template` reads `accent_color`) pick it
    /// up. The résumé-PDF path instead threads the same hex through
    /// `RenderOpts.accent` (the `.typ` prefers `data.opts.accent`). No-op when the
    /// value is absent or malformed — the template keeps its built-in palette.
    #[must_use]
    pub fn with_accent_override(mut self, accent: Option<&str>) -> Self {
        if let Some(rgb) = parse_accent_rgb(accent) {
            self.accent_color = rgb;
            self.emphasis_color = rgb;
        }
        self
    }
}

/// Calculate dynamic spacing based on content type and context.
pub fn calculate_spacing(
    current_kind: &super::types::LineKind,
    previous_kind: Option<&super::types::LineKind>,
) -> (f32, f32) {
    use super::types::LineKind;

    // Returns (before, after) in points
    match current_kind {
        LineKind::SectionHeader => (12.0, 3.0),
        LineKind::JobEntry => match previous_kind {
            Some(LineKind::Bullet) | Some(LineKind::JobTitle) => (8.0, 1.0),
            _ => (6.0, 1.0),
        },
        LineKind::JobTitle => (0.0, 3.0),
        LineKind::Bullet => match previous_kind {
            Some(LineKind::Bullet) => (0.0, 2.0),
            _ => (3.0, 2.0),
        },
        LineKind::Contact => (0.0, 0.0),
        LineKind::Name => (0.0, 2.0),
        _ => (0.0, 4.0),
    }
}

#[cfg(test)]
mod tests;
