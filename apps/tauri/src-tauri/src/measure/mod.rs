//! Text measurement — the single source of truth for glyph advances and line
//! heights, computed from the actual bundled TrueType fonts via `ttf-parser`.
//!
//! This is the principled replacement for the `pt_to_mm(font_size) * 0.52`
//! char-width estimate the PDF renderer uses today (for link rects and name
//! centering). Phase 1 introduces it with no consumers yet — the PDF/DOCX
//! layout paths move onto it in Phase 2+.
#![allow(dead_code)]

use crate::export::types::FontFamily;

/// Points → millimetres (1 pt = 1/72 in = 25.4/72 mm).
const PT_TO_MM: f32 = 25.4 / 72.0;

/// Measures rendered text in millimetres for a given font + size.
///
/// `bold` selects the bold face (which has different metrics than the regular
/// face — important for centering bold names and sizing bold link labels).
pub trait MeasureText {
    /// Horizontal advance width of `text` in mm at `size_pt`.
    fn advance_mm(&self, text: &str, font: FontFamily, bold: bool, size_pt: f32) -> f32;

    /// Single-line height in mm at `size_pt` (ascender − descender + line gap).
    fn line_height_mm(&self, font: FontFamily, bold: bool, size_pt: f32) -> f32;
}

/// `MeasureText` backed by the `include_bytes!`-bundled TTFs (the same faces the
/// PDF renderer embeds). Stateless: faces are parsed on demand (cheap — just
/// table-offset reads), so there's nothing to cache or keep `Sync`.
pub struct FontMetrics;

impl FontMetrics {
    /// Raw TTF bytes for a family + weight. Mirrors the PDF renderer's font set;
    /// families without a dedicated bold face fall back to the regular bytes.
    fn bytes(font: FontFamily, bold: bool) -> &'static [u8] {
        match (font, bold) {
            (FontFamily::Calibri, false) => include_bytes!("../../fonts/calibri.ttf"),
            (FontFamily::Calibri, true) => include_bytes!("../../fonts/calibrib.ttf"),
            (FontFamily::Inter, false) => include_bytes!("../../fonts/inter_regular.ttf"),
            (FontFamily::Inter, true) => include_bytes!("../../fonts/inter_bold.ttf"),
            (FontFamily::SourceSerif4, false) => {
                include_bytes!("../../fonts/source_serif4_regular.ttf")
            }
            (FontFamily::SourceSerif4, true) => {
                include_bytes!("../../fonts/source_serif4_bold.ttf")
            }
            (FontFamily::Manrope, false) => include_bytes!("../../fonts/manrope_regular.ttf"),
            (FontFamily::Manrope, true) => include_bytes!("../../fonts/manrope_bold.ttf"),
            (FontFamily::JetBrainsMono, false) => {
                include_bytes!("../../fonts/jetbrains_mono_regular.ttf")
            }
            (FontFamily::JetBrainsMono, true) => {
                include_bytes!("../../fonts/jetbrains_mono_bold.ttf")
            }
            (FontFamily::PlayfairDisplay, false) => {
                include_bytes!("../../fonts/playfair_display_regular.ttf")
            }
            (FontFamily::PlayfairDisplay, true) => {
                include_bytes!("../../fonts/playfair_display_bold.ttf")
            }
        }
    }

    fn face(font: FontFamily, bold: bool) -> Option<ttf_parser::Face<'static>> {
        ttf_parser::Face::parse(Self::bytes(font, bold), 0).ok()
    }
}

impl MeasureText for FontMetrics {
    fn advance_mm(&self, text: &str, font: FontFamily, bold: bool, size_pt: f32) -> f32 {
        let Some(face) = Self::face(font, bold) else {
            // Parsing should never fail for a bundled font; degrade to the legacy
            // estimate rather than panicking in a render path.
            return size_pt * PT_TO_MM * 0.52 * text.chars().count() as f32;
        };
        let upm = face.units_per_em() as f32;
        if upm <= 0.0 {
            return 0.0;
        }
        // Sum glyph horizontal advances; unmapped chars fall back to a half-em.
        let units: f32 = text
            .chars()
            .map(|ch| {
                face.glyph_index(ch)
                    .and_then(|g| face.glyph_hor_advance(g))
                    .map(|a| a as f32)
                    .unwrap_or(upm * 0.5)
            })
            .sum();
        units / upm * size_pt * PT_TO_MM
    }

    fn line_height_mm(&self, font: FontFamily, bold: bool, size_pt: f32) -> f32 {
        let Some(face) = Self::face(font, bold) else {
            return size_pt * PT_TO_MM * 1.2;
        };
        let upm = face.units_per_em() as f32;
        if upm <= 0.0 {
            return size_pt * PT_TO_MM * 1.2;
        }
        let span = face.ascender() as f32 - face.descender() as f32 + face.line_gap() as f32;
        span / upm * size_pt * PT_TO_MM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [FontFamily; 6] = [
        FontFamily::Calibri,
        FontFamily::Inter,
        FontFamily::SourceSerif4,
        FontFamily::Manrope,
        FontFamily::JetBrainsMono,
        FontFamily::PlayfairDisplay,
    ];

    #[test]
    fn every_bundled_face_parses_and_measures() {
        let m = FontMetrics;
        for f in ALL {
            for bold in [false, true] {
                assert!(
                    m.advance_mm("Resume", f, bold, 11.0) > 0.0,
                    "advance should be positive for {f:?} bold={bold}"
                );
                assert!(
                    m.line_height_mm(f, bold, 11.0) > 0.0,
                    "line height should be positive for {f:?} bold={bold}"
                );
            }
        }
    }

    #[test]
    fn advance_scales_with_length_and_size() {
        let m = FontMetrics;
        let one = m.advance_mm("M", FontFamily::Inter, false, 11.0);
        let many = m.advance_mm("MMMM", FontFamily::Inter, false, 11.0);
        assert!(many > one * 3.0, "4×M should be ~4× a single M");

        let small = m.advance_mm("Hello", FontFamily::Inter, false, 10.0);
        let big = m.advance_mm("Hello", FontFamily::Inter, false, 20.0);
        assert!(
            (big - small * 2.0).abs() < 0.01,
            "advance is linear in size"
        );
    }

    #[test]
    fn proportional_font_widths_differ_by_glyph() {
        let m = FontMetrics;
        // In a proportional font, wide glyphs advance more than narrow ones.
        let wide = m.advance_mm("WWWW", FontFamily::Inter, false, 12.0);
        let narrow = m.advance_mm("iiii", FontFamily::Inter, false, 12.0);
        assert!(
            wide > narrow,
            "W should be wider than i in a proportional face"
        );
    }

    #[test]
    fn monospace_font_widths_are_uniform() {
        let m = FontMetrics;
        let wide = m.advance_mm("WWWW", FontFamily::JetBrainsMono, false, 12.0);
        let narrow = m.advance_mm("iiii", FontFamily::JetBrainsMono, false, 12.0);
        assert!(
            (wide - narrow).abs() < 0.01,
            "mono advances should be uniform"
        );
    }

    #[test]
    fn empty_string_has_zero_advance() {
        assert_eq!(
            FontMetrics.advance_mm("", FontFamily::Calibri, false, 11.0),
            0.0
        );
    }

    #[test]
    fn bundled_faces_contain_en_and_em_dash_glyphs() {
        // The export pipeline now PRESERVES U+2013 / U+2014 instead of collapsing
        // them to '-', so every embedded face must actually carry both glyphs or a
        // sentence-break dash would render as a missing-glyph box.
        for f in ALL {
            for bold in [false, true] {
                let face = FontMetrics::face(f, bold)
                    .unwrap_or_else(|| panic!("face must parse for {f:?} bold={bold}"));
                assert!(
                    face.glyph_index('\u{2013}').is_some(),
                    "{f:?} bold={bold} is missing the en-dash (U+2013) glyph"
                );
                assert!(
                    face.glyph_index('\u{2014}').is_some(),
                    "{f:?} bold={bold} is missing the em-dash (U+2014) glyph"
                );
            }
        }
    }
}
