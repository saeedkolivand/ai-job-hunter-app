# Knowledge base (`docs/knowledge/`)

Last updated: 2026-07-16

A **thin, pointer-style** index for AI agents (and humans). It describes _shape and contracts_ and points at the **owning source symbol**; it deliberately does **not** copy drift-prone literals (scoring weights, template/board counts) — those live in code.

## How agents use this

**Context-source priority: graphify → source → docs/knowledge → lessons.**

1. graphify — MCP `query_graph` / `get_node` when connected, else `graphify query "<question>"` / `graphify explain "<concept>"` — scoped subgraph first.
2. **Source is authoritative** for any region edited this turn (graphify can lag un-indexed edits until `graphify update .`).
3. These knowledge files for shape/contracts/standards.
4. Lessons (`.claude/hooks/lessons.mjs query …`) for prior experience — on-demand, never bulk-loaded.

Read the minimum; **stop at ~90% confidence**.

## Files

| File                                                             | What it covers                                                                                                             |
| ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| [architecture.md](architecture.md)                               | Module map, Rust/TS boundary, L0–L3 layers, data flow, **feature ownership**                                               |
| [builder-form-pattern.md](builder-form-pattern.md)               | Resume Builder RHF form editing + Zustand persistence, schema, field array pattern                                         |
| [dependency-map.md](dependency-map.md)                           | Dependency hubs + boundary rules + key manifests (pointer)                                                                 |
| [domain-model.md](domain-model.md)                               | Core types/traits + registries (DocumentModel, JobPosting, ExportRequest/Result, Scraper/SCRAPERS)                         |
| [resume-domain.md](resume-domain.md)                             | Resume + ATS + export: sections, templates, country standards, ATS scoring model, PDF/DOCX contract                        |
| [automation-domain.md](automation-domain.md)                     | Scraping + AI-provider: registries, resilience, provider abstraction, embeddings, streaming, prompts                       |
| [../SCRAPING_ENDPOINTS.md](../SCRAPING_ENDPOINTS.md)             | Per-board scraping endpoint reconnaissance (external snapshot — see the doc)                                               |
| [extension-domain.md](extension-domain.md)                       | Browser extension (MV3) + desktop bridge: auth model, transport, protocol lockstep, store policy                           |
| [github-projects-import.md](github-projects-import.md)           | GitHub repository import for resume builder Projects step: Rust fetch + SSRF guard, AI bullet generation, modal UI         |
| [document-record-wire-format.md](document-record-wire-format.md) | DocumentRecord serde renames = backup-bundle on-disk format; intentional divergence from TS app model                      |
| [matching-algorithm.md](matching-algorithm.md)                   | Keyword-coverage scoring kernel (Autopilot + ATS), caching, gap analysis, intentional flat-coverage simplification         |
| [persistence.md](persistence.md)                                 | SQLite + transactions, `db::open`, DataStore trait, backup/restore, Resettable registry                                    |
| [anti-abuse-limits.md](anti-abuse-limits.md)                     | Rate + concurrency limits, per-provider daily ceilings, runtime configuration                                              |
| [performance-rules.md](performance-rules.md)                     | Hot paths, async-runtime discipline, query-client tuning, token/cost                                                       |
| [security-rules.md](security-rules.md)                           | Capabilities, CSP, deps, secrets, privacy/GDPR, updater                                                                    |
| [event-system.md](event-system.md)                               | Centralized one-way Tauri push-event channels (`app.emit`), colon-namespaced wire names, and the `IPC_CHANNELS` complement |
| [notification-center.md](notification-center.md)                 | Persisted notification store, `AppNotification` type, Titlebar bell inbox, and route-intent dispatch                       |
| [ui-theming-accent.md](ui-theming-accent.md)                     | Runtime theme engine and customizable accent-color system (CSS vars, ThemeId, accent tokens)                               |
| [decision-records/](decision-records/)                           | ADRs (maintained by `project-steward`) — see table below                                                                   |

## Decision records index

| ADR                                                                                          | Title                                                                                     |
| -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| [ADR-001](decision-records/adr-001-rust-first-business-logic.md)                             | Rust-first business logic                                                                 |
| [ADR-002](decision-records/adr-002-dual-pdf-docx-backends-golden-parity.md)                  | Dual PDF + DOCX backends with golden-parity tests                                         |
| [ADR-003](decision-records/adr-003-centralized-platform-net-error-layers.md)                 | Centralized platform/net/error layers                                                     |
| [ADR-004](decision-records/adr-004-ports-and-adapters-frontend.md)                           | Ports & adapters in the renderer                                                          |
| [ADR-005](decision-records/adr-005-universal-thinking-normalization.md)                      | Universal thinking/reasoning normalization at the provider-adapter boundary               |
| [ADR-006](decision-records/adr-006-generation-session-store.md)                              | Single app-wide generation-session store                                                  |
| [ADR-007](decision-records/adr-007-ai-generations-application-aggregate.md)                  | `ai_generations` as the application aggregate with merge-upsert by job URL                |
| [ADR-008](decision-records/adr-008-pdf-glyph-subsetting.md)                                  | PDF glyph subsetting at export time via `parse_font`                                      |
| [ADR-009](decision-records/adr-009-resettable-reset-registry.md)                             | Full factory reset via a `Resettable` registry                                            |
| [ADR-010](decision-records/adr-010-untrusted-input-fencing.md)                               | Untrusted-input fencing for web-sourced company research                                  |
| [ADR-011](decision-records/adr-011-referral-helper-manual-only.md)                           | Referral helper is manual-only; no LinkedIn profile scraping                              |
| [ADR-012](decision-records/adr-012-html-preview-approximate.md)                              | AI-Generate live preview renders the real exported PDF                                    |
| [ADR-013](decision-records/adr-013-resume-builder-base-plus-handoff.md)                      | Resume Builder: job-agnostic base + in-memory tailor handoff                              |
| [ADR-014](decision-records/adr-014-cli-agent-shell-plugin-static-allowlist.md)               | In-app agent install via shell plugin with a static allowlist                             |
| [ADR-015](decision-records/adr-015-extension-bridge-websocket-save-origin.md)                | Extension bridge: WebSocket server with origin validation and token gate                  |
| [ADR-016](decision-records/adr-016-centralized-notification-center.md)                       | Centralized notification center (Phase 1: store + Titlebar bell)                          |
| [ADR-017](decision-records/adr-017-persisted-self-invalidating-match-score-caches.md)        | Persisted, self-invalidating match-score & posting-vector caches                          |
| [ADR-018](decision-records/adr-018-revive-accent-tinted-aurora-ambient-background.md)        | Revive accent-tinted aurora ambient background                                            |
| [ADR-019](decision-records/adr-019-resolved-performance-profile-with-real-backend-tiers.md)  | Resolved performance profile with real backend tiers                                      |
| [ADR-020](decision-records/adr-020-unified-autopilot-scoring-kernel.md)                      | Unified autopilot scoring via keyword-coverage kernel; metric relabel                     |
| [ADR-021](decision-records/adr-021-windows-installer-currentuser-scope.md)                   | Windows installer pinned to currentUser scope; one-time migration for users               |
| [ADR-022](decision-records/adr-022-atomic-store-transactions-and-centralized-db.md)          | Atomic store transactions + centralized `db::open` (WAL + busy_timeout)                   |
| [ADR-023](decision-records/adr-023-polyform-noncommercial-licensing.md)                      | PolyForm Noncommercial 1.0.0 licensing                                                    |
| [ADR-024](decision-records/adr-024-consolidated-release-commit.md)                           | Consolidated atomic release commit                                                        |
| [ADR-025](decision-records/adr-025-agent-fleet-author-critic-pairing.md)                     | Agent fleet — paired author/critic per domain                                             |
| [ADR-026](decision-records/adr-026-retire-anti-bot-boards.md)                                | Retire self-scraping anti-bot boards; cover via aggregator; keep single-job import        |
| [ADR-027](decision-records/adr-027-diagnostics-bundle-privacy-boundary.md)                   | Diagnostics-bundle privacy boundary (strict allowlist + redaction before public artifact) |
| [ADR-028](decision-records/adr-028-additive-aggregator-merge-paid-provider-cost-controls.md) | Additive aggregator merge and paid-provider cost controls                                 |

Every ADR carries a `Status` field documenting its lifecycle: `Accepted | Superseded by ADR-NNN | Deprecated`. Retired decisions are visibly retired and linked to their successor, preventing confusion.

## Canonical docs (do not duplicate — link)

`docs/ARCHITECTURE.md`, `docs/architecture-rules.md`, `docs/PATTERNS.md`, `docs/DESIGN_SYSTEM.md`, `docs/EXPORT_TEMPLATES.md`, `docs/API.md`, `docs/DESIGN_DECISIONS.md`, and the graphify graph (`graphify-out/`).

**Agent system:** interactive explainer at `landing/agent-system.html` documents the 23-agent fleet, pairing structure, and command routing.

> Maintained **only** by `project-steward`. Keep each file ≤ ~150 lines. After code/doc changes: `graphify update .`.
