# Resume domain (resume + ATS + export)

Last updated: 2026-07-16

Merged knowledge for `resume-export-expert`, `pdf-docx-generator` (impl), and `job-match-expert` (ATS scoring). Canonical: [`docs/EXPORT_TEMPLATES.md`](../EXPORT_TEMPLATES.md). Source is authoritative for literals (template count, scoring weights).

## Résumé structure

`DocumentModel` (`model/document.rs`): sections → blocks → rich text. Section ordering, relationships, content hierarchy, and customization are the resume architecture. Header/contact links come from `contact_profile/` (single source of truth — don't duplicate in templates).

## Templates

Two **tiers** — `TemplateTier { Ats, Design }` (`export/templates/mod.rs`), metadata only: drives the gallery grouping (ATS-Safe / Design) and **which templates surface the ATS-mode toggle** (design-tier, incl. the photo single-column `Lebenslauf`, replacing the old two-column gate). Frontend mirror: `isDesignTier` in `renderer/lib/generate/templates/templates.ts`. `TemplateId` in `export/types.rs`; registry/styling in `export/templates/mod.rs`; `.typ` sources embedded via `include_str!` in `export/typst_engine/templates/` (ATS templates route through the parametric `single_column.typ`; photo/two-column ones have bespoke `.typ`). Unknown / removed IDs (including a saved `"modern"`) fall back to `Classic` via the custom `Deserialize` impl (serde-tolerant). Two-column set gated by `theme::is_two_column`. Section→column routing is `theme::placement_for(template_id, section)` — **template-aware** (per-template overrides pull a section into the main column). Per-export **Document accent** recolors the chosen template's accent role, validated by one shared `normalise_accent` (no PDF/DOCX drift); it never reads `ThemePrefs` — [ADR 0007](../adr/0007-document-color-is-a-knob-not-a-template.md). See [`docs/EXPORT_TEMPLATES.md`](../EXPORT_TEMPLATES.md) for the full roster + tiers; source is authoritative for the count/literals.

## ATS — two distinct concerns (don't conflate)

1. **ATS-safe formatting** (owner: `resume-export-expert`) — the _output_ document parses cleanly: no multi-column traps for parsers, standard section headings, embedded/text fonts, no text-in-image. Linearization + the `validate/` gate enforce this.
2. **ATS scoring / matching** (owner: `job-match-expert`) — `documents/keywords.rs` (shared keyword module) + `commands/match_resume.rs`. Split pipeline:
   - `keywords_normalized()` — tokenizes, lowercases, applies synonym-normalization (e.g., `js`→`javascript`, `k8s`→`kubernetes`, `c++`→`cpp`), filters: drops strings ≤3 chars unless in `SHORT_TECH_TERMS` allowlist (go, sql, aws, gcp, css, git, api, vue, ios, tdd, bdd, ci, cd, ml, ai, ui, ux, qa, rx, etl, sap, erp, crm, k8s, r, cpp), drops stopwords. **No stemming.** Synonym lookup runs on raw tokens (before trimming) so `c-plus-plus` → `cpp` survives. Cached per-document in `keywords_json` column (migration 4).
   - `apply_stemmer()` — Snowball stemming per language detected at match time (German/French/Spanish/Italian/Portuguese/Dutch via whatlang; fallback English). Stemming skipped for `SHORT_TECH_TERMS` to prevent corruption (aws → aw).
   - `keyword_coverage()` returns resume vs job keyword overlap %; a **weighted blend of semantic similarity + keyword coverage** (read source for exact ratio — never trust a copied number). Corrupt/absent cache → `parse_resume_keywords` returns None → live extraction fallback from resume.text (never an empty set / zero score). Scoring is on-demand per opened job via MatchScoresProvider + useJobMatchScore (React Query, 10-min cache) — the old batch/FIFO auto-scorer is gone. Gaps → recommendations (`recommend/`). Cover letters: `cover_letter/`.

## Export contract & pipeline

- Contract: `ExportRequest`/`ExportResult` in `export/types.rs` (format, template, ATS mode, locale, optional `contact: ContactProfile`).
- **PDF**: `export/pdf/mod.rs` dispatches to `export/typst_engine/` (Typst adapter — sole PDF engine). Templates are `.typ` files embedded via `include_str!`. Only `engine.rs` + `world.rs` import the `typst`/`typst_pdf` crates (isolation boundary). Round-trip tests + validate gate in `export/typst_engine/test.rs`. Prefer **golden tests**.
- **DOCX**: `export/docx/`, `export/model_docx.rs` — [docx-rs][docx-rs]; fallback fonts; structural fidelity. Prefer **golden tests**.
- **Golden parity** — keep PDF and DOCX outputs aligned where the design requires; deterministic snapshots, reviewed on update.
- **Validate gate** — `validate/` checks ATS compliance at export; content-based URL checks; `page_annot_dicts` reads Typst inline-dict `/Annots`.

## Cover-letter PDF

`render_letter_pdf` in `typst_engine/engine.rs`. Market conventions (date placement, recipient block, sign-off) come from `locale/letter.rs` (`LetterConventions`). **Cover letters inherit the resume template's visual style** (accent/fonts/sizes) via `style_from_template` (imported as `letter_style_from_template`, returns `LetterStyle`) in `typst_engine/letter.rs`. `parse_cover_letter` produces a `LetterModel` serialised to JSON — no user content concatenated into Typst markup.

**Letter layouts** (`LetterLayout { Classic, Refined, Banded }`, wire `letterLayoutId` in `export/types.rs`) select the letter **arrangement** — orthogonal to the résumé template. `letter_source` dispatches to `letter.typ` / `letter_refined.typ` / `letter_banded.typ`. Layout owns composition; palette/fonts inherit via `LetterStyle`; market conventions (`data.opts`) own semantics — **layouts gate structural elements on `data.opts`, never on the layout id**. DOCX approximates (Banded's angled band → flat accent-tinted shading; PDF small-caps → uppercase). Caveat: bundled Source Serif 4 lacks `smcp`, so PDF small-caps are visually inert pending a font swap. See [`docs/EXPORT_TEMPLATES.md` § Letter layouts](../EXPORT_TEMPLATES.md#letter-layouts-classic--refined--banded).

**Template previews** (for the AI-Generate template picker): offline test `generate_cover_template_previews` in `typst_engine/test.rs` renders every résumé template's cover-letter style to SVG (per-template; vector, no raster). Owned by: `export/typst_engine/`. Consumed by: `samples/cover-template-previews.ts` Vite glob → `COVER_TEMPLATE_PREVIEWS` → renderer UI. Preview assets for the new templates + the showcase banner await regeneration (the `#[ignore]` generators can't run on the dev host). See [`docs/EXPORT_TEMPLATES.md` § Cover-letter template previews](../EXPORT_TEMPLATES.md#cover-letter-template-previews-ai-generate-ui).

## Candidate photo

`ContactProfile.photo` — **`data:` URI only** (file paths rejected at `typst_engine/photo.rs: resolve_photo`). Client pipeline: `apps/desktop/src/renderer/lib/photo.ts` (crop/scale/EXIF-strip → JPEG data URL). Used by the photo templates (`Portrait`, `Lebenslauf`, `Aria`, `Saffron`); design-tier templates drop the photo under ATS mode.

## CJK deferred

CJK (zh/ja/ko) renders as tofu — no CJK font bundle yet. `isCjkLanguage` in `packages/shared/src/language-detection.ts` gates the `aiGenerate.cjkUnsupported` UI notice.

## Accessibility

PDF exports carry a **baseline tag tree** (typst-pdf 0.15 tags by default), enabling
screen-reader navigation and text extraction. PDF/UA-1 validation (certified accessible
format) is a future goal; currently blocked on four templates with link-bearing contact
blocks in page backgrounds — see [`docs/EXPORT_TEMPLATES.md` § Accessibility](../EXPORT_TEMPLATES.md#accessibility--tagged-pdf).

## Review heuristics

- HIGH: a template/layout change that breaks ATS parseability; a scoring change that violates the documented model without an ADR; an untested export error path; a header-link regression (links must come from `contact_profile/`); a photo path that accepts file URIs.
- MEDIUM: missing golden/edge-case test, non-deterministic snapshot, avoidable re-shaping in the render loop (perf → `performance-profiler`).

[docx-rs]: https://github.com/bokuweb/docx-rs
