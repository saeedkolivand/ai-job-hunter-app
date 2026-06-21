# Scraping domain (boards, company-scoped, aggregator)

Last updated: 2026-06-21

Describes the job-scraping subsystem: board registry, company-scoped ATS boards, and the Adzuna/JSearch aggregator. **Shape only** — refer to source for implementation detail. See `docs/SCRAPING_ENDPOINTS.md` for verified endpoint snapshots (external reconnaissance).

## Board registry & catalog

- **`Scraper` trait** — `apps/tauri/src-tauri/src/scraping/boards/mod.rs`. Every board implements `Scraper: Clone + Send + Sync + Debug`.
- **`SCRAPERS`** — registry of all enabled scrapers (built at compile time, no runtime plugin system).
- **Catalog** — `ScraperEngine::catalog()` (Rust) → `boards.catalog()` IPC → `useBoardsCatalog()` hook. Exposes per-board metadata:
  - `id` (slug)
  - `name`, `icon` (UI)
  - `auth` — `guest` / `optional` / `required` (affects UI gate + backend skip logic)
  - `requiresCompany` — boolean; when true, scrape form shows a "Companies" field (new in PR #464)
  - `mode` — `http` or `browser`

## Company-scoped boards (PR #464)

Six ATS boards require a company slug, not a free-text keyword search:

1. **Greenhouse** (`requires_company=true`, fan-out cap: 50)
2. **Lever** (`requires_company=true`, fan-out cap: none)
3. **Ashby** (`requires_company=true`, fan-out cap: 50)
4. **Personio** (`requires_company=true`, fan-out cap: none)
5. **Recruitee** (`requires_company=true`, fan-out cap: none)
6. **SmartRecruiters** (`requires_company=true`, fan-out cap: 20, supports keyword via `?q` param)

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

## Aggregator board (PR #465)

**Purpose:** Replace direct scraping of anti-bot sites (Indeed, Glassdoor, Xing, Workday, StepStone). Uses a provider registry pattern.

### Providers

**Adzuna** (primary, free)

- App ID + App key from https://developer.adzuna.com (user-supplied)
- Stored in OS keyring (`ai:adzuna-app-id`, `ai:adzuna-app-key`)
- Stripped from HTTP logs
- Endpoint: `https://api.adzuna.com/v1/api/jobs/{country_code}/search/1`

**JSearch** (paid, fallback only on Adzuna errors)

- API key from https://api.jsearch.io (user-supplied)
- Stored in OS keyring (`ai:jsearch-key`)
- Invoked only when Adzuna **errors** (not on legitimate empty results)
- Fallback respects cancellation signal (cancel before fallback = no fallback)
- Endpoint: `https://jskills-api.api.jsearch.io/v2/jobs-search`

### Keyring & settings

- Keys are encrypted in the OS keyring and never logged/visible in plaintext
- Settings → Jobs shows a field to enter/remove Adzuna app ID + key
- UI wraps save/remove with re-entrancy guard (rapid Enter / double-click is no-op)
- Removed OnceLock cache so new keys take effect on next search without app restart

### Behavior

- **Keyless:** Returns empty results (never crashes; logged as warning)
- **Provider errors:** Fallback to next provider (Adzuna error → JSearch)
- **Empty results:** Legitimate; no fallback triggered
- **Cancellation:** Pre-fallback cancel signal skips the paid provider entirely

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

- `docs/SCRAPING_ENDPOINTS.md` — verified external endpoint reconnaissance (20 boards + aggregator)
- `docs/knowledge/domain-model.md` — brief mention of `Scraper` trait + catalog
- `docs/ARCHITECTURE.md` — high-level diagram of scraping + IPC boundary
