# Knowledge base (`docs/knowledge/`)

Last updated: 2026-06-01

A **thin, pointer-style** index for AI agents (and humans). It describes _shape and contracts_ and points at the **owning source symbol**; it deliberately does **not** copy drift-prone literals (scoring weights, template/board counts) — those live in code.

## How agents use this

**Context-source priority: graphify → source → docs/knowledge → lessons.**

1. `graphify query "<question>"` / `graphify explain "<concept>"` — scoped subgraph first.
2. **Source is authoritative** for any region edited this turn (graphify can lag un-indexed edits until `graphify update .`).
3. These knowledge files for shape/contracts/standards.
4. Lessons (`.claude/hooks/lessons.mjs query …`) for prior experience — on-demand, never bulk-loaded.

Read the minimum; **stop at ~90% confidence**.

## Files

| File                                         | What it covers                                                                                       |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| [architecture.md](architecture.md)           | Module map, Rust/TS boundary, L0–L3 layers, data flow, **feature ownership**                         |
| [dependency-map.md](dependency-map.md)       | Dependency hubs + boundary rules + key manifests (pointer)                                           |
| [domain-model.md](domain-model.md)           | Core types/traits + registries (DocumentModel, JobPosting, ExportRequest/Result, Scraper/SCRAPERS)   |
| [resume-domain.md](resume-domain.md)         | Resume + ATS + export: sections, templates, country standards, ATS scoring model, PDF/DOCX contract  |
| [automation-domain.md](automation-domain.md) | Scraping + AI-provider: registries, resilience, provider abstraction, embeddings, streaming, prompts |
| [performance-rules.md](performance-rules.md) | Hot paths, async-runtime discipline, query-client tuning, token/cost                                 |
| [security-rules.md](security-rules.md)       | Capabilities, CSP, deps, secrets, privacy/GDPR, updater                                              |
| [decision-records/](decision-records/)       | ADRs (maintained by `project-steward`) — see table below                                             |

## Decision records index

| ADR                                                                          | Title                                                                       |
| ---------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| [ADR-001](decision-records/adr-001-rust-first-business-logic.md)             | Rust-first business logic                                                   |
| [ADR-002](decision-records/adr-002-dual-pdf-docx-backends-golden-parity.md)  | Dual PDF + DOCX backends with golden-parity tests                           |
| [ADR-003](decision-records/adr-003-centralized-platform-net-error-layers.md) | Centralized platform/net/error layers                                       |
| [ADR-004](decision-records/adr-004-ports-and-adapters-frontend.md)           | Ports & adapters in the renderer                                            |
| [ADR-005](decision-records/adr-005-universal-thinking-normalization.md)      | Universal thinking/reasoning normalization at the provider-adapter boundary |
| [ADR-006](decision-records/adr-006-generation-session-store.md)              | Single app-wide generation-session store                                    |
| [ADR-007](decision-records/adr-007-ai-generations-application-aggregate.md)  | `ai_generations` as the application aggregate with merge-upsert by job URL  |
| [ADR-008](decision-records/adr-008-pdf-glyph-subsetting.md)                  | PDF glyph subsetting at export time via `parse_font`                        |
| [ADR-009](decision-records/adr-009-resettable-reset-registry.md)             | Full factory reset via a `Resettable` registry                              |
| [ADR-010](decision-records/adr-010-untrusted-input-fencing.md)               | Untrusted-input fencing for web-sourced company research                    |
| [ADR-011](decision-records/adr-011-referral-helper-manual-only.md)           | Referral helper is manual-only; no LinkedIn profile scraping                |
| [ADR-012](decision-records/adr-012-html-preview-approximate.md)              | AI-Generate live preview is an approximate HTML mirror                      |

## Canonical docs (do not duplicate — link)

`docs/ARCHITECTURE.md`, `docs/architecture-rules.md`, `docs/PATTERNS.md`, `docs/DESIGN_SYSTEM.md`, `docs/EXPORT_TEMPLATES.md`, `docs/API.md`, `docs/DESIGN_DECISIONS.md`, and the graphify graph (`graphify-out/`).

> Maintained **only** by `project-steward`. Keep each file ≤ ~150 lines. After code/doc changes: `graphify update .`.
