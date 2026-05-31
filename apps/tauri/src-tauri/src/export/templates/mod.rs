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
    /// Section names whose content is placed in the sidebar column.
    pub sidebar_sections: Vec<&'static str>,
}

// ─── Cover letter configuration ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverLetterHeader {
    /// Matches the resume header exactly — name + contact line + accent rule.
    Matched,
    /// Name only, single bold line, no contact details.
    Compact,
    /// Name centered in display type, no rule.
    Centered,
    /// Full letterhead block with name, contact stack, and hairline rule.
    Letterhead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DatePosition {
    TopRight,
    BelowHeader,
    AboveSalutation,
    Omitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParagraphIndent {
    /// Classical: 0.25 in first-line indent, no extra space between paragraphs.
    FirstLine,
    /// Modern: no indent, blank-line-equivalent spacing between paragraphs.
    BlockNoIndent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStyle {
    /// Just typed name in body font.
    TypedOnly,
    /// Typed name (bold) + professional title underneath.
    NameAndTitle,
    /// Name in the template's name_family italic — mimics a handwritten signature.
    ScriptStyle,
}

#[derive(Debug, Clone)]
pub struct CoverLetterLayout {
    pub header_style: CoverLetterHeader,
    pub date_position: DatePosition,
    /// Whether to include a recipient/addressee block.
    pub recipient_block: bool,
    pub paragraph_indent: ParagraphIndent,
    /// Extra vertical space (pt) after each block-indent paragraph.
    pub paragraph_spacing_pt: f32,
    pub signature_block: SignatureStyle,
    /// Default closing phrase — overridden by user input if provided.
    pub closing_phrase_default: &'static str,
}

impl Default for CoverLetterLayout {
    fn default() -> Self {
        Self {
            header_style: CoverLetterHeader::Matched,
            date_position: DatePosition::BelowHeader,
            recipient_block: true,
            paragraph_indent: ParagraphIndent::BlockNoIndent,
            paragraph_spacing_pt: 8.0,
            signature_block: SignatureStyle::TypedOnly,
            closing_phrase_default: "Sincerely,",
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
            TemplateId::Executive => Self::executive(),
            TemplateId::EditorialSerif => Self::editorial_serif(),
            TemplateId::SwissMinimal => Self::swiss_minimal(),
            TemplateId::TwoColumn => Self::two_column(),
            TemplateId::MonoTechnical => Self::mono_technical(),
            TemplateId::RefinedExecutive => Self::refined_executive(),
            TemplateId::Academic => Self::academic(),
        }
    }

    // ─── Existing three templates ─────────────────────────────────────────────

    /// ATS Classic — maximum compatibility, no color, safe for all ATS parsers.
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
            section_style: SectionStyle::Underline,
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

    /// Executive — minimalist, charcoal, premium whitespace for senior roles.
    pub(super) fn executive() -> Self {
        Self {
            id: TemplateId::Executive,
            name: "Executive",
            name_color: (28, 28, 28),
            section_color: (44, 44, 44),
            accent_color: (68, 68, 68),
            body_color: (44, 44, 44),
            date_color: (128, 128, 128),
            emphasis_color: (28, 28, 28),
            rule_color: (204, 204, 204),
            name_pt: 24.0,
            section_pt: 10.5,
            body_pt: 10.5,
            margin_in: 1.1,
            line_spacing: 1.25,
            section_spacing_before: 15.0,
            name_centered: true,
            section_all_caps: false,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts::default(),
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 1.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                header_style: CoverLetterHeader::Centered,
                paragraph_indent: ParagraphIndent::FirstLine,
                ..CoverLetterLayout::default()
            },
        }
    }

    // ─── Six new templates ────────────────────────────────────────────────────

    /// Editorial Serif — NYT op-ed character, Source Serif 4 + Inter, deep indigo accent.
    fn editorial_serif() -> Self {
        Self {
            id: TemplateId::EditorialSerif,
            name: "Editorial Serif",
            name_color: (26, 26, 26),
            section_color: (45, 43, 85),
            accent_color: (45, 43, 85),
            body_color: (26, 26, 26),
            date_color: (90, 90, 90),
            emphasis_color: (45, 43, 85),
            rule_color: (45, 43, 85),
            name_pt: 22.0,
            section_pt: 11.0,
            body_pt: 11.0,
            margin_in: 1.0,
            line_spacing: 1.2,
            section_spacing_before: 13.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::SourceSerif4,
                heading_family: FontFamily::SourceSerif4,
                body_family: FontFamily::Inter,
            },
            job_title_italic: true,
            section_small_caps: true,
            rule_thickness: 0.25,
            two_column: None,
            cover_letter: CoverLetterLayout {
                header_style: CoverLetterHeader::Letterhead,
                date_position: DatePosition::TopRight,
                recipient_block: true,
                paragraph_indent: ParagraphIndent::FirstLine,
                paragraph_spacing_pt: 0.0,
                signature_block: SignatureStyle::ScriptStyle,
                closing_phrase_default: "Sincerely,",
            },
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
                header_style: CoverLetterHeader::Compact,
                date_position: DatePosition::BelowHeader,
                recipient_block: false,
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 12.0,
                signature_block: SignatureStyle::TypedOnly,
                closing_phrase_default: "Best regards,",
            },
        }
    }

    /// Two Column — Inter, light sidebar tint, contact/skills/education in sidebar.
    fn two_column() -> Self {
        Self {
            id: TemplateId::TwoColumn,
            name: "Two Column",
            name_color: (20, 20, 20),
            section_color: (30, 64, 175),
            accent_color: (30, 64, 175),
            body_color: (30, 30, 30),
            date_color: (100, 100, 120),
            emphasis_color: (30, 64, 175),
            rule_color: (180, 200, 240),
            name_pt: 22.0,
            section_pt: 10.5,
            body_pt: 10.0,
            margin_in: 0.5,
            line_spacing: 1.15,
            section_spacing_before: 10.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::BoldOnly,
            fonts: TemplateFonts {
                name_family: FontFamily::Inter,
                heading_family: FontFamily::Inter,
                body_family: FontFamily::Inter,
            },
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.5,
            two_column: Some(TwoColumnConfig {
                sidebar_width_ratio: 0.30,
                sidebar_bg_color: (240, 244, 248),
                sidebar_sections: vec![
                    "Contact",
                    "Skills",
                    "Education",
                    "Languages",
                    "Kontakt",
                    "Kenntnisse",
                    "Ausbildung",
                    "Sprachen",
                ],
            }),
            cover_letter: CoverLetterLayout {
                header_style: CoverLetterHeader::Matched,
                date_position: DatePosition::BelowHeader,
                recipient_block: true,
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
                signature_block: SignatureStyle::TypedOnly,
                closing_phrase_default: "Best,",
            },
        }
    }

    /// Mono Technical — JetBrains Mono headings, Inter body, cyan accent.
    fn mono_technical() -> Self {
        Self {
            id: TemplateId::MonoTechnical,
            name: "Mono Technical",
            name_color: (10, 10, 10),
            section_color: (0, 150, 180),
            accent_color: (0, 180, 216),
            body_color: (30, 30, 30),
            date_color: (100, 120, 130),
            emphasis_color: (0, 150, 180),
            rule_color: (0, 180, 216),
            name_pt: 20.0,
            section_pt: 10.5,
            body_pt: 10.5,
            margin_in: 1.0,
            line_spacing: 1.2,
            section_spacing_before: 12.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::JetBrainsMono,
                heading_family: FontFamily::JetBrainsMono,
                body_family: FontFamily::Inter,
            },
            job_title_italic: false,
            section_small_caps: false,
            rule_thickness: 1.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                header_style: CoverLetterHeader::Compact,
                date_position: DatePosition::TopRight,
                recipient_block: false,
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
                signature_block: SignatureStyle::TypedOnly,
                closing_phrase_default: "Regards,",
            },
        }
    }

    /// Refined Executive — Playfair Display name, Inter body, warm gold accent.
    fn refined_executive() -> Self {
        Self {
            id: TemplateId::RefinedExecutive,
            name: "Refined Executive",
            name_color: (20, 20, 20),
            section_color: (100, 80, 50),
            accent_color: (139, 115, 85),
            body_color: (40, 38, 35),
            date_color: (120, 110, 95),
            emphasis_color: (100, 80, 50),
            rule_color: (200, 185, 160),
            name_pt: 26.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 1.1,
            line_spacing: 1.25,
            section_spacing_before: 15.0,
            name_centered: true,
            section_all_caps: false,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::PlayfairDisplay,
                heading_family: FontFamily::Inter,
                body_family: FontFamily::Inter,
            },
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.5,
            two_column: None,
            cover_letter: CoverLetterLayout {
                header_style: CoverLetterHeader::Centered,
                date_position: DatePosition::BelowHeader,
                recipient_block: true,
                paragraph_indent: ParagraphIndent::FirstLine,
                paragraph_spacing_pt: 0.0,
                signature_block: SignatureStyle::NameAndTitle,
                closing_phrase_default: "Sincerely yours,",
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
            section_style: SectionStyle::Underline,
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
                header_style: CoverLetterHeader::Letterhead,
                date_position: DatePosition::TopRight,
                recipient_block: true,
                paragraph_indent: ParagraphIndent::FirstLine,
                paragraph_spacing_pt: 0.0,
                signature_block: SignatureStyle::NameAndTitle,
                closing_phrase_default: "Sincerely,",
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
