use super::types::{FontFamily, TemplateId};

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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Template {
    pub id: TemplateId,
    pub name: &'static str,

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
    /// When true: uppercase section header text + render at 0.85 × section_pt.
    pub section_small_caps: bool,
    /// Section rule thickness in pt (0.0 = no rule).
    pub rule_thickness: f32,

    // Two-column layout (None = single column)
    pub two_column: Option<TwoColumnConfig>,

    // Cover letter paired layout
    pub cover_letter: CoverLetterLayout,
}

impl Template {
    /// Get template by ID.
    pub fn get(id: TemplateId) -> Self {
        match id {
            TemplateId::Classic => Self::classic(),
            TemplateId::Modern => Self::modern(),
            TemplateId::SwissMinimal => Self::swiss_minimal(),
            TemplateId::Academic => Self::academic(),
            TemplateId::Atelier => Self::atelier(),
            TemplateId::Meridian => Self::meridian(),
            TemplateId::Throughline => Self::throughline(),
            TemplateId::Portrait => Self::portrait(),
            TemplateId::Lebenslauf => Self::lebenslauf(),
        }
    }

    // ─── Nine live templates ──────────────────────────────────────────────────

    /// ATS Classic — maximum compatibility, no color, safe for all ATS parsers.
    ///
    /// `section_style` is [`SectionStyle::RuledBottom`] because `classic.typ`
    /// renders a full-width rule below each section heading (not just an
    /// underline on the text).  Keeping declaration and render in sync avoids
    /// misleading callers that inspect this field.
    pub(super) fn classic() -> Self {
        Self {
            id: TemplateId::Classic,
            name: "ATS Classic",
            name_color: (17, 17, 17),
            section_color: (17, 17, 17),
            accent_color: (34, 34, 34),
            body_color: (34, 34, 34),
            date_color: (85, 85, 85),
            emphasis_color: (0, 0, 0),
            rule_color: (170, 170, 170),
            name_pt: 20.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 1.0,
            line_spacing: 1.15,
            section_spacing_before: 12.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts::default(),
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.5,
            two_column: None,
            cover_letter: CoverLetterLayout::default(),
        }
    }

    /// Modern Technical — clean navy, professional, best for tech roles.
    pub(super) fn modern() -> Self {
        Self {
            id: TemplateId::Modern,
            name: "Modern Technical",
            name_color: (13, 31, 60),
            section_color: (13, 31, 60),
            accent_color: (26, 58, 107),
            body_color: (26, 26, 46),
            date_color: (107, 107, 138),
            emphasis_color: (13, 61, 107),
            rule_color: (184, 196, 220),
            name_pt: 22.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 1.0,
            line_spacing: 1.2,
            section_spacing_before: 13.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts::default(),
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 1.0,
            two_column: None,
            cover_letter: CoverLetterLayout::default(),
        }
    }

    /// Swiss Minimal — Manrope, red accent, generous whitespace, almost empty page.
    fn swiss_minimal() -> Self {
        Self {
            id: TemplateId::SwissMinimal,
            name: "Swiss Minimal",
            name_color: (20, 20, 20),
            section_color: (20, 20, 20),
            accent_color: (230, 57, 70),
            body_color: (40, 40, 40),
            date_color: (120, 120, 120),
            emphasis_color: (20, 20, 20),
            rule_color: (230, 57, 70),
            name_pt: 22.0,
            section_pt: 10.5,
            body_pt: 10.5,
            margin_in: 1.15,
            line_spacing: 1.3,
            section_spacing_before: 16.0,
            name_centered: false,
            section_all_caps: false,
            section_style: SectionStyle::BoldOnly,
            fonts: TemplateFonts {
                name_family: FontFamily::Manrope,
                heading_family: FontFamily::Manrope,
                body_family: FontFamily::Manrope,
            },
            job_title_italic: false,
            section_small_caps: false,
            rule_thickness: 0.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 12.0,
            },
        }
    }

    /// Academic — Source Serif 4 throughout, forest green accent, formal block letter.
    fn academic() -> Self {
        Self {
            id: TemplateId::Academic,
            name: "Academic",
            name_color: (20, 40, 30),
            section_color: (27, 67, 50),
            accent_color: (27, 67, 50),
            body_color: (30, 30, 30),
            date_color: (90, 110, 100),
            emphasis_color: (27, 67, 50),
            rule_color: (100, 150, 120),
            name_pt: 20.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 0.85,
            line_spacing: 1.1,
            section_spacing_before: 12.0,
            name_centered: false,
            section_all_caps: false,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::SourceSerif4,
                heading_family: FontFamily::SourceSerif4,
                body_family: FontFamily::SourceSerif4,
            },
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.5,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::FirstLine,
                paragraph_spacing_pt: 0.0,
            },
        }
    }

    /// Atelier — premium two-column sidebar template.
    ///
    /// Design: slate-indigo accent (#4A4580), Source Serif 4 main column,
    /// Inter sidebar, full-height sidebar band at 30 % page width. Sidebar
    /// tint is a very light warm grey (#F0EFF8) that complements the indigo.
    /// Skills / Education / Languages / Certifications go to the sidebar via
    /// `theme::placement_for`; everything else flows in the main column.
    ///
    /// Phase 1b: Typst engine only — not yet wired into the live export flow.
    fn atelier() -> Self {
        Self {
            id: TemplateId::Atelier,
            name: "Atelier",
            // Slate-indigo palette: a deep, sophisticated purple-grey.
            name_color: (22, 20, 54),
            section_color: (74, 69, 128),
            accent_color: (74, 69, 128),
            body_color: (30, 28, 50),
            date_color: (110, 105, 145),
            emphasis_color: (74, 69, 128),
            rule_color: (180, 175, 220),
            name_pt: 22.0,
            section_pt: 10.5,
            body_pt: 10.5,
            margin_in: 0.55,
            line_spacing: 1.2,
            section_spacing_before: 11.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                // Main column: editorial serif body; sidebar: clean sans.
                name_family: FontFamily::SourceSerif4,
                heading_family: FontFamily::SourceSerif4,
                body_family: FontFamily::Inter,
            },
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.5,
            two_column: Some(TwoColumnConfig {
                sidebar_width_ratio: 0.30,
                // Very light warm-grey tint that pairs with the slate-indigo accent.
                sidebar_bg_color: (240, 239, 248),
            }),
            // Cover letter mirrors modern's layout (not yet specialized for Atelier).
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }

    // ─── Phase 3a premium single-column templates ─────────────────────────────

    /// Meridian — header-forward band.
    ///
    /// Design: a full-width tinted header band holds the name, title, and contact
    /// line. Accent: warm copper-sienna (#A0522D) — distinct from Atelier's indigo.
    /// Font: Inter throughout (clean, modern sans). Below the band: airy single-
    /// column body using the house rhythm. Section headings in the accent.
    /// Cover-letter layout mirrors `modern`.
    pub(super) fn meridian() -> Self {
        Self {
            id: TemplateId::Meridian,
            name: "Meridian",
            // Warm copper-sienna palette — original, professional.
            name_color: (255, 255, 255),  // white on the dark band
            section_color: (160, 82, 45), // copper-sienna for section headings
            accent_color: (160, 82, 45),  // copper-sienna accent
            body_color: (30, 25, 20),     // near-black warm body
            date_color: (120, 100, 80),   // muted warm brown dates
            emphasis_color: (160, 82, 45),
            rule_color: (210, 170, 140), // soft copper rule
            name_pt: 26.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 0.0, // controlled per-zone in the template
            line_spacing: 1.2,
            section_spacing_before: 13.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::Inter,
                heading_family: FontFamily::Inter,
                body_family: FontFamily::Inter,
            },
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.5,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }

    /// Throughline — timeline spine.
    ///
    /// Design: a thin vertical spine with a filled dot per entry in EXPERIENCE and
    /// PROJECTS sections. Other sections render as normal single-column blocks.
    /// Accent: deep forest teal (#1A5C52) — cool, grounded, original.
    /// Font: Carlito body, Manrope headings.
    /// Cover-letter layout mirrors `modern`.
    pub(super) fn throughline() -> Self {
        Self {
            id: TemplateId::Throughline,
            name: "Throughline",
            // Deep forest-teal palette.
            name_color: (15, 50, 45),    // very dark teal name
            section_color: (26, 92, 82), // forest teal sections
            accent_color: (26, 92, 82),  // forest teal accent (spine + nodes)
            body_color: (25, 35, 32),    // near-black cool body
            date_color: (85, 120, 110),  // muted teal dates
            emphasis_color: (26, 92, 82),
            rule_color: (160, 200, 190), // light teal rule
            name_pt: 22.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 1.0,
            line_spacing: 1.2,
            section_spacing_before: 13.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::Manrope,
                heading_family: FontFamily::Manrope,
                body_family: FontFamily::Calibri,
            },
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.5,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }

    // ─── Phase 3b-i photo templates ───────────────────────────────────────────

    /// Portrait — circular photo top-left, name/title stacked right, accent
    /// keyline, two-column sidebar for contact/skills/education.
    ///
    /// Design: deep slate-teal accent (#2A6478), circular photo top-left,
    /// Inter throughout, sidebar 30 % width.  When no photo: graceful monogram
    /// / name-only fallback so the header never looks broken.
    /// Phase 3b-i: Typst-only; not yet wired into the live export flow.
    pub(super) fn portrait() -> Self {
        Self {
            id: TemplateId::Portrait,
            name: "Portrait",
            // Deep slate-teal palette — professional, original.
            name_color: (18, 40, 50),
            section_color: (42, 100, 120),
            accent_color: (42, 100, 120),
            body_color: (28, 30, 32),
            date_color: (90, 110, 120),
            emphasis_color: (42, 100, 120),
            rule_color: (160, 195, 210),
            name_pt: 22.0,
            section_pt: 10.5,
            body_pt: 10.5,
            margin_in: 0.55,
            line_spacing: 1.2,
            section_spacing_before: 11.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::Inter,
                heading_family: FontFamily::Inter,
                body_family: FontFamily::Inter,
            },
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.5,
            // Two-column: sidebar holds contact, skills, education, languages,
            // certifications — same set as Atelier.
            two_column: Some(TwoColumnConfig {
                sidebar_width_ratio: 0.30,
                // Very light teal tint for the sidebar.
                sidebar_bg_color: (235, 244, 248),
            }),
            // Cover letter mirrors modern layout.
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }

    /// Lebenslauf — DACH DIN-style tabular CV.
    ///
    /// Design: warm slate accent (#3D4F6B), formal A4, photo top-right,
    /// left-label / right-value rows, Carlito body, restrained accent.
    /// When no photo: text-only formal header.
    /// Phase 3b-i: Typst-only; not yet wired into the live export flow.
    pub(super) fn lebenslauf() -> Self {
        Self {
            id: TemplateId::Lebenslauf,
            name: "Lebenslauf",
            // Warm slate palette — formal, DACH-appropriate.
            name_color: (20, 25, 35),
            section_color: (61, 79, 107),
            accent_color: (61, 79, 107),
            body_color: (30, 30, 35),
            date_color: (100, 110, 125),
            emphasis_color: (61, 79, 107),
            rule_color: (180, 190, 210),
            name_pt: 20.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 0.85,
            line_spacing: 1.15,
            section_spacing_before: 13.0,
            name_centered: false,
            section_all_caps: false, // DIN style: normal-case section headings
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::Calibri,
                heading_family: FontFamily::Calibri,
                body_family: FontFamily::Calibri,
            },
            job_title_italic: false, // DIN style: no italic job titles
            section_small_caps: false,
            rule_thickness: 0.5,
            // Single-column — DIN tabular layout manages its own columns.
            two_column: None,
            // Cover letter mirrors modern layout (appropriate for DIN letters).
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
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
mod test;
