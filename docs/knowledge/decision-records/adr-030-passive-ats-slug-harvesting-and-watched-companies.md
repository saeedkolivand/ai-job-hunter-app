# ADR-030 — Passive ATS slug harvesting + watched companies

**Status:** Accepted
**Date:** 2026-07-21
**Deciders:** repo owner, main session

## Context

Company-scoped ATS scrapers (Greenhouse, Lever, Personio, Workable, Ashby, Recruitee, SmartRecruiters, Breezy, BambooHR, Pinpoint, Rippling) require hand-typed company slugs that users cannot know in advance. Meanwhile, every aggregator and board job posting already leaks the ATS slug in its apply or redirect URL. A curated 59-company seed exists (`scraping/boards/ats_seed.rs`, surfaced via `BoardCatalogEntry.seededCompanies`); pure per-ATS URL parsers already exist in `scraping/scrape_url/mod.rs` for Greenhouse, Lever, Ashby, SmartRecruiters, Personio, and Workday. Autopilot targets (`AutopilotTargetSchema`) have no companies field — autopilot ATS fan-out uses the curated seed only. The manual scrape request's `companies` array is the only user slug entry today (comma-separated text input). Automation to extract, persist, and surface discovered slugs is absent; users must remember slug formats or paste new ones repeatedly.

## Decision

### (a) Pure extractor, single source of truth

Add `extract_ats_ref(url) -> Option<(AtsKind, slug)>` as a new pure module under `scraping/` that REUSES (hoists if needed) the existing `scrape_url` URL parsers rather than duplicating shapes. Extend existing parsers to cover Workable, Recruitee, Breezy, BambooHR, Pinpoint per documented endpoint shapes in `docs/SCRAPING_ENDPOINTS.md`. `AtsKind` aligns with existing registry board IDs (no parallel enum divergence). Near-miss URLs (e.g., Greenhouse blog pages) return None. Host matching is case-insensitive; slug casing is preserved exactly (Ashby has strict casing requirements).

### (b) Store: `DiscoveredCompanyStore` with transactional migrations

Create a new SQLite database via `db::open` (per ADR-022) with table `discovered_companies(id, ats_kind, slug, display_name, first_seen_at, last_seen_at, seen_count, source, starred, UNIQUE(ats_kind, slug))`. Upsert logic bumps `last_seen_at` and `seen_count` on slug re-encounter and backfills `display_name` if empty. Register in the `Resettable` reset registry (ADR-009) and `DataStore` backup bundle (section `discoveredCompanies`). The `source` column is free-text (`'scrape' | 'extension' | 'seed'` initially) so future feeders (active probers, extension fingerprinting, community slug directories) require no schema changes.

### (c) Harvest sites — parse-only, zero new network

Integrate two parse-only passes: (1) after the engine returns in `commands/scrape.rs` and after `scrape_url` single-adds, harvest every stored posting URL before `job_complete`; (2) extension single-job import after URL resolve (via `extension_bridge/import_flow.rs`). Batch all discoveries into single-transaction upserts. No new network calls; parse existing URLs only.

### (d) Surfacing: slug typeahead in ScrapeForm

Refactor the ScrapeForm company-slug field into a typeahead primitive modeled on the existing `LocationInput` component (no new UI dependency). Feed it with a new `discovery` IPC namespace that searches over slug + display_name, ranking by most-seen count first and merging discovered rows with curated seeds. Curated seeds are display-only unless starred or used. An empty-state link explains "what is a slug?" via `SetupHint`.

### (e) Watched companies: runtime-resolved star list

Starred discoveries are user-watched companies. Autopilot's board step gains an optional boolean `watchedCompaniesOnly?: boolean` on `AutopilotTargetSchema` (additive; old records unaffected). At run time, the autopilot engine resolves this flag to the currently-starred set and fans out to per-company scrapers under the existing server-side board/company caps. Starred curated seeds are materialized as rows with source='seed' and starred=true. A frozen company list was rejected so that star changes propagate immediately without editing every autopilot.

### (f) IPC via standard 5 touchpoints

Add discovery contract to `packages/shared/src/ipc/contracts/discovery.ts` with `searchCompanies`, `setStarred`, and `watched` methods (channels: `discovery:searchCompanies|setStarred|watched`). Implement handlers in `commands/discovery.rs`. Wire service hooks in `renderer/services/use-discovery/`. Generate IPC bindings via `pnpm gen:ipc` and commit output. See the contract file for the canonical method shapes and query-key registrations.

## Alternatives rejected

- **Duplicate URL shapes in a second parser** — risk of divergence from `scrape_url`; reuse existing parsers instead.
- **Frozen watched-company list on each autopilot** — stale when user stars/unstars; runtime resolution chosen so changes propagate without editing records.
- **Fold discovered companies into an existing store** — separate concern with distinct lifecycle; own store avoids coupling.
- **Typed `AtsKind` enum decoupled from registry board IDs** — two vocabularies create drift; align with existing board-id registry.
- **Active slug probing / extension fingerprinting / community directory in v1** — explicitly future work; `source` column designed as the entry point for these feeders.

## Consequences

Slugs populate the autocomplete after ordinary scraping with zero user effort. The curated seed list stops being the ceiling of ATS coverage as discoveries grow organically. Watched-company autopilot runs track the user's current stars live, with no stale frozen lists. New store joins backup and privacy reset via standard registries. Harvesting adds a bounded parse pass per ingest with no network overhead. The extractor becomes the single URL-shape authority, shared with the extension import path and future Feature C URL resolution. Users never need to hand-type a slug again once they've scraped any posting from that ATS company. Watched fan-out is routed per board through the engine's seeded-companies override (replacing the original flat union design), so a board with no matching stars gets a clean needs-company skip.

Owning symbols: `scraping/ats_ref.rs::extract_ats_ref`, `discovered/mod.rs::DiscoveredCompanyStore`, `commands/discovery.rs::harvest_ats_refs`, `autopilot_helpers/mod.rs::resolve_watched_companies`, `scraping/engine::scrape_boards_with_overrides`, `extension_bridge/import_flow.rs`, `packages/shared/src/ipc/contracts/discovery.ts`, `packages/ui` CompanyTypeahead.
