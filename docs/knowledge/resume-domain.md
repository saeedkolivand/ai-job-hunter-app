# Resume domain (resume + ATS + export)

Merged knowledge for `resume-export-expert`, `pdf-docx-generator` (impl), and `job-match-expert` (ATS scoring). Canonical: [`docs/EXPORT_TEMPLATES.md`](../EXPORT_TEMPLATES.md). Source is authoritative for literals (template count, scoring weights).

## Résumé structure

`DocumentModel` (`model/document.rs`): sections → blocks → rich text. Section ordering, relationships, content hierarchy, and customization are the resume architecture. Header/contact links come from `contact_profile/` (single source of truth — don't duplicate in templates).

## Templates

`export/templates/` — the template set (count lives in `export/templates/mod.rs`; do not copy a number here). Each must be **ATS-safe** (linear, parseable, predictable rendering, sensible section naming) and may be industry-specific. Country/locale standards (US Letter vs A4, regional conventions) via `locale/` + `theme/`.

## ATS — two distinct concerns (don't conflate)

1. **ATS-safe formatting** (owner: `resume-export-expert`) — the _output_ document parses cleanly: no multi-column traps for parsers, standard section headings, embedded/text fonts, no text-in-image. Linearization + the `validate/` gate enforce this.
2. **ATS scoring / matching** (owner: `job-match-expert`) — `commands/match_resume.rs`: `keywords()` (tokenize + drop short/stopwords), `keyword_coverage()` (resume vs job keyword overlap), and a **weighted blend of semantic similarity + keyword coverage**. **Read `match_resume.rs` for the exact weights/algorithm — never trust a copied number.** Gaps → recommendations (`recommend/`). Cover letters: `cover_letter/`.

## Export contract & pipeline

- Contract: `ExportRequest`/`ExportResult` in `export/types.rs` (format, template, ATS mode, locale).
- **PDF**: `export/pdf/`, `export/pdf_renderer/`, `export/layout_pdf/` — printpdf + ttf-parser; embed fonts; pre-measure layout before render; compute pagination once (avoid overflow). Prefer **golden tests**.
- **DOCX**: `export/docx/`, `export/model_docx/`, `export/docx_renderer.rs` — docx-rs; fallback fonts; structural fidelity. Prefer **golden tests**.
- **Golden parity** — keep PDF and DOCX outputs aligned where the design requires; deterministic snapshots, reviewed on update.
- **Validate gate** — `validate/` checks ATS compliance before/at export.

## PDF glyph subsetting

`export/pdf_renderer/fonts.rs: parse_font` subsets each embedded font to rendered codepoints via `printpdf::subset_font`; falls back to full-font on failure. A size-budget guardrail test (`export/pdf/test.rs: classic_resume_pdf_is_glyph_subset_under_budget`, 800 KB limit) catches subsetting regressions. See [ADR-008](decision-records/adr-008-pdf-glyph-subsetting.md).

## Review heuristics

- HIGH: a template/layout change that breaks ATS parseability; a scoring change that violates the documented model without an ADR; an untested export error path; a header-link regression (links must come from `contact_profile/`).
- MEDIUM: missing golden/edge-case test, non-deterministic snapshot, avoidable re-shaping in the render loop (perf → `performance-profiler`).
