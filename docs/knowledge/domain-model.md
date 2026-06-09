# Domain model (core types, traits, registries)

Last updated: 2026-06-09

Describes the **shape**; the source is authoritative for field-level detail. Use `graphify explain "<type>"` then read the owning file.

## Resume document

- **`DocumentModel`** — `apps/tauri/src-tauri/src/model/` (`document.rs`). The structured résumé: sections → blocks → rich text. The export pipeline and templates consume this; the renderer edits a serialized form via IPC.
- **Sections / blocks / rich text** — section ordering, content hierarchy, and customization are owned by `resume-export-expert`; see `model/` + `docs/EXPORT_TEMPLATES.md`.
- **Contact profile** — `contact_profile/` + `commands/contact_profile.rs` (header source of truth for links/contact). Conflict detection on résumé import: `contact_profile/mod.rs: detect_contact_conflicts` (normalizers + per-field diffing); `documents.import` returns additive `contactConflicts` / `suggestedContact` fields (loose JSON, no schema change, import never gated). Renderer resolution: `components/generation/EditableOutput/` area → `ContactConflictModal` (keep-mine / use-résumé per field). Conflict resolution is **local-only** — no data leaves the device.

## Export contract

- **`ExportRequest` / `ExportResult`** — `apps/tauri/src-tauri/src/export/types.rs` (the request/response shape: target format, template, ATS mode, locale). Owned by `resume-export-expert`; **implemented** by `pdf-docx-generator`.
- PDF path: `export/typst_engine/` (sole PDF backend — printpdf removed). DOCX path: `export/docx/`, `export/model_docx/` ([docx-rs][docx-rs]). Templates: `export/templates/`. Gate: `validate/`.

## Job / matching

- **Job posting / postings** — `jobs/`, `postings/`, `commands/jobs.rs`. The scraped/normalized job representation consumed by matching.
- **Matching** — `commands/match_resume.rs`: keywords lookup via `documents/keywords.rs` (shared module). Pipeline split: `keywords_normalized()` caches pre-stemmed tokens (language-agnostic), `apply_stemmer()` stems at match time using JD language detection (`whatlang` + Snowball per language). Tokenization: lowercase + synonym-normalize + language-detect + filter (drop ≤3 chars unless in `SHORT_TECH_TERMS` allowlist, drop stopwords). Corrupt-cache fallback: `from_str().unwrap_or_default()` returns empty set. **Translation (Ollama-only)** — `translation.rs`: `TranslationCache` (session-scoped HashMap); `translate_if_needed()` detects non-English via whatlang, gates on local-provider (`is_local()`), invokes Ollama, caches result, falls back to original on any failure. Called before stemming/keyword extraction. **Keyword coverage** — `keyword_coverage()` returns resume vs job overlap %; score model is weighted blend of semantic similarity + keyword coverage (**read source for exact weights**). Auto-scoring via service hook `useJobMatchScore` with `enabled` param, gated by sequential FIFO `ScoringScheduler` (CONCURRENCY=1, prevents mass-concurrent IPC bursts). Recommendations: `recommend/`. Cover letters: `cover_letter/` + `commands/cover_letter.rs`.

## Referrals

- **`referrals` store** — `referrals/mod.rs` (L1 domain); full CRUD via `commands/referrals.rs`. Each record captures a contact (name, company, role, relationship) plus a generated or hand-edited referral note in up to three formats (email, LinkedIn message, cold-ask). Local-only — no data leaves the device.
- **`ReferralModal`** — `apps/tauri/src/renderer/features/autopilot/components/ReferralModal` (or adjacent apply-flow component); surfaced in the autopilot apply flow.
- **Prompt layer** — `buildReferralPrompt` / `generateReferral` in `packages/prompts`; produces connection-note (≤ 300 chars), email, and LinkedIn-message variants; reuses `streamGenerate`.
- **Data lifecycle** — wired into `manage_resettable` (full reset) and `commands/data.rs::build_bundle` (export/import). See [ADR-011](decision-records/adr-011-referral-helper-manual-only.md) for the decision to keep entry manual and discard LinkedIn scraping.

## Automation traits + registries

- **`Scraper`** + **`SCRAPERS`** — `scraping/boards/mod.rs`; `ScraperMode` (Http/Browser); `ScrapeContext` carries a cancellation token + progress/item callbacks.
- _(No applier registry: the auto-apply engine was removed — the app is an apply **assistant**. See [automation-domain.md](automation-domain.md).)_

## AI / providers

- **Provider adapters** — `commands/ai_provider/{ollama,openai,anthropic,gemini}.rs` behind a shared interface (`mod.rs`); business logic must not depend on a specific provider.
- **Embeddings** — `documents/mod.rs` (storage + embedding-space invalidation on model change).
- **Prompts** — `packages/prompts` (provider-aware, locale-driven templates).
- **`ai_generations` aggregate** — per-job merge-upsert by `job_url` (`save_application` + pure `merge_application` in `ai_generations/mod.rs`); `applied` status derived via `applied_job_urls()`. See [ADR-007](decision-records/adr-007-ai-generations-application-aggregate.md).

## Platform / data

- **Errors** — `error.rs` (`AppError`/`AppResult`). **Config/paths** — `platform/config.rs` (`data_dir()`). **HTTP** — `net/http.rs` (`shared()`). **Tracing** — `observability.rs` (`Span`).
- **Data** — `db.rs`, `data_store.rs` ([SQLite][sqlite] via [rusqlite][rusqlite]); migrations + GDPR (`commands/privacy.rs`) owned by `rust-backend-architect` (data security lens → `tauri-security-reviewer`).

[docx-rs]: https://github.com/bokuweb/docx-rs
[sqlite]: https://www.sqlite.org
[rusqlite]: https://github.com/rusqlite/rusqlite
