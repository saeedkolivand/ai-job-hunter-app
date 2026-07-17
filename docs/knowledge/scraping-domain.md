# Scraping domain (boards, company-scoped, aggregator)

Last updated: 2026-07-17

Describes the job-scraping subsystem: board registry (24 active scrapers), company-scoped ATS boards, and the Adzuna/JSearch aggregator. **Shape only** — refer to source for implementation detail. See `docs/SCRAPING_ENDPOINTS.md` for verified endpoint snapshots (external reconnaissance) and `docs/knowledge/decision-records/adr-026-retire-anti-bot-boards.md` for the retirement rationale.

## Board registry & catalog

- **`Scraper` trait** — `apps/desktop/src-tauri/src/scraping/types/mod.rs` (bounds: `Send + Sync`). Every board implements it.
- **`SCRAPERS`** — registry in `scraping/boards/mod.rs` of all enabled scrapers (built at compile time, no runtime plugin system).
- **`BOARD_IDS`** — const array in `packages/shared/src/schemas/index.ts`; lists all scrapeable boards (24 total, `aggregator` counted once among them). `AGGREGATOR_BOARD_ID = 'aggregator'` is the stable catalog id for the Adzuna/JSearch provider.
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
- Pagination: response includes `page_count`; the scraper reads it from page 0 and iterates up to `min(input.pages, page_count, 5)` — 5-page cap matches Arbeitnow's `input.pages.clamp(1, 5)`, not career-ops's 100-page crawl. A paginated board that fails on page N>0 after collecting items keeps its partial harvest and signals truncation (e.g., arbeitnow page 2 429'd after 20 postings collected → `count: 20, error: None, skipped: None, truncated: Some("page 2 of 5 failed: HTTP 429")`). Source: `scraping/boards/{themuse,arbeitnow,arbeitsagentur}/mod.rs` (report truncation via `ScrapeContext.report_truncation()`), `scraping/engine/mod.rs` (`BoardScrapeSummary.truncated` field).
- No stable job id in the response, so the (validated `^https?://`) posting URL doubles as the id — same precedent as Breezy/Pinpoint.

### PR 3 (2026-07-02): Workable (company-scoped, live-verified) + Comeet (credentialed)

Two more boards, bringing the registry to 23 (Jobicy later added in #700, bringing it to 24). Neither is career-ops-ported — see `docs/SCRAPING_ENDPOINTS.md` for full endpoint detail.

- **Workable** (`workable`, `apps/desktop/src-tauri/src/scraping/boards/workable/mod.rs` — company-scoped, `requires_company()=true`). Endpoint `apply.workable.com/api/v1/widget/accounts/{slug}?details=true` is **live-verified** (real request against slug `careers-at-sleek`, 55 jobs), not reconnaissance-ported. Company display name comes from the response's top-level `name` (falls back to the slug); job URL is host-locked to `apply.workable.com`; job id is namespaced `workable:{slug}:{shortcode}` since a bare shortcode is only unique within one tenant. Each job row is deserialized independently (`rows_to_jobs`), mirroring Rippling's per-row resilience idiom, so one malformed row can't zero out a whole company's results.
- **Comeet** (`comeet`, `apps/desktop/src-tauri/src/scraping/boards/comeet/mod.rs` — credentialed, single-company, `requires_company()` stays `false` since the "company" is a fixed per-user credential rather than a per-search input). Endpoint `www.comeet.co/careers-api/2.0/company/{uid}/positions?token={token}` is confirmed live (400 without real credentials) but its **response shape is unconfirmed** — built from the career-ops (MIT) field spec, needs live-verification with a real company UID + token via the Settings UI. Clones the Apify LinkedIn provider's credential pattern: company UID + API token read via `credentials::read_credential("ai:comeet-company-uid" / "ai:comeet-api-token")` at scrape time (slots in `packages/shared/src/provider-slots.ts`); absent credentials → keyless-empty (`Ok(vec![])`), never an error; job URL is host-locked to `comeet.co`; client-side query/location filtering reuses the shared `boards::common::matches_filters` helper (originally The Muse-local, extracted to `common.rs` once Comeet needed the identical filter). `auth()` is explicitly `Guest` (credentials aren't surfaced through the board-login connect flow) — same explicit-override precedent as the Aggregator board.
- `scraping/trust/mod.rs`'s `ATS_ALLOWLIST` gained `comeet.co` (Workable's `workable.com`/`apply.workable.com` were already present from an earlier pass) so postings from either board don't trip a spurious `CompanyDomainMismatch` badge.

## ATS seed table (PR #620, wired in PR #621)

**Purpose:** Curated, live-verified (2026-07-11) static table of 59 companies → (ats, slug) mappings. Wired into engine routing (PR #621): when a company-scoped ATS board is selected with an empty `companies` list, the engine auto-populates it from `ats_seed::by_ats(scraper.id())`. Reachable by both autopilot (which passes no companies) and manual search. User-provided companies take priority; if supplied, the seed is ignored. Unseeded boards still skip when no companies are provided. Catalog surface exposes `seededCompanies` (curated company names per board), and a shared `SeededCompaniesNote` disclosure component renders in both the autopilot wizard and manual jobs picker.

**Shape and quirks:**

- **Module:** `apps/desktop/src-tauri/src/scraping/boards/ats_seed.rs` — struct `AtsSeedEntry` with fields: `company`, `ats` (matches `Scraper::id()`), `slug`, `tld` (Personio only), `dach` (DACH market flag).
- **Entries:** 59 total; 23 DACH-flagged (Germany/Austria/Switzerland market). Organized per ATS: 27 Greenhouse, 4 Lever, 9 Ashby, 4 SmartRecruiters, 5 Recruitee, 7 Personio, 3 Workable.
- **Personio TLD quirk:** Each entry has `tld: Some("de")` or `Some("com")` (one TLD per company) or `None` for all other ATS boards. Personio's URL pattern requires the TLD: `https://{slug}.jobs.personio.{tld}/xml`.
- **Ashby casing:** Slug casing is exact and preserved verbatim (e.g., `Linear`, `Perplexity`); must match the registered board.
- **Lever and SmartRecruiters churn:** Slugs churn fastest (companies migrate off or go dormant); re-verify live before trusting future updates to this table.
- **Accessor functions:** `all()` → all 59 entries in source order; `by_ats(board_id)` → entries for one ATS board (e.g., `"greenhouse"`).
- **Test coverage:** Compile-time truth table (`apps/desktop/src-tauri/src/scraping/boards/ats_seed/test.rs`) verifies table integrity: non-empty, every `ats` value matches a registered `Scraper::id()`, Personio entries have exactly one TLD, DACH count ≥ 20 (currently 23), no duplicate ats-slug pairs, etc.

## Retired boards (Glassdoor, Indeed, Xing, StepStone, Workday)

These five boards were retired as direct scrapers (ADR-026, 2026-06-21). Their Rust modules are deleted; the registry went from 21 → 16 boards. Coverage is now provided by the Aggregator. The single-job import resolvers (`scrape_url::canonical_job_url` for Indeed, `scrape_url::try_workday`) and the dormant `board_login`/credential machinery are **deliberately kept** — see ADR-026 for the full keep-list and rationale.

## Aggregator board (Adzuna + JSearch + Jooble + Apify)

**Purpose:** Cover anti-bot sites (Indeed, Glassdoor, Xing, Workday, StepStone) that return empty results or errors when self-scraped. Uses a provider registry pattern: Adzuna (primary, free) → JSearch (paid fallback) → Jooble (third-tier BYO-key, ~67-country coverage) → ApifyLinkedInProvider (opt-in, token + autofill-gate gated).

**Full-description resolution:** Aggregator sources return short snippets; the detail pane auto-fetches full descriptions on open by following redirect chains and re-dispatching to named-board handlers. See `html_to_markdown()` in `apps/desktop/src-tauri/src/scraping/http/mod.rs` and `scraping/boards/aggregator/mod.rs` for fetch logic, IP-guarding, and snippet-vs-resolved floor semantics.

**Keys and configuration:**

- Adzuna, JSearch, Jooble, and Apify API keys are stored in the OS keyring and never logged (encrypted at rest, decrypted only in Rust).
- Settings → Jobs exposes UI to enter/remove credentials (seven `AggregatorKeyField` controls: two for Adzuna's app-id/app-key, one each for JSearch/Jooble/Apify token, plus Comeet company UID and API token).
- Keys are read on-demand via `credentials::read_credential` (module at `apps/desktop/src-tauri/src/credentials/mod.rs`) into the respective slots: `ai:adzuna-app-id`, `ai:adzuna-app-key`, `ai:jsearch-key`, `ai:jooble-key`, `ai:apify-token`, `ai:comeet-company-uid`, `ai:comeet-api-token`.
- **Path-embedded-key redaction (PR #618):** Jooble embeds its API key in the URL path (`POST https://jooble.org/api/{key}`), unlike Adzuna/JSearch which use query params. New `FetchOptions.redact_path` boolean + `safe_log_url(url, redact_path)` in `apps/desktop/src-tauri/src/scraping/http/mod.rs` redact the entire path when `true`, keeping logs safe. Providers using path-embedded keys must pass `redact_path: true` in their `FetchOptions`.

**Provider details, endpoints, and fallback logic:** see `apps/desktop/src-tauri/src/scraping/boards/aggregator/providers.rs` (AdzunaProvider, JSearchProvider, JoobleProvider, ApifyLinkedInProvider, and JobProvider trait).

**Adzuna country-code limitation (PR #483):**

Adzuna's API is country-scoped with a fixed market allowlist; see `ADZUNA_SUPPORTED_COUNTRIES` in the aggregator module. Unsupported countries trigger a diagnostic error (not silent empty) that surfaces as `BoardScrapeSummary.error` with JSearch as fallback. Autopilot skip/error reasons surface in the run step log via `scrape_diagnostics`.

**Behavior:**

- **Keyless:** Returns empty results (never crashes; logged as warning).
- **Configured-failed:** If a provider is configured (keys present) and fails, diagnostic error surfaces as `BoardScrapeSummary.error` (not silent empty); fallback to next provider.
- **Unsupported country (Adzuna only):** Pre-request guard rejects before HTTP invocation; error includes remedy (add JSearch key for global coverage).
- **Empty results:** Legitimate (do NOT trigger fallback to next provider; only errors do).
- **Cancellation:** Pre-fallback cancel signal skips the paid provider entirely.

**Aggregator location determinism (PR D, 2026-07-10):**

Location input policy is now visible via per-board summary notes. When a search input lacks explicit country but specifies a city/region, the aggregator may broaden to country-level results or guess a market (fallback when Adzuna is sparse).

- **Floor guard:** `ADZUNA_BROADEN_FLOOR = 3` at `aggregator/mod.rs:52`. Broaden is triggered only when Adzuna returns fewer than 3 results for a location query AND `!country_guessed` (i.e. the user didn't supply a country). Guessed-market fallback to JSearch happens only when guessed Adzuna also returns `< 3` results.
- **Note tokens:** `BoardScrapeSummary.note: Option<String>` records the location policy decision as a machine token:
  - `broadened:<cc>` — Adzuna results exist but sparse (`< 3`); broadened from city to country-level (e.g., "few local results in Berlin — showing Germany-wide").
  - `guessed-market:<cc>` — No explicit country provided; market was guessed and Adzuna returned >= 3 results (authoritative guess). Never emitted for sub-floor guesses that fall through to JSearch global fallback.
  - Only one note per run; guessed and broadened are mutually exclusive (guessed when `country_guessed=true`, broadened when `country_guessed=false`).
- **Frontend rendering:** Chips mapped via `BoardSummaryChips.tsx` `ChipTone 'note'` (informational blue); locale-keyed labels `jobs.boardSummary.note.{broadened, guessed}` with country name via `Intl.DisplayNames({type:'region'})` for user-friendly labels (en + de).
- **Wizard visibility:** Autopilot wizard shows inline "Country: <Name>" when `countryCode` is set, cleared on manual location edit (user may re-pick via the location-input autocompleter).
- **Source:** `scraping/types/mod.rs` (`on_note` side-channel + `report_note()`), `scraping/engine/mod.rs` (`BoardScrapeSummary.note` wiring), `boards/aggregator/mod.rs` (inject), `boards/aggregator/providers.rs` (Adzuna emit sites + `guessed_market_note` helper).

## Canonical location model & central filter (PR F, 2026-07-11)

Location input is now canonical: resolved once from user-supplied city/region/country/lat/lon/radius at search time; boards declare server-side support via `supports_location()`; the engine centrally filters results for boards that cannot honor location constraints.

**LocationSpec canonical model:**

- **Type:** `apps/desktop/src-tauri/src/scraping/types/mod.rs` — struct with optional fields: `city`, `region`, `country_code`, `lat`, `lon`, `radius_km` (all present per Nominatim pick, but none required).
- **Accessor:** `BoardSearchInput::location_spec()` (not a stored field) — assembles the spec from existing input fields (`location` string + loose geo fields). Zero literal churn; back-compat automatic (None spec ⇒ filter inert).

**Board supports_location() flag:**

- **Trait method:** `Scraper::supports_location()` — returns true ONLY for boards with server-side location consumption.
- **Truthful catalog:** Only **3 boards** read location server-side:
  - **Aggregator** (Adzuna `where` param + country-scoped market routing; JSearch fallback)
  - **LinkedIn** (`geoId` typeahead + `distance` radius)
  - **Arbeitsagentur** (`wo` param)
- **20 non-supporting boards:** Remote feeds (remotive/remoteok/wwr), regional feeds (berlinstartupjobs/germantechjobs), ycombinator/arbeitnow, themuse+comeet, all 11 company-slug ATS (greenhouse/lever/ashby/smartrecruiters/personio/recruitee/pinpoint/rippling/breezy/bamboohr/workable).
- **IPC contract:** `BoardCatalogEntry.supportsLocation?` boolean flag in `packages/shared/src/ipc/contracts/boards.ts` (flows to renderer picker).

**Central conservative filter:**

- **Module:** `apps/desktop/src-tauri/src/scraping/engine/location_filter.rs` — pure function `location_mismatch(posting, &LocationSpec) -> bool`.
- **Policy:** DROP a posting iff it has a concrete, non-remote location whose diacritic-folded form (ü/ö/ä→u/o/a variants) shares NO substring token (len≥3) with any folded-expanded requested needle (raw city/region tokens + curated exonym pairs like Munich/München). KEEP always: `extra.remote==true`; remote text marker (remote/wfh/etc.); empty/unknown location; no usable requested token (country-code-only is inert); diacritic spelling variant; curated exonym pair. **Known limitation:** exonym pairs outside the small curated table (`EXONYM_PAIRS`: Munich/München, Cologne/Köln, Nuremberg/Nürnberg) still drop — this is documented and tested, not silently "fixed" by folding alone.
- **Wiring:** Applied ONLY to `supports_location()==false` boards ONLY when a location was requested. Threaded into the per-item streaming cap counting (not downstream, so cap counts POST-filter matches) + applied once post-hoc to final Vec for consistency across manual-cache, autopilot-found-jobs, and per-board `count` signals.
- **Drop tracking:** `location-filtered:<n>` note token records the count (never raw location text — PII). Emitted UNCONDITIONALLY on every `supports_location()==false` board when location requested, even if n=0 (preserves honesty that location was NOT honored, whether or not anything was dropped this run).

**Frontend picker surface:**

- **LocationFilterNote component:** `apps/desktop/src/renderer/components/scrape/LocationFilterNote.tsx` — renders when selected boards include non-supporting ones + location is set. Lists by name the non-supporting boards in the selection (absent flag reads as unsupported per contract). Scoped to SELECTED boards (not all 20, to avoid noise).
- **Chip rendering:** `BoardSummaryChips.tsx` extends note token mapping with `location-filtered:<n>` handling — `n>0` shows pluralized "N off-location result(s) hidden", `n===0` shows plain "location filtered locally" marker.
- **I18n keys:** `jobs.locationFilterHint` (label row), `jobs.boardSummary.note.locationFiltered_one` / `_other` (count/marker), en+de parity verified by real `@ajh/translations` import test.

**Source pointers:**

- **LocationSpec + accessor:** `apps/desktop/src-tauri/src/scraping/types/mod.rs` + `BoardSearchInput::location_spec()` (call path)
- **Filter policy + tests:** `apps/desktop/src-tauri/src/scraping/engine/location_filter.rs`
- **Trait flag + catalog:** `apps/desktop/src-tauri/src/scraping/boards/mod.rs` (`Scraper::supports_location()`, `SCRAPERS` overrides)
- **Filter wiring + note emission:** `apps/desktop/src-tauri/src/scraping/engine/mod.rs` (stream gate + post-hoc filter + note unconditional gate)
- **Frontend picker + chip mapping:** `apps/desktop/src/renderer/components/scrape/LocationFilterNote.tsx` + `BoardSummaryChips.tsx` `noteDetail()`
- **Autopilot forwarding (scope):** `apps/desktop/src-tauri/src/autopilot_helpers/mod.rs` (comment documents that lat/lon/radius are NOT persisted on AutopilotTarget; location+country_code are forwarded, future follow-up needed for radius expansion)
- **IPC contract:** `packages/shared/src/ipc/contracts/boards.ts` (BoardCatalogEntry.supportsLocation)

## Trust assessment (PR 3, 2026-07-01)

Every finalized `JobPosting` carries a **ghost-job / trust signal** — a pure,
non-blocking enrichment ported from `santifer/career-ops`'s
`providers/_trust-validator.mjs` (MIT). V1 is **flag-only: enrich, never
drop** — a low score never removes a posting, it only lowers the level for a
renderer badge (separate frontend pass). No config/enabled toggle; always
computed.

- **Module:** `apps/desktop/src-tauri/src/scraping/trust/mod.rs` —
  `pub fn assess_trust(url: &str, company: &str) -> TrustAssessment` (pure,
  no I/O, no new deps — `reqwest::Url`, i.e. the `url` crate reqwest
  re-exports). All helpers `pub(crate)` for fixture testing.
- **Shape:** `TrustAssessment { score: u8, level: TrustLevel, flags: Vec<TrustFlag> }`.
  `score` starts at 100 and is only ever decreased, clamped `0..=100`.
  `level`: `>=90 High`, `>=60 Medium`, else `Low`. The renderer `TrustBadge` only displays for `'medium'` or `'low'`; `'high'` renders no badge (no badge = trusted, noise-free).
- **Flags** — four signals, each decrements `score` by a penalty amount (see
  `SUSPICIOUS_DOMAINS`, `ATS_ALLOWLIST`, and `finish()` in
  `apps/desktop/src-tauri/src/scraping/trust/mod.rs` for penalty magnitudes
  and thresholds):
  - `MissingApplyUrl` (early return) — `url` empty/whitespace.
  - `InvalidUrl` (early return) — `url` doesn't parse or scheme isn't `http(s)`.
  - `SuspiciousDomain` — host is a URL shortener (see `SUSPICIOUS_DOMAINS`
    constant).
  - `CompanyDomainMismatch` — `company` is non-empty, the host isn't on the
    ATS allowlist, and the host doesn't plausibly name the company (normalized
    slug or a ≥3-char word match).
- **ATS allowlist** — never raises `CompanyDomainMismatch`. See `ATS_ALLOWLIST`
  constant in `apps/desktop/src-tauri/src/scraping/trust/mod.rs`; includes the
  standard ATS platforms (Greenhouse, Lever, etc.) plus our 24 `SCRAPERS` boards
  where `JobPosting.url` is systematically the BOARD's own domain rather than the
  employer's, plus the Adzuna aggregator (whose redirect host is a constant
  `api.adzuna.com` — country code is a path segment, not a subdomain).
- **`company_matches_host` is an unanchored substring heuristic**, not
  label-boundary matching — see the doc comment in `trust/mod.rs` for the
  known both-direction trade-off (misses a brand-embedding phishing host like
  `amazon-careers.xyz`; a short/generic company word can over-match).
  Deliberately deferred for V1 since the flag is advisory/non-gating; anchor
  it if a future flow ever gates behavior on `level`.
- **Wiring — three call sites** (every other `JobPosting`/`FoundJob`
  construction site is untouched; `JobPosting.trust` is NOT a dedicated
  struct field, to avoid touching all ~23 board literals — it's attached into
  the existing `#[serde(flatten)] extra: HashMap<String, Value>` channel, the
  same one salary/remote-status metadata already uses):
  - `ScraperEngine::run_one`'s streaming wrapper
    (`apps/desktop/src-tauri/src/scraping/engine/mod.rs`) — the funnel every
    board's streamed item passes through before reaching either caller's
    `on_item` (manual scrape → `PostingsCache` + `job.stream`; Autopilot's
    _live_ scrape stream → `SCRAPE_ITEM`).
  - `scrape_url::resolve()` (`apps/desktop/src-tauri/src/scraping/scrape_url/mod.rs`)
    — the single-URL resolver shared by the `scrape_url`/`scrape_resolve_url`
    commands and the extension-bridge import (import doesn't persist `trust`
    — it only ever writes an `ApplicationMeta`, not the postings/`JobPosting`
    contract).
  - `commands::autopilot::autopilot_run`'s `postings → FoundJob` `.map()`
    (`apps/desktop/src-tauri/src/commands/autopilot.rs`) — the **persisted**
    `AutopilotFoundJob` record the badge UI actually reads is built from the
    board's separately-returned `Vec<JobPosting>`, not the streamed copy the
    engine wrapper attaches to, so it calls `assess_trust` directly (a pure,
    cheap call — no need to round-trip through `extra`). `FoundJob.trust` is
    `Option<TrustAssessment>` (unlike `JobPosting`'s always-`Some`): a run
    recorded before this field existed deserializes with `None`
    (`#[serde(default)]`), every run recorded from here on sets `Some(..)`.
- **Contract:** `packages/shared/src/types/index.ts` — `JobTrustAssessment`,
  `JobPosting.trust?`, and `AutopilotFoundJob.trust?`. None are
  Zod-schema-derived (like the rest of those two interfaces), so none are
  `gen:ipc`-generated; kept hand-in-sync with the Rust serde shapes, same as
  every other field on both interfaces.

## Scrape results persistence (PR #463)

Results now persist across navigation thanks to React Query + backend cache:

- **Backend:** `PostingsCache` in Rust is the source of truth (populated as streamed results arrive)
- **Frontend:** Throttled `invalidatePostings()` on `job.stream` event (~1 ms throttle; eager on first item of new search)
- **Hydration:** React Query re-fetches cache on component remount → results reappear

## Cross-source dedup (PR E, 2026-07-10)

When a job posting appears across multiple boards (e.g., a job scraped both from the Aggregator and a direct ATS board), the engine performs a single cross-source dedup pass to eliminate duplicates before the UI renders them.

**Canonical job key:**

Every job is keyed by its `canonical_job_key(url, title, company)` — defined in `apps/desktop/src-tauri/src/scraping/boards/common.rs`, mirrored in `apps/desktop/src/renderer/features/jobs/lib/canonical-job-key.ts` for renderer-side dedup. The key normalizes URLs (lowercase whole URL, strip `www.`, drop fragments, keep only board-identifying query params like `indeed.com`'s `jk`) or falls back to `{title.trim().lowercase()}\u{0001}{company.trim().lowercase()}` (U+0001 separator prevents title/company forgery). The Rust function `normalize_job_url` is the shared app-wide URL identity (also used by autopilot's merge and extension import); the TS mirror must maintain byte-equivalence to the Rust implementation to ensure the same duplicate pairs are identified both sides.

**Engine dedup pass:**

`scraping/engine/mod.rs`: `dedup_cross_source()` runs once per manual scrape after all boards are concatenated, before results are returned to the UI. Duplicates sharing a canonical key are collapsed down to one survivor; the survivor is the incumbent (first-seen job for that key), but its fields are field-level-upgraded (not wholesale replaced):

- `id`, `url`, `source`, `interactions` — never overwritten (incumbent identity kept; user-state is tied to `id`)
- `description` — upgraded when the challenger's is strictly longer (measured in UTF-8 bytes)
- `extra` (salary/remote/etc.) — unioned key-by-key, non-empty incumbent values win, keys the incumbent lacks are filled from the challenger

This policy prevents silent data loss (e.g., Adzuna's salary fields wouldn't be dropped if a direct board with no salary won purely on description length).

**Autopilot merge:**

`autopilot/mod.rs`: `merge_found_jobs()` dedupes the incoming batch intra-batch using the same canonical key before merging against existing persisted rows. The engine's cross-source pass runs upstream (the Autopilot path consumes the already-deduped vec directly), so this is defensive; intra-batch dedup follows the same field-level-upgrade pattern. Notification count reflects post-dedup genuinely-new jobs only.

**Renderer dedup pass:**

`features/jobs/lib/merge-postings.ts`: `mergePostings(display, livePostings, absorbed?)` now runs a second pass (after pass 1's same-id merge) keyed by `canonicalJobKey`. The survivor is the incumbent (first-seen row, persisted rows over streamed duplicates), field-level-upgraded (description byte-length, extra union), and has its user-state preserved. An optional `absorbed` out-param records which row ids were collapsed so the selection-reconciliation effect can re-point a stale `selectedId` to the survivor instead of falling back to `display[0]`.

This renderer pass reconciles manual-scrape live streams (which are not engine-deduped) with the completion count — visible rows now match the deduped unique total.

**Shared fixtures:**

The 5 Rust truth-table test cases (same URL across boards, url-less title+company, near-miss distinct, U+0001-unforgeable, empty/whitespace/dangerous-scheme fallback) are copied verbatim into the TS test as a drift guard — if either side's algorithm diverges, one of these tests fails.

**Source pointers:**

- Canonical key (Rust): `apps/desktop/src-tauri/src/scraping/boards/common.rs:canonical_job_key`
- Engine dedup: `apps/desktop/src-tauri/src/scraping/engine/mod.rs:dedup_cross_source`
- Autopilot merge: `apps/desktop/src-tauri/src/autopilot/mod.rs:merge_found_jobs` (delegates `merge_key` to the shared canonical key)
- Canonical key (TS mirror): `apps/desktop/src/renderer/features/jobs/lib/canonical-job-key.ts`
- Renderer dedup: `apps/desktop/src/renderer/features/jobs/lib/merge-postings.ts:mergePostings`

## Jobs-page diagnostics surface (PR C, 2026-07-10)

Per-board scrape outcomes are now visible via a shared `BoardSummaryChips` component, surfacing the sanitized failure reasons, skip reasons, partial-harvest signals, and success counts:

- **Header strip** (Jobs page) — persistent chip strip rendered in the pinned header when results are present, survives the auto-closing scrape form. Gated on `!scraping && filteredJobs.length > 0`. Cleared on new scrape start, `job.failed`, or clear-postings.
- **Empty state** (Jobs page) — same strip rendered below the zero-results message to explain per-board why nothing was found. Only owner of the zero-results explanation surface; header strip is never shown alongside it (mutual exclusivity).
- **Autopilot card** — renders persisted `lastRunSummaries` as chips when run is finished (`!running`). Needs-configuration variant (when `runStatus==='failed'` AND every summary skipped with none errored) renders a neutral `autopilot.badge.needsConfig` badge + HoverPopover hint, not a red failure state.
- **Chip rendering** — `BoardSummaryChips.tsx` (component + pure `sanitizeReason` sanitizer). Sanitizer redacts UNC paths, IPv4/IPv6, URLs, emails, credential patterns; caps input @1000 chars, output @200 chars. Chip detail display is further capped @60 chars (wraps with `whitespace-normal break-words`). Chip order by severity: error (red) > skipped (neutral) > truncated (amber) > note (blue, informational) > success (green). All-success all-green collapse when multiple boards and all succeeded. **Location notes** (`broadened:<cc>` / `guessed-market:<cc>`, added in PR D) are rendered with tone 'note' and i18n labels mapping the machine token to a user-friendly country-aware message. Source: `apps/desktop/src/renderer/components/scrape/BoardSummaryChips.tsx`.
- **I18n keys** — `jobs.boardSummary.{label, count_one, count_other, partial, allOk_one, allOk_other, skip.{needsLogin, needsCompany, needsKeys, other}, note.{broadened, guessed}}` + `autopilot.badge.{needsConfig, needsConfigHint, completedWithErrorsHint}` + `autopilot.wizard.target.countryResolved` (en + de).
- **Whole-scrape failures** — persisted as `lastFailureNote` (sanitized, same redactor) on `JobsPage`; rendered as a small `role="status"` line in header + forwarded to `JobsResults` empty state (never triple-explained when `missingAdzunaKeys` is the root cause).
- **Skip-toast folding** — the three sticky notification warnings (needs-login / needs-company / needs-keys) were removed; their signal is now persistent in the chip strip (still actionable via `BoardConnectChip` in the scrape form for login boards).

## Full-description resolution on detail-pane open

**Aggregator short snippets → full descriptions:** Adzuna search API returns snippets (~200–500 chars). Detail pane auto-fetches on open when aggregator source + description < 700 chars, following the redirect chain and re-dispatching named-board handlers on the final URL. If the resolved text is meaningfully longer, it replaces the snippet; otherwise the snippet floor is kept. Redirect following is IP-guarded per-hop (closes DNS-rebinding TOCTOU); 429/login-wall/error returns Ok(None) and the snippet is retained.

- **Resolver:** `apps/desktop/src-tauri/src/commands/scrape.rs: scrape_resolve_url(app, url)` — public command invoked by detail pane
- **Re-dispatch:** `apps/desktop/src-tauri/src/scraping/scrape_url/mod.rs: resolve()` — follows redirect and re-dispatches handlers per final URL
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
  - Workable: `apps/desktop/src-tauri/src/scraping/boards/workable/mod.rs`
- **Credentialed, single-company (no `companies[]` fan-out):**
  - Comeet: `apps/desktop/src-tauri/src/scraping/boards/comeet/mod.rs`
- **Keyword aggregators (client-side filter, no server-side search):**
  - The Muse: `apps/desktop/src-tauri/src/scraping/boards/themuse/mod.rs`
- **Simple HTTP remote-jobs boards (keyless, full descriptions, tag-based keyword search):**
  - Jobicy: `apps/desktop/src-tauri/src/scraping/boards/jobicy/mod.rs` — HTTP GET request to Jobicy's free API with `count`/`tag` query params; returns full inline job descriptions (no truncation, no detail-fetch wall); free-text `tag` parameter for keyword search; part of the SCRAPERS registry.
- **Aggregator:**
  - Registry: `apps/desktop/src-tauri/src/scraping/boards/aggregator/mod.rs`
  - Providers (Adzuna, JSearch, Jooble, Apify LinkedIn): `apps/desktop/src-tauri/src/scraping/boards/aggregator/providers.rs` (JobProvider trait + implementations)

## Error representability (PR A, 2026-07-10)

Board fetch errors are now representable end-to-end, distinguishing between "board is blocked/rotted/misconfigured" and "no jobs found".

- **`fetch_json` signature** — `apps/desktop/src-tauri/src/scraping/http/mod.rs`. Returns `AppResult<T>` (was `AppResult<Option<T>>`). Non-2xx responses → `Err(AppError::Provider("HTTP <status>"))` (preserves status code). Serde drift → `Err(AppError::Parse("response body did not match the expected schema"))` (serde details logged, never returned). 2xx-valid → `Ok(T)`. Empty payloads return `Ok(empty)` not `Ok(None)`.
- **`fetch_text` boards** — germantechjobs, wwr, berlinstartupjobs now propagate non-200 as errors instead of silent-empty.
- **Board error outcome** — All boards propagate fetch errors via `search()` → `Err`, which surfaces in `BoardScrapeSummary.error: Option<String>` (engine records it). Boards with multi-company fanout (lever, recruitee, smartrecruiters, etc.) track `successful_fetches` and `first_fetch_error`; when `successful_fetches == 0`, the error propagates (all-fail → board error, not `Ok(empty)`). Pagination boards (themuse, arbeitnow, arbeitsagentur) distinguish page-0 failure (error) from later-page failure (keep partial harvest).
- **Shared decision functions** — `apps/desktop/src-tauri/src/scraping/boards/common.rs` exports two pure fns: `ats_all_fetches_failed(board_id, successful_fetches, first_fetch_error) -> Option<String>` (returns error message only when all companies failed) and `should_propagate_page_error(collected_so_far: usize) -> bool` (propagate if nothing collected yet, else keep partial harvest). Wired into 10 ATS boards + 3 paginated boards; no production URL embedded (testable without wiremock).
- **Frontend:** Service hook `apps/desktop/src/renderer/services/use-boards/use-boards.ts` (scrape mutations)
- **Settings UI:** `apps/desktop/src/renderer/features/settings/components/preferences/AggregatorKeysSettings/index.tsx`
- **Detail pane:**
  - Resolve + merge gate: `apps/desktop/src/renderer/features/jobs/components/JobDetailPane/index.tsx` (on-open resolve if short snippet; keep-longer merge logic; calls `scrape_resolve_url` + `scrape_update_description` IPC)
  - Description formatting: `html_to_markdown()` in `apps/desktop/src-tauri/src/scraping/http/mod.rs` — converts HTML job descriptions to Markdown
  - Rendering: `JobDescription` component (`packages/ui/src/components/JobDescription/index.tsx`) renders Markdown via react-markdown with design-token-only styling
- **Rust commands:** `apps/desktop/src-tauri/src/commands/scrape.rs` (`scrape_resolve_url`, `scrape_update_description`)
- **PostingsCache mutation:** `apps/desktop/src-tauri/src/postings/mod.rs: update_description(job_id, text)` — in-place cache mutation; text-hash-keyed result/embedding caches auto-invalidate on changed job text

## Board hygiene (PR G, 2026-07-11)

Live verification of never-verified recon-ported boards and hardening against silent zeros.

**Live verification (curl, single polite requests, 2026-07-11):**

- **Themuse** — `GET /api/public/jobs?page=0` shape confirmed; `items_per_page=20` documented.
- **Breezy HR** — drift FIXED: `location.state` is object `{id,name}`, not string. Parser switched to per-row `rows_to_jobs` (matching rippling/workable idiom) so a single drifted row can't zero the board. Added recovery from `all rows failed to parse` state (`raw_row_count > 0 && postings.empty()` → records `first_fetch_error`, skips `successful_fetches` increment, treated as fetch failure not genuine zero).
- **BambooHR** — response shape confirmed; `state` IS a plain string (unlike breezy). No changes needed.
- **Rippling** — response shape confirmed; `workLocation` tolerant of per-row objects. No changes needed.
- **Pinpoint** — unverifiable: no public slug found (bespoke subdomains). Parser kept as reconnaissance; drift/wrong-slug covered by existing error plumbing.
- **Comeet** — hidden from picker via `listed()→false`; code kept, dispatchable via API. Credentials remain configurable in Settings (no removal to avoid orphaning user data).

**LinkedIn soft-block detection:**

A 200 page-0 response with zero job cards is never genuine empty (verified: nonsense-keyword query still returns 10 padded cards). Chosen discriminator: `page0_is_soft_block(card_count)==card_count==0`. Returns board Err (`"LinkedIn returned no job cards — may be rate-limiting/require login/changed layout; results unavailable"`). Also: added country-biased cached `select_geo_id` (in-process cache keyed `(query, country)`, successes only) + `country_aliases` map for ambiguous names (e.g. `"be"→"Belgium"`).

**Rejected-slug surfacing (ATS boards):**

New `ats_finish_search(signal, out, board_id, successful_fetches, rejected_slugs, first_fetch_error)` in `boards/common.rs` centralizes "cancellation wins, else delegate to `ats_board_failure`"; wired into 7 ATS boards (bamboohr, breezy, pinpoint, rippling, workable, recruitee, personio). All-rejected → distinct error: `"all N company slug(s) invalid for {board} — check the company names in the jobs search form"`. Partial-reject (some fetched, some invalid) → log-only (chip mapping deferred).

**Source pointers:**

- Breezy row-drift recovery: `apps/desktop/src-tauri/src/scraping/boards/breezy/mod.rs` (capture `raw_row_count`, check emptiness post-`rows_to_jobs`)
- LinkedIn soft-block detector: `apps/desktop/src-tauri/src/scraping/linkedin/api_client/mod.rs` (`page0_is_soft_block`)
- LinkedIn geoId cache: `apps/desktop/src-tauri/src/scraping/boards/linkedin/mod.rs` (`GEO_ID_CACHE`, `select_geo_id`, `country_aliases`)
- Rejected-slug finalization: `apps/desktop/src-tauri/src/scraping/boards/common.rs` (`ats_finish_search`, `ats_all_slugs_invalid_message`)
- Tests: breezy `rows_to_jobs_all_rows_undeserializable_returns_empty` · linkedin `page0_is_soft_block`, `select_geo_id` · common `finish_search_*` truth table + personio inline cancel-after-reject seam

## Partial-failure notes (PR H, 2026-07-11)

When a board achieves partial success (some companies reached, some invalid slugs; some rows parsed, some dropped), fixed note tokens surface the partial outcome:

- **`slugs-invalid:<n>`** — `<n>` = count of company slugs rejected by SSRF/DNS-label validator. Emitted ONLY when `successful_fetches > 0` (at least one company reached) and some slugs were invalid. All-invalid case is the error instead (`ats_finish_search` Err). All 7 ATS slug-validating boards: bamboohr, breezy, pinpoint, rippling, workable, recruitee, personio.
- **`rows-dropped:<n>`** — `<n>` = total rows dropped by per-row deserialize across successful companies (partial parse failure, not fetch failure). Breezy, rippling, workable only (the 3 ATS boards with per-row parsing; bamboohr, pinpoint, recruitee deserialize atomically and pass `rows_dropped=0`).
- **Token emission** — via `ctx.report_note` (PR D side-channel), gated on `!ctx.signal.is_cancelled()`. At most ONE token per board per run; `slugs-invalid` wins when both apply (precedence order below).
- **Frontend rendering** — `BoardSummaryChips.tsx` maps both tokens: numeric gate `n > 0` (strict; these tokens only emitted for n>0), tone `processing` (informational blue). No precedence change within the chip severity order (error > skipped > truncated > note > success); both are `note` tone.
- **I18n keys** — `jobs.boardSummary.note.slugsInvalid` (en "{{count}} company name(s) invalid", de "{{count}} Firmenname(n) ungültig") and `jobs.boardSummary.note.rowsDropped` (en "{{count}} row(s) unreadable — board format may have changed", de "{{count}} Zeile(n) unlesbar — Board-Format evtl. geändert"). Pluralized via i18next `_one`/`_other`.

**Note precedence table (all 4 note types):**

| note token                                     | condition                                 | winner            | tone        | example                        |
| ---------------------------------------------- | ----------------------------------------- | ----------------- | ----------- | ------------------------------ |
| board-native (`slugs-invalid`, `rows-dropped`) | present                                   | board-native wins | note (blue) | 3 invalid company slugs        |
| `location-filtered`                            | non-supporting board + location requested | fallback          | note (blue) | 5 off-location results hidden  |
| `broadened`/`guessed`                          | aggregator location heuristic             | aggregator-only   | note (blue) | broadened from city to country |
| error                                          | fatal (all-fail, all-reject)              | error not note    | error (red) | all hosts failed               |

**Implementation:** `location-filtered` uses `note.get_or_insert_with()` (fills empty slot only). `ats_partial_note(successful_fetches, rejected_slugs, rows_dropped)` returns `Option<String>` (None for clean runs) via sequential if checks: `successful_fetches==0` → None (all-fail is an error); `rejected_slugs>0` → `"slugs-invalid:{n}"` (preferred, wins); else `rows_dropped>0` → `"rows-dropped:{n}"`. Source: `apps/desktop/src-tauri/src/scraping/boards/common.rs:210-222` + `scraping/engine/mod.rs:791-800`.

## Job-search trust program — COMPLETE (PRs A–H, 2026-07-10/11)

**Program status:** All 8 PRs shipped (#597–#604). Audit root causes (retry gap, run-guard gap, snippet-score honesty, personio semantics, note precedence, partial-failure visibility, stable sort, provisional-score rendering) are resolved.

**Fast-follow work items (deferred, not blocking):**

1. **`fetches-failed:<n>` token** — partial fetch-error runs (some companies 404/5xx while others succeed on the SAME board) stay log-only; a dedicated note token was deferred as an explicit fast-follow for the next scraping PR. Not required for PR H; tracked for context.
2. **LinkedIn geoId via fetch_json** — routing LinkedIn's geographic typeahead through `fetch_json` (for UA-override + size-cap + rate-limit parity) requires teaching `fetch_text` to let a caller UA header OVERRIDE the default. Security LOW; deferred pending concrete use case.
3. **LinkedIn soft-block telemetry** — verify the 200-zero-cards soft-block detector against real LinkedIn changes; currently unverified post-launch (unlike the verified 200-zero-cards claim in the code comments).
4. **Per-posting truncation signal** — forwarding a per-posting `truncated` flag from scrapers (for JSearch vs Adzuna distinction in Autopilot) enables smarter provisional-score derivation; currently the flag keys on `source=="aggregator"` (conservative, flags JSearch as provisional). Deferred to avoid a schema change in Scope A.
5. **Thundering-herd jitter** — add random jitter to `RETRY_BACKOFF` and daily-schedule retry delays to prevent multiple Autopilots retry-firing in lock-step; currently both retry at exactly `12min` / `00:00 UTC`. Low priority (most users have 1 Autopilot).

## See also

- `docs/SCRAPING_ENDPOINTS.md` — verified external endpoint reconnaissance (24 active boards, `aggregator` counted once among them; retired boards noted)
- `docs/knowledge/domain-model.md` — brief mention of `Scraper` trait + catalog
- `docs/ARCHITECTURE.md` — high-level diagram of scraping + IPC boundary
