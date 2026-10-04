//! Phase 8 Track B: classic LaTeX community-template designs — Jake, Awesome,
//! Deedy.

use crate::export::types::{FontFamily, TemplateId};

use super::{
    CoverLetterLayout, ParagraphIndent, SectionStyle, Template, TemplateFonts, TemplateTier,
};

impl Template {
    /// Jake — after "Jake's Resume": ultra-minimal single column, centred name,
    /// thin ruled section headings, compact entry lines (title left / date
    /// right-aligned). Renders through the parametric `single_column.typ` — no
    /// bespoke `.typ`, no new knobs (every field it needs already exists). Tier:
    /// ATS.
    ///
    /// NOTE: `single_column.typ`'s PDF page margin is a locked 25.4mm/1in for
    /// every parametric template — `margin_in` only drives the DOCX margin, the
    /// same asymmetry Classic/SwissMinimal/Academic/Cadence/Regent already have.
    /// Jake's "compact" character therefore comes from font size and rule
    /// thickness, not a PDF-only margin override.
    pub(super) fn jake() -> Self {
        Self {
            id: TemplateId::Jake,
            name: "Jake",
            tier: TemplateTier::Ats,
            // Monochrome ink — maximum ATS/print safety, matching the reference's
            // plain black-and-white aesthetic.
            name_color: (17, 17, 17),
            section_color: (17, 17, 17),
            accent_color: (17, 17, 17),
            body_color: (34, 34, 34),
            date_color: (85, 85, 85),
            emphasis_color: (17, 17, 17),
            rule_color: (170, 170, 170),
            name_pt: 24.0,
            section_pt: 11.0,
            body_pt: 10.0,
            margin_in: 0.6,
            line_spacing: 1.0,
            section_spacing_before: 10.0,
            name_centered: true,
            section_all_caps: true,
            section_style: SectionStyle::RuledBottom,
            fonts: TemplateFonts::default(),
            job_title_italic: true,
            section_small_caps: false,
            rule_thickness: 0.4,
            heading_tracking: 0.0,
            link_underline: false,
            section_above_extra: 0.0,
            two_column: None,
            cover_letter: CoverLetterLayout::default(),
        }
    }

    /// Awesome — after Awesome-CV: a thin accent-tinted header band (name +
    /// contact line) and accent-bar section-title markers, single column body.
    /// Renders through the bespoke `awesome.typ`. Design tier so the ATS toggle
    /// surfaces — `is-ats` drops the band to a plain header and the bar to a
    /// plain heading, the linearization the tier advertises (the layout was
    /// already single-column and parser-safe either way).
    ///
    /// Crimson accent (#C41E3A) — distinct from Swiss Minimal's brighter coral
    /// red and Regent's burgundy.
    ///
    /// `name_color` is near-black, NOT the white the PDF band text renders in:
    /// `awesome.typ` hardcodes white for its own band text (mirroring
    /// `meridian.typ`'s `c-band-text`), but the generic DOCX renderer
    /// (`model_docx::add_header`) reads this registry field literally with no
    /// colored band behind it — a white `name_color` here would print invisible
    /// white-on-white DOCX text.
    pub(super) fn awesome() -> Self {
        Self {
            id: TemplateId::Awesome,
            name: "Awesome",
            tier: TemplateTier::Design,
            name_color: (26, 26, 26),
            section_color: (26, 26, 26),
            accent_color: (196, 30, 58), // #C41E3A crimson
            body_color: (34, 34, 34),
            date_color: (110, 110, 110),
            emphasis_color: (196, 30, 58),
            rule_color: (196, 30, 58),
            name_pt: 24.0,
            section_pt: 11.0,
            body_pt: 10.5,
            margin_in: 0.7,
            line_spacing: 1.15,
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

    /// Deedy — after the modern single-column Deedy revision: a bold name block
    /// with an accent-colored surname, generous section spacing, subtle grey
    /// meta-line under entries. Renders through the bespoke `deedy.typ`. Design
    /// tier so the ATS toggle surfaces — `is-ats` renders the whole name in one
    /// color (the surname split is cosmetic only; text extraction is unaffected
    /// either way).
    ///
    /// Cobalt accent (#1E4FB3) — distinct from Cologne Navy's muted link-blue
    /// and Atelier's slate-indigo.
    pub(super) fn deedy() -> Self {
        Self {
            id: TemplateId::Deedy,
            name: "Deedy",
            tier: TemplateTier::Design,
            name_color: (26, 26, 26),
            section_color: (26, 26, 26),
            accent_color: (30, 79, 179), // #1E4FB3 cobalt
            body_color: (34, 34, 34),
            date_color: (120, 120, 120),
            emphasis_color: (30, 79, 179),
            rule_color: (200, 200, 200),
            name_pt: 27.0,
            section_pt: 11.5,
            body_pt: 10.5,
            margin_in: 0.75,
            line_spacing: 1.2,
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
            heading_tracking: 0.0,
            link_underline: false,
            // The reference's "generous section spacing" trait, declared here
            // rather than as a local constant in `deedy.typ` — `_scale.typ`'s
            // rhythm is locked and no template may fork it in its own file.
            section_above_extra: 8.0,
            two_column: None,
            cover_letter: CoverLetterLayout {
                paragraph_indent: ParagraphIndent::BlockNoIndent,
                paragraph_spacing_pt: 8.0,
            },
        }
    }
}
