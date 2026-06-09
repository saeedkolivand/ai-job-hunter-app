# Resume domain (resume + ATS + export)

Last updated: 2026-06-09

Merged knowledge for `resume-export-expert`, `pdf-docx-generator` (impl), and `job-match-expert` (ATS scoring). Canonical: [`docs/EXPORT_TEMPLATES.md`](../EXPORT_TEMPLATES.md). Source is authoritative for literals (template count, scoring weights).

## Résumé structure

`DocumentModel` (`model/document.rs`): sections → blocks → rich text. Section ordering, relationships, content hierarchy, and customization are the resume architecture. Header/contact links come from `contact_profile/` (single source of truth — don't duplicate in templates).

## Templates

Nine templates. `TemplateId` in `export/types.rs`; registry/styling data in `export/templates/mod.rs`; `.typ` sources embedded at build time in `export/typst_engine/templates/`. Unknown IDs fall back to `Classic` via the custom `Deserialize` impl (serde-tolerant). Two-column set: `Atelier` + `Portrait` — gate via `theme::is_two_column`. Section→column routing: `theme::placement_for` (single source of truth). See [`docs/EXPORT_TEMPLATES.md`](../EXPORT_TEMPLATES.md) for the full table; do not copy counts here.

## ATS — two distinct concerns (don't conflate)

1. **ATS-safe formatting** (owner: `resume-export-expert`) — the _output_ document parses cleanly: no multi-column traps for parsers, standard section headings, embedded/text fonts, no text-in-image. Linearization + the `validate/` gate enforce this.
2. **ATS scoring / matching** (owner: `job-match-expert`) — `documents/keywords.rs` (shared keyword module) + `commands/match_resume.rs`. Split pipeline:
   - `keywords_normalized()` — tokenizes, lowercases, applies synonym-normalization (e.g., `js`→`javascript`, `k8s`→`kubernetes`, `c++`→`cpp`), filters: drops strings ≤3 chars unless in `SHORT_TECH_TERMS` allowlist (go, sql, aws, gcp, css, git, api, vue, ios, tdd, bdd, ci, cd, ml, ai, ui, ux, qa, rx, etl, sap, erp, crm, k8s, r, cpp), drops stopwords. **No stemming.** Synonym lookup runs on raw tokens (before trimming) so `c-plus-plus` → `cpp` survives. Cached per-document in `keywords_json` column (migration 4).
   - `apply_stemmer()` — Snowball stemming per language detected at match time (German/French/Spanish/Italian/Portuguese/Dutch via whatlang; fallback English). Stemming skipped for `SHORT_TECH_TERMS` to prevent corruption (aws → aw).
   - `keyword_coverage()` returns resume vs job keyword overlap %; a **weighted blend of semantic similarity + keyword coverage** (read source for exact ratio — never trust a copied number). Corrupt-cache fallback: `from_str().unwrap_or_default()` returns empty set (domain-visible ATS-score drop → zero). Auto-scoring gated by sequential FIFO scheduler (CONCURRENCY=1). Gaps → recommendations (`recommend/`). Cover letters: `cover_letter/`.

## Export contract & pipeline

- Contract: `ExportRequest`/`ExportResult` in `export/types.rs` (format, template, ATS mode, locale, optional `contact: ContactProfile`).
- **PDF**: `export/pdf/mod.rs` dispatches to `export/typst_engine/` (Typst adapter — sole PDF engine). Templates are `.typ` files embedded via `include_bytes!`. Only `engine.rs` + `render.rs` import the `typst`/`typst_pdf` crates (isolation boundary). Round-trip tests + validate gate in `export/typst_engine/test.rs`. Prefer **golden tests**.
- **DOCX**: `export/docx/`, `export/model_docx.rs` — [docx-rs][docx-rs]; fallback fonts; structural fidelity. Prefer **golden tests**.
- **Golden parity** — keep PDF and DOCX outputs aligned where the design requires; deterministic snapshots, reviewed on update.
- **Validate gate** — `validate/` checks ATS compliance at export; content-based URL checks; `page_annot_dicts` reads Typst inline-dict `/Annots`.

## Cover-letter PDF

`render_letter_pdf` in `typst_engine/engine.rs`. Market conventions (date placement, recipient block, sign-off) come from `locale/letter.rs` (`LetterMarketConventions`). **Cover letters inherit the resume template's visual style** (accent/fonts/sizes) via `letter_style_from_template` in `typst_engine/letter.rs`. `parse_cover_letter` produces a `LetterModel` serialised to JSON — no user content concatenated into Typst markup.

**Template previews** (for the AI-Generate template picker): offline test `generate_cover_template_previews` in `typst_engine/test.rs` renders each of the 9 resume templates' cover-letter style to SVG (per-template; vector, no raster). Owned by: `export/typst_engine/`. Consumed by: `samples/cover-template-previews.ts` Vite glob → `COVER_TEMPLATE_PREVIEWS` → renderer UI. See [`docs/EXPORT_TEMPLATES.md` § Cover-letter template previews](../EXPORT_TEMPLATES.md#cover-letter-template-previews-ai-generate-ui).

## Candidate photo

`ContactProfile.photo` — **`data:` URI only** (file paths rejected at `typst_engine/photo.rs: resolve_photo`). Client pipeline: `apps/tauri/src/renderer/lib/photo.ts` (crop/scale/EXIF-strip → JPEG data URL). Used by `Portrait` + `Lebenslauf` templates.

## CJK deferred

CJK (zh/ja/ko) renders as tofu — no CJK font bundle yet. `isCjkLanguage` in `packages/shared/src/language-detection.ts` gates the `aiGenerate.cjkUnsupported` UI notice.

## Review heuristics

- HIGH: a template/layout change that breaks ATS parseability; a scoring change that violates the documented model without an ADR; an untested export error path; a header-link regression (links must come from `contact_profile/`); a photo path that accepts file URIs.
- MEDIUM: missing golden/edge-case test, non-deterministic snapshot, avoidable re-shaping in the render loop (perf → `performance-profiler`).

[docx-rs]: https://github.com/bokuweb/docx-rs
