# ADR-008: PDF glyph subsetting at export time via `parse_font`

**Status:** Accepted

## Context

The PDF renderer bundles multiple font families (Calibri, Inter, SourceSerif4, Manrope, JetBrains Mono, Playfair Display) compiled into the binary. Full-font embedding produces exports well over 3 MB. `printpdf` 0.9.1 ships with its own font-subsetting hard-disabled at serialize time (`if false && do_subset …` in `serialize.rs`), so without explicit subsetting every export embeds every glyph of every face.

## Decision

`apps/tauri/src-tauri/src/export/pdf_renderer/fonts.rs: parse_font` subsets each font to the codepoints actually rendered in the document before embedding, using `printpdf::subset_font`. `collect_codepoints` gathers the used character set (plus a `BASELINE_GLYPHS` safety set) from the document content before any font is loaded. On subset failure the function falls back to embedding the full font so the export never hard-errors. A guardrail test (`export/pdf/test.rs: classic_resume_pdf_is_glyph_subset_under_budget`) asserts the output of the default template stays under 800 KB, catching a full-font regression by an order of magnitude.

## Consequences

- Typical PDF export size is a fraction of the full-font baseline.
- The 800 KB budget is generous enough to be stable across content changes but detects a subsetting regression immediately.
- `parse_font` is the only font-loading entry point; all templates call `load_all_fonts`, which calls `parse_font` for every face.
- If `printpdf::subset_font` is upgraded or replaced, the guardrail test validates the new path without code-review action.
