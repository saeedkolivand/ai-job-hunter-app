# Scraping domain (boards, company-scoped, aggregator)

Last updated: 2026-07-01

Describes the job-scraping subsystem: board registry (21 active scrapers), company-scoped ATS boards, and the Adzuna/JSearch aggregator. **Shape only** — refer to source for implementation detail. See `docs/SCRAPING_ENDPOINTS.md` for verified endpoint snapshots (external reconnaissance) and `docs/knowledge/decision-records/adr-026-retire-anti-bot-boards.md` for the retirement rationale.

## Board registry & catalog

- **`Scraper` trait** — `apps/desktop/src-tauri/src/scraping/boards/mod.rs`. Every board implements `Scraper: Clone + Send + Sync + Debug`.
- **`SCRAPERS`** — registry of all enabled scrapers (built at compile time, no runtime plugin system).
- **`BOARD_IDS`** — const array in `packages/shared/src/schemas/index.ts`; lists all scrapeable boards (21 total, `aggregator` counted once among them). `AGGREGATOR_BOARD_ID = 'aggregator'` is the stable catalog id for the Adzuna/JSearch provider.
- **Catalog** — `ScraperEngine::catalog()` (Rust) → `boards.catalog()` IPC → `useBoardsCatalog()` hook. Exposes per-board metadata:
  - `id` (slug)
  - `name`, `icon` (UI)
  - `auth` — `guest` / `optional` / `required` (affects UI gate + backend skip logic)
  - `requiresCompany` — boolean; when true, scrape form shows a "Companies" field (new in PR #464)
  - `mode` — `http` or `browser`

## Company-scoped boards (PR #464)

Company-scoped ATS boards require company slugs instead of free-text keyword searches. Each board declares `requiresCompany=true` in the catalog metadata and implements its own fanout and filtering logic. For the authoritative list of boards and per-board limits, see `apps/desktop/src-tauri/src/scraping/boards/` (each module) and the registry `apps/desktop/src-tauri/src/scraping/boards/mod.rs` (`SCRAPERS`).

### BoardSearchInput contract

- **`companies?: string[]`** — optional list of company slugs (e.g. `["stripe", "notion"]`). Populated only when scraping a `requiresCompany` board.
- Generated from `packages/shared/src/ipc/contracts/scrape.ts` → Zod → `pnpm gen:ipc` → `apps/desktop/src-tauri/src/ipc_contracts/scrape.rs`.

### Skip state: `needs-company`

When a company-scoped board is selected with an empty `companies` list:

- Backend: skip the board, emit `BoardScrapeSummary { skipped: Some("needs-company") }`
- Renderer: display a sign-in/config prompt (same pattern as `needs-login`)

### Hardening (PR #467)

- **SSRF guard:** Personio & Recruitee validate company slug as a DNS label (alphanumeric + hyphen, max 63 chars)
- **Per-company dedup:** SmartRecruiters + Personio deduplicate results within each company (partial failure isolation: one company's error doesn't block others)
- **Consistent IDs:** `personio::make_job_id(company, position_id)` ensures job IDs match across ingestion paths (scrape + URL resolve)

### PR 1 of the ATS-boards program (2026-07-01): pinpoint, rippling, breezy, bamboohr

Four more company-scoped boards, endpoint-reconnaissance-ported from `santifer/career-ops` (MIT) — **not yet re-verified live**; see `docs/SCRAPING_ENDPOINTS.md` for the unverified-endpoint caveat and per-board detail.

- **Pinpoint** (`{slug}.pinpointhq.com/postings.json`) and **Breezy HR** (`{slug}.breezy.hr/json`) — subdomain-scoped, DNS-label SSRF guard (same shape as Personio). Neither response has a stable job id, so the (per-company deduped) posting URL doubles as the id.
- **Rippling** (`api.rippling.com/platform/api/ats/v1/board/{slug}/jobs`) — fixed API host, slug is a percent-encoded **path segment**, not a hostname. The response `url` field is host-locked to `ats.rippling.com` before use (an untrusted response could otherwise inject arbitrary URLs into `JobPosting.url`).
- **BambooHR** (`{slug}.bamboohr.com/careers/list`) — subdomain-scoped, DNS-label SSRF guard; has a real `id` field (accepted as either JSON number or string), so the job URL is _constructed_ by the scraper (`.../careers/{id}`), not taken from the response.
- Each board's response→`JobPosting` mapping is a standalone `pub fn parse_<board>_response(...)`, unit-testable against a JSON fixture without a network round-trip (mirrors Personio's `parse_xml_feed`).

### PR 2 of the aggregator-boards program (2026-07-01): The Muse

**The Muse** (`themuse`, `apps/desktop/src-tauri/src/scraping/boards/themuse/mod.rs`) — `requires_company()` stays `false` (default): it's a **keyword aggregator**, not a company-scoped ATS board. Same career-ops reconnaissance provenance and unverified-endpoint caveat as PR 1.

- The Muse's public API (`www.themuse.com/api/public/jobs?page={n}`) has no free-text keyword param — only `category`/`level`/`location`/`company`/`page`. The scraper browses the feed and filters client-side (title+company substring for `query`, location substring for `location`), the same convention as Remotive/RemoteOK/Arbeitnow.
- Pagination: response includes `page_count`; the scraper reads it from page 0 and iterates up to `min(input.pages, page_count, 5)` — 5-page cap matches Arbeitnow's `input.pages.clamp(1, 5)`, not career-ops's 100-page crawl.
- No stable job id in the response, so the (validated `^https?://`) posting URL doubles as the id — same precedent as Breezy/Pinpoint.

## Retired boards (Glassdoor, Indeed, Xing, StepStone, Workday)

These five boards were retired as direct scrapers (ADR-026, 2026-06-21). Their Rust modules are deleted; the registry went from 21 → 16 boards. Coverage is now provided by the Aggregator. The single-job import resolvers (`scrape_url::canonical_job_url` for Indeed, `scrape_url::try_workday`) and the dormant `board_login`/credential machinery are **deliberately kept** — see ADR-026 for the full keep-list and rationale.

## Aggregator board (Adzuna + JSearch)

**Purpose:** Cover anti-bot sites (Indeed, Glassdoor, Xing, Workday, StepStone) that return empty results or errors when self-scraped. Uses a provider registry pattern: Adzuna (primary, free) with JSearch (paid fallback, invoked only on Adzuna errors).

**Full-description resolution:** Aggregator sources return short snippets; the detail pane auto-fetches full descriptions on open by following redirect chains and re-dispatching to named-board handlers. See `apps/desktop/src-tauri/src/scraping/http/html_to_markdown.rs` and `scraping/boards/aggregator/mod.rs` for fetch logic, IP-guarding, and snippet-vs-resolved floor semantics.

**Keys and configuration:**

- Adzuna and JSearch API keys are stored in the OS keyring and never logged (encrypted at rest, decrypted only in Rust).
- Settings → Jobs exposes UI to enter/remove credentials.
- Keys are read on-demand via `credentials::read_credential` (module at `apps/desktop/src-tauri/src/credentials/mod.rs`).

**Provider details, endpoints, and fallback logic:** see `apps/desktop/src-tauri/src/scraping/boards/aggregator/mod.rs` (AdzunaProvider, JSearchProvider, and JobProvider trait).

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

## Full-description resolution on detail-pane open

**Aggregator short snippets → full descriptions:** Adzuna search API returns snippets (~200–500 chars). Detail pane auto-fetches on open when aggregator source + description < 700 chars, following the redirect chain and re-dispatching named-board handlers on the final URL. If the resolved text is meaningfully longer, it replaces the snippet; otherwise the snippet floor is kept. Redirect following is IP-guarded per-hop (closes DNS-rebinding TOCTOU); 429/login-wall/error returns Ok(None) and the snippet is retained.

- **Resolver:** `apps/desktop/src-tauri/src/commands/scrape.rs: scrape_resolve_url(req)` — public command invoked by detail pane
- **Re-dispatch:** `apps/desktop/src-tauri/src/scraping/scrape_url/mod.rs: resolve_full_description()` — follows redirect and re-dispatches handlers per final URL
- **Pane gate:** `apps/desktop/src/renderer/features/jobs/components/JobDetailPane/index.tsx` — on-open resolve if (isAggregatorSource && descLength < 700); keep-longer merge logic
- **Description mutation & re-score:** Backend command `scrape_update_description(id, text)` writes resolved text to the live `PostingsCache`. The frontend's `MatchScoresProvider` holds a reactive `requested` set; when the description is updated, the per-job match score is re-computed on-demand via `useJobMatchScore` (single-job scoring, not batch). See IPC contract in `packages/shared/src/ipc/contracts/scrape.ts`.

## Source pointers

- **Board enum + registry:** `apps/desktop/src-tauri/src/scraping/boards/mod.rs` (`Scraper`, `SCRAPERS`)
- **IPC contract:** `packages/shared/src/ipc/contracts/scrape.ts` (`BoardSearchInput`, `updateDescription`)
- **Engine skip logic:** `apps/desktop/src-tauri/src/scraping/engine/mod.rs` (handles `needs-login` + `needs-company`)
- **Company-scoped implementations:**
  - Greenhouse: `apps/desktop/src-tauri/src/scraping/boards/greenhouse/mod.rs`
  - Lever: `apps/desktop/src-tauri/src/scraping/boards/lever/mod.rs`
  - Ashby: `apps/desktop/src-tauri/src/scraping/boards/ashby/mod.rs`
  - Personio: `apps/desktop/src-tauri/src/scraping/boards/personio/mod.rs`
  - Recruitee: `apps/desktop/src-tauri/src/scraping/boards/recruitee/mod.rs`
  - SmartRecruiters: `apps/desktop/src-tauri/src/scraping/boards/smartrecruiters/`
  - Pinpoint: `apps/desktop/src-tauri/src/scraping/boards/pinpoint/mod.rs`
  - Rippling: `apps/desktop/src-tauri/src/scraping/boards/rippling/mod.rs`
  - Breezy HR: `apps/desktop/src-tauri/src/scraping/boards/breezy/mod.rs`
  - BambooHR: `apps/desktop/src-tauri/src/scraping/boards/bamboohr/mod.rs`
- **Keyword aggregators (client-side filter, no server-side search):**
  - The Muse: `apps/desktop/src-tauri/src/scraping/boards/themuse/mod.rs`
- **Aggregator:**
  - Registry: `apps/desktop/src-tauri/src/scraping/boards/aggregator/`
  - Adzuna provider: `apps/desktop/src-tauri/src/scraping/boards/aggregator/adzuna.rs`
  - JSearch provider: `apps/desktop/src-tauri/src/scraping/boards/aggregator/jsearch.rs`
- **Frontend:** Service hook `apps/desktop/src/renderer/services/boards.ts` (scrape mutations)
- **Settings UI:** `apps/desktop/src/renderer/features/settings/routes/JobsSettings.tsx`
- **Detail pane:**
  - Resolve + merge gate: `apps/desktop/src/renderer/features/jobs/components/JobDetailPane/index.tsx` (on-open resolve if short snippet; keep-longer merge logic; calls `scrape_resolve_url` + `scrape_update_description` IPC)
  - Description formatting: `apps/desktop/src-tauri/src/scraping/http/html_to_markdown` — Rust module converting HTML job descriptions to Markdown
  - Rendering: `JobDescription` component (`packages/ui/src/components/JobDescription/index.tsx`) renders Markdown via react-markdown with design-token-only styling
- **Rust commands:** `apps/desktop/src-tauri/src/commands/scrape.rs` (`scrape_resolve_url`, `scrape_update_description`)
- **PostingsCache mutation:** `apps/desktop/src-tauri/src/postings/mod.rs: update_description(job_id, text)` — in-place cache mutation; text-hash-keyed result/embedding caches auto-invalidate on changed job text

## See also

- `docs/SCRAPING_ENDPOINTS.md` — verified external endpoint reconnaissance (21 active boards, `aggregator` counted once among them; retired boards noted)
- `docs/knowledge/domain-model.md` — brief mention of `Scraper` trait + catalog
- `docs/ARCHITECTURE.md` — high-level diagram of scraping + IPC boundary
