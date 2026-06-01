//! Font loading + glyph subsetting for the PDF renderer.
//!
//! Bundled faces are compiled into the binary but **glyph-subset per export**, so
//! a document only embeds the glyphs it actually draws. printpdf 0.9.1 ships with
//! its own subsetting disabled at serialize time, so we subset up front in
//! [`parse_font`] (see its docs for the why).

use anyhow::Context;
use printpdf::*;
use std::collections::{BTreeMap, BTreeSet};

use crate::export::types::FontFamily;

/// Printable ASCII plus the typographic glyphs the renderers inject directly —
/// bullets, separators, en/em dashes, smart quotes, the arrow, NBSP, the common
/// currency marks, and the German umlauts (a supported export locale) — so glyph
/// subsetting can never drop a glyph the layout actually draws even when the
/// source text never contained it.
const BASELINE_GLYPHS: &str = concat!(
    " !\"#$%&'()*+,-./0123456789:;<=>?@",
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`",
    "abcdefghijklmnopqrstuvwxyz{|}~",
    "•·–—‘’“”…→€£",
    "äöüÄÖÜß",
    "\u{00A0}",
);

/// Collect the distinct codepoints a document draws, seeded with
/// [`BASELINE_GLYPHS`]. Drives glyph subsetting so every embedded font carries
/// only the glyphs actually in use.
pub fn collect_codepoints<'a>(texts: impl IntoIterator<Item = &'a str>) -> BTreeSet<char> {
    let mut set: BTreeSet<char> = BASELINE_GLYPHS.chars().collect();
    for t in texts {
        set.extend(t.chars());
    }
    set
}

/// Per-family font IDs (regular + bold + optional italic).
#[derive(Clone)]
pub struct FamilyFonts {
    pub regular: FontId,
    pub bold: FontId,
    pub italic: Option<FontId>,
}

/// All template fonts embedded at compile time regardless of which template the
/// user selects. This keeps the renderer stateless and avoids runtime font-file IO
/// at the cost of ~4.2 MB binary size. Switch to a lazy font registry keyed by
/// FontFamily if bundle size becomes a concern.
pub struct LoadedFontSet {
    pub calibri: FamilyFonts,
    pub inter: FamilyFonts,
    pub source_serif4: FamilyFonts,
    pub manrope: FamilyFonts,
    pub jetbrains_mono: FamilyFonts,
    pub playfair_display: FamilyFonts,
}

impl LoadedFontSet {
    pub fn family(&self, fam: FontFamily) -> &FamilyFonts {
        match fam {
            FontFamily::Calibri => &self.calibri,
            FontFamily::Inter => &self.inter,
            FontFamily::SourceSerif4 => &self.source_serif4,
            FontFamily::Manrope => &self.manrope,
            FontFamily::JetBrainsMono => &self.jetbrains_mono,
            FontFamily::PlayfairDisplay => &self.playfair_display,
        }
    }
}

/// Parse a bundled font and embed only the glyphs in `used` (glyph subsetting).
///
/// printpdf 0.9.1 ships with font subsetting hard-disabled at serialize time
/// (`serialize.rs`: `if false && do_subset …`), and it already prunes any face
/// that no glyph references — so without this every export embeds the *full*
/// remaining face. Calibri regular+bold alone is ~3.2 MB, which is the bulk of
/// the oversized ~3 MB résumé PDFs.
///
/// We subset the face to the document's codepoints up front via printpdf's own
/// allsorts subsetter (compiled in through the default `html` → `text_layout`
/// feature) and embed the result. Any failure (parse, subset, or re-parse) falls
/// back to embedding the full font, so the worst case is exactly today's output —
/// never a missing glyph.
fn parse_font(
    bytes: &[u8],
    doc: &mut PdfDocument,
    used: &BTreeSet<char>,
) -> anyhow::Result<FontId> {
    let mut warnings = Vec::new();
    let full = ParsedFont::from_bytes(bytes, 0, &mut warnings).context("Failed to parse font")?;

    // Map each used codepoint to its glyph id in this face, skipping any the face
    // lacks (the subset only needs the glyphs that actually resolve here).
    let glyph_ids: BTreeMap<u16, char> = used
        .iter()
        .filter_map(|&ch| full.lookup_glyph_index(ch as u32).map(|gid| (gid, ch)))
        .collect();

    if !glyph_ids.is_empty() {
        if let Ok(subset) = printpdf::subset_font(&full, &glyph_ids) {
            if !subset.bytes.is_empty() {
                let mut w = Vec::new();
                if let Some(parsed) = ParsedFont::from_bytes(&subset.bytes, 0, &mut w) {
                    return Ok(doc.add_font(&parsed));
                }
            }
        }
    }

    Ok(doc.add_font(&full))
}

/// Load all template fonts into the PDF document, glyph-subset to `used` (the
/// document's codepoints — see [`collect_codepoints`]).
/// Returns LoadedFontSet ready to use across all render functions.
pub fn load_all_fonts(
    doc: &mut PdfDocument,
    used: &BTreeSet<char>,
) -> anyhow::Result<LoadedFontSet> {
    // Calibri (existing — bundled separately)
    let cal_reg = include_bytes!("../../../fonts/calibri.ttf");
    let cal_bol = include_bytes!("../../../fonts/calibrib.ttf");

    // Inter
    let int_reg = include_bytes!("../../../fonts/inter_regular.ttf");
    let int_bol = include_bytes!("../../../fonts/inter_bold.ttf");

    // Source Serif 4
    let ss4_reg = include_bytes!("../../../fonts/source_serif4_regular.ttf");
    let ss4_bol = include_bytes!("../../../fonts/source_serif4_bold.ttf");
    let ss4_ita = include_bytes!("../../../fonts/source_serif4_italic.ttf");

    // Manrope
    let man_reg = include_bytes!("../../../fonts/manrope_regular.ttf");
    let man_bol = include_bytes!("../../../fonts/manrope_bold.ttf");

    // JetBrains Mono
    let jbm_reg = include_bytes!("../../../fonts/jetbrains_mono_regular.ttf");
    let jbm_bol = include_bytes!("../../../fonts/jetbrains_mono_bold.ttf");

    // Playfair Display
    let pfd_reg = include_bytes!("../../../fonts/playfair_display_regular.ttf");
    let pfd_bol = include_bytes!("../../../fonts/playfair_display_bold.ttf");

    Ok(LoadedFontSet {
        calibri: FamilyFonts {
            regular: parse_font(cal_reg, doc, used)?,
            bold: parse_font(cal_bol, doc, used)?,
            italic: None,
        },
        inter: FamilyFonts {
            regular: parse_font(int_reg, doc, used)?,
            bold: parse_font(int_bol, doc, used)?,
            italic: None,
        },
        source_serif4: FamilyFonts {
            regular: parse_font(ss4_reg, doc, used)?,
            bold: parse_font(ss4_bol, doc, used)?,
            italic: Some(parse_font(ss4_ita, doc, used)?),
        },
        manrope: FamilyFonts {
            regular: parse_font(man_reg, doc, used)?,
            bold: parse_font(man_bol, doc, used)?,
            italic: None,
        },
        jetbrains_mono: FamilyFonts {
            regular: parse_font(jbm_reg, doc, used)?,
            bold: parse_font(jbm_bol, doc, used)?,
            italic: None,
        },
        playfair_display: FamilyFonts {
            regular: parse_font(pfd_reg, doc, used)?,
            bold: parse_font(pfd_bol, doc, used)?,
            italic: None,
        },
    })
}

/// Resolve the (regular, bold, italic_opt) font IDs for a given family.
pub fn resolve_fonts(set: &LoadedFontSet, fam: FontFamily) -> (&FontId, &FontId, Option<&FontId>) {
    let f = set.family(fam);
    (&f.regular, &f.bold, f.italic.as_ref())
}
