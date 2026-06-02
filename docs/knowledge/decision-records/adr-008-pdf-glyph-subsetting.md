# ADR-008: PDF glyph subsetting at export time via `parse_font`

Last updated: 2026-06-02

**Status:** Superseded by the Typst migration (`feat/typst-premium-resume-templates`).
Typst handles font subsetting internally when producing PDF bytes; the explicit
`parse_font` / `subset_font` path described below no longer exists.

---

## Original decision (historical reference only)

The former printpdf-based PDF renderer bundled six font families as TTFs and
embedded them via `export/pdf_renderer/fonts.rs: parse_font`, which subsetted each
font to rendered codepoints using printpdf's `subset_font`. A size-budget test
(`export/pdf/test.rs: classic_resume_pdf_is_glyph_subset_under_budget`, 800 KB
limit) enforced the subsetting contract.

## Migration outcome

- `export/pdf_renderer/` and `export/layout_pdf.rs` deleted.
- `printpdf` and `ttf-parser` removed from `Cargo.toml`.
- Fonts are now vendored as `Carlito` (Calibri-metric-compatible, OFL) and
  `Noto Sans` (Latin + Cyrillic, OFL), loaded into the Typst world via
  `include_bytes!` in `export/typst_engine/world.rs`.
- Typst's PDF serialiser performs its own subsetting — no application-level
  `subset_font` call is required.
- The size-budget guardrail test was re-added in `export/typst_engine/test.rs`
  for the new engine path.
