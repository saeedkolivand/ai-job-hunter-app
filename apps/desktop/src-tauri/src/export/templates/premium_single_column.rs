//! Phase 3a premium single-column templates: Meridian (header-forward band)
//! and Throughline (timeline spine).

use crate::export::types::{FontFamily, TemplateId};

use super::{
    CoverLetterLayout, ParagraphIndent, SectionStyle, Template, TemplateFonts, TemplateTier,
};

impl Template {
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
            tier: TemplateTier::Ats,
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
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
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
            tier: TemplateTier::Ats,
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
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }
}
