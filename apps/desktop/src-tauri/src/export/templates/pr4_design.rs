//! PR4 photo / two-column design templates: Aria (untinted right sidebar) and
//! Saffron (warm tinted left sidebar).

use crate::export::types::{FontFamily, TemplateId};

use super::{
    CoverLetterLayout, ParagraphIndent, SectionStyle, Template, TemplateFonts, TemplateTier,
    TwoColumnConfig,
};

impl Template {
    /// Aria — minimalist design two-column (Ava-Amelia reference).
    ///
    /// Design: an untinted RIGHT sidebar carries the rectangular top-right photo,
    /// contact, and Skills/Languages/Certifications; the main column (left) opens
    /// with a 30pt Manrope name and carries Summary/Experience/Projects **and
    /// Education** (per the `theme::placement_for` override). Slate accent
    /// (#46505C), letter-spaced (`heading_tracking 0.06`) all-caps hairline-ruled
    /// headings, Inter body, generous whitespace. No-photo fallback is a name-only
    /// header (no monogram). Renders through the bespoke `aria.typ`. Tier: Design.
    pub(super) fn aria() -> Self {
        Self {
            id: TemplateId::Aria,
            name: "Aria",
            tier: TemplateTier::Design,
            // Slate palette on near-black ink.
            name_color: (17, 17, 17),
            section_color: (26, 26, 26),
            accent_color: (70, 80, 92), // #46505C slate
            body_color: (42, 42, 42),
            date_color: (122, 122, 122),
            emphasis_color: (70, 80, 92),
            rule_color: (214, 217, 221), // #D6D9DD hairline
            name_pt: 30.0,
            section_pt: 10.5,
            body_pt: 10.0,
            margin_in: 0.6,
            line_spacing: 1.25,
            section_spacing_before: 16.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::Manrope,
                heading_family: FontFamily::Manrope,
                body_family: FontFamily::Inter,
            },
            job_title_italic: false,
            section_small_caps: false,
            rule_thickness: 0.5,
            heading_tracking: 0.06,
            link_underline: false,
            section_above_extra: 0.0,
            two_column: Some(TwoColumnConfig {
                sidebar_width_ratio: 0.32,
                // Untinted (white) — aria.typ skips the sidebar band fill.
                sidebar_bg_color: (255, 255, 255),
            }),
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }

    /// Saffron — warm design two-column (warm reference).
    ///
    /// Design: a tinted LEFT sidebar (#F5E7DA) carries a circular photo with a
    /// 1.5pt accent ring (the differentiator vs Portrait's ringless circle),
    /// contact, and Skills/Education/Languages; the main column carries
    /// Summary/Experience/Projects **and Certifications** (per the
    /// `theme::placement_for` override). Source Serif 4 small-caps headings, Inter
    /// body, terracotta accent (#A85A3E), warm dark name. No-photo fallback is a
    /// warm accent monogram circle. Renders through the bespoke `saffron.typ`.
    /// Tier: Design.
    pub(super) fn saffron() -> Self {
        Self {
            id: TemplateId::Saffron,
            name: "Saffron",
            tier: TemplateTier::Design,
            // Terracotta accent on a warm dark ink.
            name_color: (58, 46, 40),
            section_color: (168, 90, 62), // #A85A3E terracotta
            accent_color: (168, 90, 62),
            body_color: (48, 42, 38),
            date_color: (138, 122, 110),
            emphasis_color: (168, 90, 62),
            rule_color: (226, 201, 180),
            name_pt: 24.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 0.55,
            line_spacing: 1.2,
            section_spacing_before: 12.0,
            name_centered: false,
            section_all_caps: false,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::SourceSerif4,
                heading_family: FontFamily::SourceSerif4,
                body_family: FontFamily::Inter,
            },
            job_title_italic: true,
            section_small_caps: true,
            rule_thickness: 0.5,
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
            two_column: Some(TwoColumnConfig {
                sidebar_width_ratio: 0.34,
                // Warm peach tint.
                sidebar_bg_color: (245, 231, 218),
            }),
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }
}
