//! Phase 3b-i photo templates: Portrait (circular photo top-left, sidebar)
//! and Lebenslauf (DACH DIN-style tabular CV, photo top-right).

use crate::export::types::{FontFamily, TemplateId};

use super::{
    CoverLetterLayout, ParagraphIndent, SectionStyle, Template, TemplateFonts, TemplateTier,
    TwoColumnConfig,
};

impl Template {
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
            tier: TemplateTier::Design,
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
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
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
            tier: TemplateTier::Design,
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
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
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
