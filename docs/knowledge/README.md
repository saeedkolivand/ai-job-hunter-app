# Knowledge base (`docs/knowledge/`)

Last updated: 2026-09-02

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
| [generation-domain.md](generation-domain.md)                     | Generation pipeline: stages, grounding, validation (Critical/Warning), fabrication gate, eval harness, search ranking      |
| [scraping-domain.md](scraping-domain.md)                         | Board/aggregator scraping: registries, aggregator-first routing (Adzuna/JSearch/Jooble), curated ATS seeds                 |
| [../SCRAPING_ENDPOINTS.md](../SCRAPING_ENDPOINTS.md)             | Per-board scraping endpoint reconnaissance (external snapshot — see the doc)                                               |
| [extension-domain.md](extension-domain.md)                       | Browser extension (MV3) + desktop bridge: auth model, transport, protocol lockstep, store policy                           |
| [github-projects-import.md](github-projects-import.md)           | GitHub repository import for resume builder Projects step: Rust fetch + SSRF guard, AI bullet generation, modal UI         |
| [document-record-wire-format.md](document-record-wire-format.md) | DocumentRecord serde renames = backup-bundle on-disk format; intentional divergence from TS app model                      |
| [matching-algorithm.md](matching-algorithm.md)                   | Keyword-coverage scoring kernel (Autopilot + ATS), caching, gap analysis, intentional flat-coverage simplification         |
| [persistence.md](persistence.md)                                 | State ownership + transient boundary, SQLite/`db::open`, DataStore, backup/restore, Resettable, JSON exceptions            |
| [anti-abuse-limits.md](anti-abuse-limits.md)                     | Rate + concurrency limits, per-provider daily ceilings, runtime configuration                                              |
| [performance-rules.md](performance-rules.md)                     | Hot paths, async-runtime discipline, query-client tuning, token/cost                                                       |
| [security-rules.md](security-rules.md)                           | Capabilities, CSP, deps, secrets, privacy/GDPR, updater                                                                    |
| [event-system.md](event-system.md)                               | Centralized one-way Tauri push-event channels (`app.emit`), colon-namespaced wire names, and the `IPC_CHANNELS` complement |
| [notification-center.md](notification-center.md)                 | Persisted notification store, `AppNotification` type, Titlebar bell inbox, and route-intent dispatch                       |
| [ui-theming-accent.md](ui-theming-accent.md)                     | Runtime theme engine and customizable accent-color system (CSS vars, ThemeId, accent tokens)                               |
| [agent-cli.md](agent-cli.md)                                     | CLI mode of the shipped binary (`ajh-tauri agent <verb>`): invocation, exit codes, error sentinels, binary locations       |
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
| [ADR-012](decision-records/adr-012-html-preview-approximate.md)                              | Live preview renders the real exported document via SVG; templates stay single-source     |
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
| [ADR-029](decision-records/adr-029-cross-board-job-clustering-recompute-at-ingest.md)        | Cross-board job clustering: recompute-at-ingest, pair tombstones only                     |
| [ADR-030](decision-records/adr-030-passive-ats-slug-harvesting-and-watched-companies.md)     | Passive ATS slug harvesting and watched companies                                         |
| [ADR-031](decision-records/adr-031-url-import-persists-provenance-and-harvests.md)           | URL import persists provenance and feeds slug harvesting                                  |
| [ADR-032](decision-records/adr-032-generation-pipeline-ownership-and-rule-enforcement.md)    | Generation-pipeline ownership — mechanical core-rule enforcement (Rust orchestration)     |
| [ADR-033](decision-records/adr-033-no-model-written-agent-memory.md)                         | No model-written agent memory (immutable run history, not LLM-synthesized state)          |
| [ADR-034](decision-records/adr-034-cover-letter-export-boundary-completion.md)               | Cover-letter export boundary completes a body-only letter (salutation/sign-off/signature) |
| [ADR-035](decision-records/adr-035-work-type-filter-declared-data-only.md)                   | Work-type filter classifies from declared board data only; undeclared is kept             |
| [ADR-036](decision-records/adr-036-cross-autopilot-best-matches.md)                          | Cross-autopilot best matches: fuzzy clustering, rank by ADR-020 two-block rule            |
| [ADR-037](decision-records/adr-037-agent-cli-as-binary-mode-thin-client.md)                  | Agent CLI is a mode of the shipped binary and a thin client over the loopback bridge      |
| [ADR-038](decision-records/adr-038-agent-cli-full-parity-two-tier.md)                        | Full CLI parity via a per-command policy table; curated and generic tiers kept apart      |
| [ADR-039](decision-records/adr-039-hybrid-postings-search-lexical-dense-rerank.md)           | Hybrid postings search: lexical FTS5 + dense cosine + RRF fusion + optional LLM rerank    |

### The `NNNN-` series (closed)

A second ADR tree grew under `docs/adr/` on 2026-06-12, twelve days after this one, and ran to 23 records before being folded in here on 2026-08-16. **The files were moved but deliberately not renumbered** — an ADR is a dated record, and its number is cited from commit messages, merged PR bodies, code comments and the published tech radar; renumbering would falsify all of them. The `NNNN-` series is therefore **closed**: cite these by their existing numbers, and give every NEW ADR the next `adr-NNN` number in the series above.

Because every number from 1 to 23 exists in **both** series, the digit count is what disambiguates a citation: **four digits mean the closed series** (`ADR-0013` — email-confirmation watching), **three mean the open one** (`ADR-013` — the résumé builder). Keep that padding in commit messages, code comments, docs and tech-radar `adrSlug`s. `pnpm check:agent-system` guards both series against index drift, `pnpm check:tech-radar` resolves every `adrSlug` to a real file, and `pnpm check:adr-citations` rejects any citation or `decision-records/` path that resolves to neither series. That last one runs at write time because nothing can recover which ADR a mis-padded number was meant to point at afterwards.

| ADR                                                                                     | Title                                                                   |
| --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| [ADR-0001](decision-records/0001-application-aggregate-split.md)                        | Application is the aggregate root; a Generation is a child Document     |
| [ADR-0002](decision-records/0002-coderabbit-ai-review.md)                               | CodeRabbit as the always-on AI PR reviewer                              |
| [ADR-0003](decision-records/0003-consolidate-ci-workflows.md)                           | CI workflows consolidated 16 → 9 with a DRY job bootstrap               |
| [ADR-0004](decision-records/0004-single-source-user-customizable-accent-color.md)       | Single-source accent colour with native OS integration                  |
| [ADR-0005](decision-records/0005-network-egress-privacy-boundary.md)                    | Network egress and the local-first privacy boundary (8 egress classes)  |
| [ADR-0006](decision-records/0006-support-page-faq-only-diagnostics-removed.md)          | Support is FAQ-only; the diagnostics dashboard was removed              |
| [ADR-0007](decision-records/0007-document-color-is-a-knob-not-a-template.md)            | Document colour is a knob, not a template                               |
| [ADR-0008](decision-records/0008-ai-review-enforcement.md)                              | Mandatory AI review via deterministic schema-1 verdicts                 |
| [ADR-0009](decision-records/0009-assisted-autofill.md)                                  | Assisted autofill + answer capture — user-initiated, no persistence     |
| [ADR-0010](decision-records/0010-bridge-hmac-handshake.md)                              | Extension bridge auth via mutual HMAC handshake (protocol v2)           |
| [ADR-0011](decision-records/0011-extension-ai-assist-optin.md)                          | Extension AI-assist is opt-in (billable egress, distinct from autofill) |
| [ADR-0012](decision-records/0012-ai-provider-base-url-provenance.md)                    | AI provider `base_url` provenance, not IP filtering                     |
| [ADR-0013](decision-records/0013-email-confirmation-watching.md)                        | Email-confirmation watching via IMAP app password                       |
| [ADR-0014](decision-records/0014-landing-gl-takeover.md)                                | Landing becomes a built GL experience with a semantic fallback          |
| [ADR-0015](decision-records/0015-ripbook-notebook-landing.md)                           | Landing → RIPBOOK full-WebGL kraft notebook                             |
| [ADR-0016](decision-records/0016-terminal-velocity-scroll-film-landing.md)              | Landing → TERMINAL VELOCITY CG scroll-film                              |
| [ADR-0017](decision-records/0017-landing-consolidation-static-site.md)                  | Landing returns to a self-contained static site                         |
| [ADR-0018](decision-records/0018-landing-nextjs-static-export.md)                       | Landing → Next.js static export with real routes                        |
| [ADR-0019](decision-records/0019-scroll-world-landing-route.md)                         | Landing `/world`: scroll-scrubbed papercraft camera flight              |
| [ADR-0020](decision-records/0020-crash-reporting.md)                                    | Crash reporting via Sentry — default on, desktop only                   |
| [ADR-0021](decision-records/0021-editor-owns-resume-header.md)                          | The editor owns the résumé header                                       |
| [ADR-0022](decision-records/0022-live-model-listing-no-hardcoded-defaults.md)           | Live model listing — providers are the source, no hardcoded defaults    |
| [ADR-0023](decision-records/0023-web-search-is-a-separate-axis-from-the-ai-provider.md) | Web search is a separate axis from the AI provider                      |

Every ADR carries a `Status` field documenting its lifecycle: `Accepted | Superseded by ADR-NNN | Deprecated`. Retired decisions are visibly retired and linked to their successor, preventing confusion.

## Canonical docs (do not duplicate — link)

`docs/ARCHITECTURE.md`, `docs/architecture-rules.md`, `docs/PATTERNS.md`, `docs/DESIGN_SYSTEM.md`, `docs/EXPORT_TEMPLATES.md`, `docs/API.md`, `docs/DESIGN_DECISIONS.md`, and the graphify graph (`graphify-out/`).

**Agent system:** interactive explainer at `apps/landing/public/agent-system.html` documents the agent fleet, pairing structure, and command routing.

> Maintained **only** by `project-steward`. Per-domain knowledge docs may exceed ~150 lines (e.g., scraping-domain.md, extension-domain.md). After code/doc changes: `graphify update .`.
>
> **Open decision:** the ~150-line target is aspirational, not enforced. `extension-domain.md` sits right at it; `scraping-domain.md` is roughly 500 lines and needs a split-or-retire call (split by sub-domain, or retire the historical PR-program narrative that source now documents better than prose). Deferred deliberately — restructuring it mid-batch would churn every inbound pointer at once. Note the target is about lines, not bytes: both files carry very wide table rows.
