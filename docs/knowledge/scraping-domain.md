# Scraping domain (boards, company-scoped, aggregator)

Last updated: 2026-06-23

Describes the job-scraping subsystem: board registry (16 active scrapers), company-scoped ATS boards, and the Adzuna/JSearch aggregator. **Shape only** — refer to source for implementation detail. See `docs/SCRAPING_ENDPOINTS.md` for verified endpoint snapshots (external reconnaissance) and `docs/knowledge/decision-records/adr-026-retire-anti-bot-boards.md` for the retirement rationale.

## Board registry & catalog

- **`Scraper` trait** — `apps/tauri/src-tauri/src/scraping/boards/mod.rs`. Every board implements `Scraper: Clone + Send + Sync + Debug`.
- **`SCRAPERS`** — registry of all enabled scrapers (built at compile time, no runtime plugin system).
- **`BOARD_IDS`** — const array in `packages/shared/src/schemas/index.ts`; lists all scrapeable boards (16 active scrapers + aggregator). `AGGREGATOR_BOARD_ID = 'aggregator'` is the stable catalog id for the Adzuna/JSearch provider.
- **Catalog** — `ScraperEngine::catalog()` (Rust) → `boards.catalog()` IPC → `useBoardsCatalog()` hook. Exposes per-board metadata:
  - `id` (slug)
  - `name`, `icon` (UI)
  - `auth` — `guest` / `optional` / `required` (affects UI gate + backend skip logic)
  - `requiresCompany` — boolean; when true, scrape form shows a "Companies" field (new in PR #464)
  - `mode` — `http` or `browser`

## Company-scoped boards (PR #464)

Company-scoped ATS boards require company slugs instead of free-text keyword searches. Each board declares `requiresCompany=true` in the catalog metadata and implements its own fanout and filtering logic. For the authoritative list of boards and per-board limits, see `apps/tauri/src-tauri/src/scraping/boards/` (each module) and the registry `apps/tauri/src-tauri/src/scraping/boards/mod.rs` (`SCRAPERS`).

### BoardSearchInput contract

- **`companies?: string[]`** — optional list of company slugs (e.g. `["stripe", "notion"]`). Populated only when scraping a `requiresCompany` board.
- Generated from `packages/shared/src/ipc/contracts/scrape.ts` → Zod → `pnpm gen:ipc` → `apps/tauri/src-tauri/src/ipc_contracts/scrape.rs`.

### Skip state: `needs-company`

When a company-scoped board is selected with an empty `companies` list:

- Backend: skip the board, emit `BoardScrapeSummary { skipped: Some("needs-company") }`
- Renderer: display a sign-in/config prompt (same pattern as `needs-login`)

### Hardening (PR #467)

- **SSRF guard:** Personio & Recruitee validate company slug as a DNS label (alphanumeric + hyphen, max 63 chars)
- **Per-company dedup:** SmartRecruiters + Personio deduplicate results within each company (partial failure isolation: one company's error doesn't block others)
- **Consistent IDs:** `personio::make_job_id(company, position_id)` ensures job IDs match across ingestion paths (scrape + URL resolve)

## Retired boards (Glassdoor, Indeed, Xing, StepStone, Workday)

These five boards were retired as direct scrapers (ADR-026, 2026-06-21). Their Rust modules are deleted; the registry went from 21 → 16 boards. Coverage is now provided by the Aggregator. The single-job import resolvers (`scrape_url::canonical_job_url` for Indeed, `scrape_url::try_workday`) and the dormant `board_login`/credential machinery are **deliberately kept** — see ADR-026 for the full keep-list and rationale.

## Aggregator board (PR #465)

**Purpose:** Cover anti-bot sites (Indeed, Glassdoor, Xing, Workday, StepStone) that return empty results or errors when self-scraped. Uses a provider registry pattern: Adzuna (primary, free) with JSearch (paid fallback, invoked only on Adzuna errors).

**Keys and configuration:**

- Adzuna and JSearch API keys are stored in the OS keyring and never logged (encrypted at rest, decrypted only in Rust).
- Settings → Jobs exposes UI to enter/remove credentials.
- Keys are read on-demand via `credentials::read_credential` (module at `apps/tauri/src-tauri/src/credentials/mod.rs`).

**Provider details, endpoints, and fallback logic:** see `apps/tauri/src-tauri/src/scraping/boards/aggregator/mod.rs` (AdzunaProvider, JSearchProvider, and JobProvider trait).

**Adzuna country-code limitation (PR #483):**

Adzuna's API is country-scoped with a fixed market allowlist; see `ADZUNA_SUPPORTED_COUNTRIES` in the aggregator module. Unsupported countries trigger a diagnostic error (not silent empty) that surfaces as `BoardScrapeSummary.error` with JSearch as fallback. Autopilot skip/error reasons surface in the run step log via `scrape_diagnostics`.

**Behavior:**

- **Keyless:** Returns empty results (never crashes; logged as warning).
- **Configured-failed:** If a provider is configured (keys present) and fails, diagnostic error surfaces as `BoardScrapeSummary.error` (not silent empty); fallback to next provider.
- **Unsupported country (Adzuna only):** Pre-request guard rejects before HTTP invocation; error includes remedy (add JSearch key for global coverage).
- **Empty results:** Legitimate (do NOT trigger fallback to next provider; only errors do).
- **Cancellation:** Pre-fallback cancel signal skips the paid provider entirely.

## Scrape results persistence (PR #463)

Results now persist across navigation thanks to React Query + backend cache:

- **Backend:** `PostingsCache` in Rust is the source of truth (populated as streamed results arrive)
- **Frontend:** Throttled `invalidatePostings()` on `job.stream` event (~1 ms throttle; eager on first item of new search)
- **Hydration:** React Query re-fetches cache on component remount → results reappear

## Source pointers

- **Board enum + registry:** `apps/tauri/src-tauri/src/scraping/boards/mod.rs` (`Scraper`, `SCRAPERS`)
- **IPC contract:** `packages/shared/src/ipc/contracts/scrape.ts` (`BoardSearchInput`)
- **Engine skip logic:** `apps/tauri/src-tauri/src/scraping/engine/mod.rs` (handles `needs-login` + `needs-company`)
- **Company-scoped implementations:**
  - Greenhouse: `apps/tauri/src-tauri/src/scraping/boards/greenhouse/mod.rs`
  - Lever: `apps/tauri/src-tauri/src/scraping/boards/lever/mod.rs`
  - Ashby: `apps/tauri/src-tauri/src/scraping/boards/ashby/mod.rs`
  - Personio: `apps/tauri/src-tauri/src/scraping/boards/personio/mod.rs`
  - Recruitee: `apps/tauri/src-tauri/src/scraping/boards/recruitee/mod.rs`
  - SmartRecruiters: `apps/tauri/src-tauri/src/scraping/boards/smartrecruiters/`
- **Aggregator:**
  - Registry: `apps/tauri/src-tauri/src/scraping/boards/aggregator/`
  - Adzuna provider: `apps/tauri/src-tauri/src/scraping/boards/aggregator/adzuna.rs`
  - JSearch provider: `apps/tauri/src-tauri/src/scraping/boards/aggregator/jsearch.rs`
- **Frontend:** Service hook `apps/tauri/src/renderer/services/boards.ts` (scrape mutations)
- **Settings UI:** `apps/tauri/src/renderer/features/settings/routes/JobsSettings.tsx`

## See also

- `docs/SCRAPING_ENDPOINTS.md` — verified external endpoint reconnaissance (16 active boards + aggregator; retired boards noted)
- `docs/knowledge/domain-model.md` — brief mention of `Scraper` trait + catalog
- `docs/ARCHITECTURE.md` — high-level diagram of scraping + IPC boundary
