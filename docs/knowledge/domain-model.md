# Domain model (core types, traits, registries)

Describes the **shape**; the source is authoritative for field-level detail. Use `graphify explain "<type>"` then read the owning file.

## Resume document

- **`DocumentModel`** — `apps/tauri/src-tauri/src/model/` (`document.rs`). The structured résumé: sections → blocks → rich text. The export pipeline and templates consume this; the renderer edits a serialized form via IPC.
- **Sections / blocks / rich text** — section ordering, content hierarchy, and customization are owned by `resume-export-expert`; see `model/` + `docs/EXPORT_TEMPLATES.md`.
- **Contact profile** — `contact_profile/` + `commands/contact_profile.rs` (header source of truth for links/contact).

## Export contract

- **`ExportRequest` / `ExportResult`** — `apps/tauri/src-tauri/src/export/types.rs` (the request/response shape: target format, template, ATS mode, locale). Owned by `resume-export-expert`; **implemented** by `pdf-docx-generator`.
- PDF path: `export/pdf/`, `export/pdf_renderer/`, `export/layout_pdf/` (printpdf + ttf-parser). DOCX path: `export/docx/`, `export/model_docx/`, `export/docx_renderer.rs` (docx-rs). Templates: `export/templates/`. Gate: `validate/`.

## Job / matching

- **Job posting / postings** — `jobs/`, `postings/`, `commands/jobs.rs`. The scraped/normalized job representation consumed by matching.
- **Matching** — `commands/match_resume.rs` (`keywords()`, `keyword_coverage()`, the score model — a weighted blend of semantic similarity + keyword coverage; **read the source for exact weights**). Recommendations: `recommend/`. Cover letters: `cover_letter/` + `commands/cover_letter.rs`.

## Automation traits + registries

- **`Scraper`** + **`SCRAPERS`** — `scraping/boards/mod.rs`; `ScraperMode` (Http/Browser); `ScrapeContext` carries a cancellation token + progress/item callbacks.
- **`Applier`** + **`APPLIERS`** — `applying/registry/mod.rs`; `ApplyContext` (form filling via `form_filler/`, `selectors/`, captcha via `captcha_handler.rs`).

## AI / providers

- **Provider adapters** — `commands/ai_provider/{ollama,openai,anthropic,gemini}.rs` behind a shared interface (`mod.rs`); business logic must not depend on a specific provider.
- **Embeddings** — `documents/mod.rs` (storage + embedding-space invalidation on model change).
- **Prompts** — `packages/prompts` (provider-aware, locale-driven templates).

## Platform / data

- **Errors** — `error.rs` (`AppError`/`AppResult`). **Config/paths** — `platform/config.rs` (`data_dir()`). **HTTP** — `net/http.rs` (`shared()`). **Tracing** — `observability.rs` (`Span`).
- **Data** — `db.rs`, `data_store.rs` (SQLite); migrations + GDPR (`commands/privacy.rs`) owned by `rust-backend-architect` (data security lens → `tauri-security-reviewer`).
