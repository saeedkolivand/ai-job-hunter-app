# Resume domain (resume + ATS + export)

Last updated: 2026-08-09

Merged knowledge for `resume-export-expert`, `pdf-docx-generator` (impl), and `job-match-expert` (ATS scoring). Canonical: [`docs/EXPORT_TEMPLATES.md`](../EXPORT_TEMPLATES.md). Source is authoritative for literals (template count, scoring weights).

## Résumé structure

`DocumentModel` (`model/document.rs`): sections → blocks → rich text. Section ordering, relationships, content hierarchy, and customization are the resume architecture. **Header contact line is editor-owned at export time** ([ADR 0021](../adr/0021-editor-owns-resume-header.md)) — two distinct moments, don't conflate them; read source for the exact rules, this is a pointer not a spec. **Generation** (`generateResume` AND `synthesizeResume` → `seedHeaderFromProfile` in `apps/desktop/src/renderer/lib/generate/generation/generation.ts`) seeds the profile's name + contact line into the model-written text, so re-generating a document discards header edits by design; the function's own doc comments carry the replace-vs-insert invariants (never delete a line). **Export** (`ContactProfile::apply_to_header` in `apps/desktop/src-tauri/src/contact_profile/mod.rs`, `meta.candidate_name` in `export/pdf/mod.rs` / `export/model_docx.rs`) fills from the profile only when the parsed header is blank — whatever the text says wins for PDF/DOCX/TXT. Export validation (`validate/mod.rs`'s `pdf_render_issues`) gates on which side is the header's source of truth; a job-board/ATS host in the header band always warns, independent of whether a profile is present.

## Page target (≠ page size)

**Page target** = the customary maximum _length_ of a résumé in a market, in pages — `LocaleProfile::max_pages` (`locale/mod.rs`, per-market values + their rationale on the field's doc comment). Do not conflate it with `page_size`, the physical sheet, which sits on the same struct. Hiring convention, not a published standard. **Advisory only** — nothing blocks a longer export.

Consumed by the trim panel (`features/ai-generate/components/TrimPanel`): when the rendered preview exceeds the target, `resume_trim_suggestions` ranks the résumé's `LineKind::Bullet` lines weakest-first by how much of _this_ posting's vocabulary each carries. The ranking core is `documents/evidence.rs` → `rank_bullets` (which ranks EvidenceBullet via `score`); match_resume.rs wraps it via a `From<EvidenceBullet>` shim for wire compatibility. Embedding-free, zero model calls. Read-only — it never edits the document.

**Both surfaces that intersect a résumé against a posting must route through `keywords::languages_align`** — `score_one` and rank_bullets alike. It decides whether both sides get stemmed or both stay normalized-only; one side stemmed alone mangles language-neutral tech tokens, and two surfaces disagreeing on it makes the panel contradict the match score for the same pair (`cross_language_pair_ranks_symmetrically_like_score_one`). The renderer skips the query below a floor (`SHORTEST_OVERFLOW` in `TrimPanel`), sound only while no market target sits under it — pinned by `no_market_targets_fewer_than_two_pages`.

## Content validation

**`ContentReport`** (`validate/content/mod.rs`) — deterministic validators that detect factual discrepancies (unsourced claims, dropped roles, altered links) and guidance issues (low keyword coverage, AI-tell prose, formatting). **Critical issues only from deterministic checks** against the candidate's own source document; no model may emit a Critical. The code roster + severity split live in `CONTENT_ISSUE_CODES` (read the const — don't copy the count). **Evidence extraction** (`documents/evidence.rs`) — `extract_evidence` + `rank_bullets` — parses the source résumé into roles, projects, skills, and evidence spans; the `EvidenceSet` feeds both trim-panel scoring and content validation. Reported per-generation via the opaque `quality_report` column on `ai_generations` (JSON wrapper carrying per-document `ContentReport` payloads + metadata); see ADR-007 §F2 for merge rules. Consumed by: UI badge + panel (`QualityReportPanel`), with staleness detection via source text hash.

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

- HIGH: a template/layout change that breaks ATS parseability; a scoring change that violates the documented model without an ADR; an untested export error path; a header-link regression (validation must parse the same text as the renderer — `validate/` must run `extract_section` exactly as `prepare_resume_render` does, or the gate is dead for marker-wrapped text; links come from the document's extracted header, with a non-blocking `header_url_job_board` warning when a job-board host appears there); a photo path that accepts file URIs.
- MEDIUM: missing golden/edge-case test, non-deterministic snapshot, avoidable re-shaping in the render loop (perf → `performance-profiler`).

[docx-rs]: https://github.com/bokuweb/docx-rs
