//! The original launch roster: ATS Classic/Swiss Minimal/Academic, plus the
//! first premium two-column sidebar design (Atelier).

use crate::export::types::{FontFamily, TemplateId};

use super::{
    CoverLetterLayout, ParagraphIndent, SectionStyle, Template, TemplateFonts, TemplateTier,
    TwoColumnConfig,
};

impl Template {
    /// ATS Classic — maximum compatibility, no color, safe for all ATS parsers.
    ///
    /// Classic renders through the parametric `single_column.typ`; its
    /// `section_style` is [`SectionStyle::RuledBottom`] so the template draws a
    /// full-width rule below each section heading (not just an underline on the
    /// text).  Keeping declaration and render in sync avoids misleading callers
    /// that inspect this field.
    pub(super) fn classic() -> Self {
        Self {
            id: TemplateId::Classic,
            name: "ATS Classic",
            tier: TemplateTier::Ats,
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
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
            two_column: None,
            cover_letter: CoverLetterLayout::default(),
        }
    }

    /// Swiss Minimal — Manrope, red accent, generous whitespace, almost empty page.
    pub(super) fn swiss_minimal() -> Self {
        Self {
            id: TemplateId::SwissMinimal,
            name: "Swiss Minimal",
            tier: TemplateTier::Ats,
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
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 12.0,
            },
        }
    }

    /// Academic — Source Serif 4 throughout, forest green accent, formal block letter.
    pub(super) fn academic() -> Self {
        Self {
            id: TemplateId::Academic,
            name: "Academic",
            tier: TemplateTier::Ats,
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
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
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
    pub(super) fn atelier() -> Self {
        Self {
            id: TemplateId::Atelier,
            name: "Atelier",
            tier: TemplateTier::Design,
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
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
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
}
