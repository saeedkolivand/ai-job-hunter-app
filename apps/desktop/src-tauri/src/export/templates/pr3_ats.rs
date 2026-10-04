//! PR3 single-column ATS templates: Cadence, Cologne Navy, Regent.

use crate::export::types::{FontFamily, TemplateId};

use super::{
    CoverLetterLayout, ParagraphIndent, SectionStyle, Template, TemplateFonts, TemplateTier,
};

impl Template {
    /// Cadence — Claude-PDF-style ATS single-column.
    ///
    /// Design: Inter throughout, large 28pt name, blue-grey accent (#4A6785),
    /// letter-spaced (`heading_tracking 0.08`) all-caps ruled section headings,
    /// underlined hyperlinks (`link_underline true`). Renders through the
    /// parametric `single_column.typ` — no bespoke `.typ`. Tier: ATS.
    pub(super) fn cadence() -> Self {
        Self {
            id: TemplateId::Cadence,
            name: "Cadence",
            tier: TemplateTier::Ats,
            // Near-black ink with a restrained blue-grey accent.
            name_color: (26, 26, 26),
            section_color: (26, 26, 26),
            accent_color: (74, 103, 133), // #4A6785 blue-grey
            body_color: (43, 43, 43),
            date_color: (107, 107, 107),
            emphasis_color: (74, 103, 133),
            rule_color: (74, 103, 133),
            name_pt: 28.0,
            section_pt: 10.5,
            body_pt: 10.0,
            margin_in: 0.8,
            line_spacing: 1.15,
            section_spacing_before: 12.0,
            name_centered: false,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::Inter,
                heading_family: FontFamily::Inter,
                body_family: FontFamily::Inter,
            },
            job_title_italic: false,
            section_small_caps: false,
            rule_thickness: 0.75,
            heading_tracking: 0.08,
            link_underline: true,
            section_above_extra: 0.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }

    /// Cologne Navy — centred tracked-caps navy header, rule-underlined
    /// uppercase section headings, blue company names, right-aligned italic
    /// dates. Renders through the bespoke `cologne_navy.typ`.
    ///
    /// Carlito throughout — `FontFamily::Calibri` already RESOLVES to the
    /// bundled Carlito faces in Typst (`letter.rs`'s `font_name`), which is
    /// exactly the metric-compatible substitution this design calls for, so no
    /// new font variant or bundling is needed.
    /// Navy `#1F3864` for the name/headings/rule, a lighter blue `#1F5C99` as
    /// the accent for company and institution names. Single-column and
    /// parser-safe, so it is an ATS-tier template.
    pub(super) fn cologne_navy() -> Self {
        Self {
            id: TemplateId::CologneNavy,
            name: "Cologne Navy",
            tier: TemplateTier::Ats,
            name_color: (31, 56, 100),    // #1F3864 navy
            section_color: (31, 56, 100), // #1F3864 navy
            accent_color: (31, 92, 153),  // #1F5C99 link blue
            body_color: (26, 26, 26),     // #1A1A1A ink
            date_color: (74, 74, 74),     // #4A4A4A
            emphasis_color: (31, 92, 153),
            rule_color: (31, 56, 100),
            // Ratios off body_pt, per the design: name 2.08, heading 0.95.
            name_pt: 20.8,
            section_pt: 9.5,
            body_pt: 10.0,
            margin_in: 0.55, // 14mm side margins
            line_spacing: 1.15,
            section_spacing_before: 12.0,
            name_centered: true,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::Calibri,
                heading_family: FontFamily::Calibri,
                body_family: FontFamily::Calibri,
            },
            job_title_italic: false,
            section_small_caps: false,
            rule_thickness: 0.9, // the design's 0.9pt navy rule
            // 0.10, NOT the brief's 0.18: wider tracking makes the PDF text
            // extract letter-by-letter ("S U M M A R Y") and an ATS cannot read
            // it. See the DEVIATION note in `cologne_navy.typ`. The template
            // reads this field, so the two cannot drift.
            heading_tracking: 0.10,
            link_underline: false,
            section_above_extra: 0.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }

    /// Regent — executive serif ATS single-column.
    ///
    /// Design: Source Serif 4 throughout, 26pt name, burgundy (#6E1E2B) small-caps
    /// section headings with a rose rule (#C9A9AE), lightly tracked
    /// (`heading_tracking 0.04`), italic job titles, first-line-indent cover
    /// letter (executive serif pairing, like Academic). Renders through the
    /// parametric `single_column.typ` — no bespoke `.typ`. Tier: ATS.
    pub(super) fn regent() -> Self {
        Self {
            id: TemplateId::Regent,
            name: "Regent",
            tier: TemplateTier::Ats,
            // Charcoal ink, burgundy accent, muted rose rule.
            name_color: (42, 42, 46),
            section_color: (110, 30, 43), // #6E1E2B burgundy
            accent_color: (110, 30, 43),
            body_color: (38, 38, 42),
            date_color: (122, 106, 110),
            emphasis_color: (110, 30, 43),
            rule_color: (201, 169, 174), // #C9A9AE rose
            name_pt: 26.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 0.9,
            line_spacing: 1.2,
            section_spacing_before: 14.0,
            name_centered: false,
            section_all_caps: false,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts {
                name_family: FontFamily::SourceSerif4,
                heading_family: FontFamily::SourceSerif4,
                body_family: FontFamily::SourceSerif4,
            },
            job_title_italic: true,
            section_small_caps: true,
            rule_thickness: 0.5,
            heading_tracking: 0.04,
            link_underline: false,
            section_above_extra: 0.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::FirstLine,
                paragraph_spacing_pt: 0.0,
            },
        }
    }
}
