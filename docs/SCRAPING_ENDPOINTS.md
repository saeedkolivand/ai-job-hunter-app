# Scraping Endpoints — Job Board Reconnaissance

> **SNAPSHOT — 2026-06-21. This document is NOT auto-updated.**
>
> **What this document is:** The URLs, query params, response shapes, selectors, and anti-bot notes documented here are an **external, web-verified snapshot of the third-party job sites** — they describe the targets our scrapers must hit, not our own code. They were verified by live inspection of the public sites, not extracted from this repository. Several of our scrapers are stale or broken relative to the current live endpoints; capturing the verified external truth is the whole purpose of this doc.
>
> **What lives in source:** Live constants that exist in our code (URL strings, selector literals, field mappings) are owned by the Rust source files cited in each section's **Source** line. Those are authoritative for what the scraper currently does; this doc is authoritative for what the external site currently exposes. Re-verify external values before relying on them — job sites change markup, endpoints, and anti-bot measures frequently.
>
> Obey each site's Terms of Service; this application is a personal job-hunting tool, not a bulk aggregator or reseller. See [`SECURITY.md`](../SECURITY.md) for the project Responsible Use policy.

---

## Summary table

Active scrapers: **23 boards** (registry as of 2026-07-02). Five boards were retired as direct scrapers and are now covered via the Adzuna/JSearch aggregator (see "Retired — now via aggregator" section below).

| Board               | Mode                                | Status            | Confidence | Verified endpoint                                                                 | Notes                                                 |
| ------------------- | ----------------------------------- | ----------------- | ---------- | --------------------------------------------------------------------------------- | ----------------------------------------------------- |
| **Aggregator**      | **HTTP (provider registry)**        | **✅ works**      | **high**   | **Adzuna (primary) / JSearch (paid fallback)**                                    | **Bring-your-own-key; keyless = empty**               |
| LinkedIn            | HTTP (guest)                        | ✅ works          | high       | `https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search`          |                                                       |
| YCombinator (HN)    | HTTP (JSON API)                     | ✅ works          | high       | `https://hacker-news.firebaseio.com/v0/jobstories.json`                           |                                                       |
| Remotive            | HTTP (JSON API)                     | ✅ works          | high       | `https://remotive.com/api/remote-jobs`                                            |                                                       |
| RemoteOK            | HTTP (JSON API)                     | ✅ works          | high       | `https://remoteok.com/api`                                                        |                                                       |
| WWR                 | HTTP (RSS)                          | ✅ works          | high       | `https://weworkremotely.com/remote-jobs.rss`                                      |                                                       |
| Arbeitnow           | HTTP (JSON API)                     | ✅ works          | high       | `https://www.arbeitnow.com/api/job-board-api`                                     |                                                       |
| Berlin Startup Jobs | HTTP (RSS)                          | ✅ works          | high       | `https://berlinstartupjobs.com/feed/`                                             |                                                       |
| German Tech Jobs    | HTTP (XML)                          | ✅ works          | high       | `https://germantechjobs.de/job_feed.xml` (working)                                |                                                       |
| **Greenhouse**      | **HTTP (JSON API, company-scoped)** | **✅ works**      | **high**   | **`https://boards-api.greenhouse.io/v1/boards/{slug}/jobs`**                      | **Requires company slug**                             |
| **Lever**           | **HTTP (JSON API, company-scoped)** | **✅ works**      | **high**   | **`https://api.lever.co/v0/postings/{company}?mode=json`**                        | **Requires company slug**                             |
| **SmartRecruiters** | **HTTP (JSON API, company-scoped)** | **✅ works**      | **high**   | **`https://api.smartrecruiters.com/v1/companies/{id}/postings`**                  | **Requires company + supports keyword**               |
| **Personio**        | **HTTP (XML feed, company-scoped)** | **✅ works**      | **high**   | **`https://{company}.jobs.personio.de/xml`**                                      | **Requires company slug**                             |
| **Recruitee**       | **HTTP (JSON API, company-scoped)** | **✅ works**      | **high**   | **`https://{company}.recruitee.com/api/offers/`**                                 | **Requires company slug**                             |
| **Ashby**           | **HTTP (JSON API, company-scoped)** | **✅ works**      | **high**   | **`https://api.ashbyhq.com/posting-api/job-board/{clientname}`**                  | **Requires company slug**                             |
| Arbeitsagentur      | HTTP (JSON API)                     | ✅ works          | high       | `https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4/jobs`            |                                                       |
| **Pinpoint**        | **HTTP (JSON API, company-scoped)** | **⚠️ unverified** | **medium** | **`https://{slug}.pinpointhq.com/postings.json`**                                 | **Requires company slug**                             |
| **Rippling**        | **HTTP (JSON API, company-scoped)** | **⚠️ unverified** | **medium** | **`https://api.rippling.com/platform/api/ats/v1/board/{slug}/jobs`**              | **Requires company slug**                             |
| **Breezy HR**       | **HTTP (JSON API, company-scoped)** | **⚠️ unverified** | **medium** | **`https://{slug}.breezy.hr/json`**                                               | **Requires company slug**                             |
| **BambooHR**        | **HTTP (JSON API, company-scoped)** | **⚠️ unverified** | **medium** | **`https://{slug}.bamboohr.com/careers/list`**                                    | **Requires company slug**                             |
| **The Muse**        | **HTTP (JSON API, browse feed)**    | **⚠️ unverified** | **medium** | **`https://www.themuse.com/api/public/jobs`**                                     | **No keyword param; client-side filter**              |
| **Workable**        | **HTTP (JSON API, company-scoped)** | **✅ works**      | **high**   | **`https://apply.workable.com/api/v1/widget/accounts/{slug}?details=true`**       | **Requires company slug**                             |
| **Comeet**          | **HTTP (JSON API, credentialed)**   | **⚠️ unverified** | **low**    | **`https://www.comeet.co/careers-api/2.0/company/{uid}/positions?token={token}`** | **Requires company UID + API token; keyless = empty** |

**Retired — now via aggregator** (removed from `SCRAPERS` registry; results available via Adzuna/JSearch):

| Board     | Former mode              | Retirement reason                                                                 |
| --------- | ------------------------ | --------------------------------------------------------------------------------- |
| Glassdoor | Browser (login required) | Cloudflare Bot Management blocks all headless sessions; aggregator covers         |
| Indeed    | HTTP (login required)    | hCaptcha + Cloudflare at volume; login cookies unreliable; aggregator covers      |
| Xing      | HTTP (login required)    | Cloudflare tightening since mid-2025; login cookies unreliable; aggregator covers |
| StepStone | HTTP (ld+json)           | Cloudflare/bot filter causes consistent 403/timeouts; aggregator covers           |
| Workday   | HTTP (JSON POST)         | 422 on all programmatic POSTs (Cloudflare Bot Management); aggregator covers      |

**Status legend:** ✅ works · ⚠️ fragile (anti-bot or stale selectors; works but needs monitoring) · ❌ broken (endpoint returns errors or scraper logic is stale and requires code changes)

---

## Per-board details

### Aggregator (Adzuna + JSearch)

**Source:** `apps/desktop/src-tauri/src/scraping/boards/aggregator/` (provider registry pattern)

#### Current scraper

- **Mode:** HTTP via provider registry (pluggable adapters; Adzuna primary, JSearch paid fallback)
- **Configuration:** Bring-your-own-key — credentials stored in OS keyring (`ai:adzuna-app-id`, `ai:adzuna-app-key`, `ai:jsearch-key`)
- **Behavior:** Returns empty results if no key is configured (never crashes); fallback to JSearch only fires on Adzuna error, not on legitimately empty results

#### Verified endpoints (2026-06-21)

**Adzuna** (free tier, primary)

- **Search URL:** `https://api.adzuna.com/v1/api/jobs/{country_code}/search/1`
- **Query params:** `app_id`, `app_key` (from keyring), `what` (keyword), `where` (location), `limit` (default 50)
- **Pagination:** Via `pageNumber`; API returns `pageNumber` in response
- **Auth required:** App ID + App key (user-supplied, obtained from https://developer.adzuna.com); stored encrypted in OS keyring
- **Response format:** JSON `{ results: [...], pageNumber: N }`
- **Key data fields:**
  - Stable ID: `result.id`
  - Title: `result.title`
  - Company: `result.company.display_name`
  - Location: `result.location.display_name`
  - URL: `result.redirect_url`
  - Posted: `result.created` (ISO 8601)
  - Description: `result.description` (HTML, stripped)
  - Salary: `result.salary_min`, `result.salary_max`, `result.salary_currency`
- **Anti-bot:** None; standard rate limits (API docs: reasonable use); no Cloudflare
- **Secrets guard:** App ID + App key stripped from HTTP logs; only scheme://host/path logged

**JSearch** (paid tier, fallback only on Adzuna errors)

- **Search URL:** `https://jskills-api.api.jsearch.io/v2/jobs-search`
- **Query params:** `api_key` (from keyring), `query`, `employment_type`, `job_is_remote`, `page`
- **Pagination:** `page` (1-indexed)
- **Auth required:** API key (user-supplied, obtained from https://api.jsearch.io); stored encrypted in OS keyring
- **Response format:** JSON `{ data: [...] }`
- **Fallback trigger:** Adzuna returns an error (network fault, auth error, etc.); will NOT fallback on empty results (empty results are legitimate)
- **Cancellation guard:** If a cancel signal arrives before JSearch fires, the fallback is skipped entirely

#### Recommendation

Bring-your-own-key design: users with no key see empty results (not an error). Settings → Jobs shows a dedicated field to enter/update Adzuna app keys with a link to https://developer.adzuna.com. Removed the OnceLock cache so a newly-saved key takes effect on the next search without app restart. On-save errors are surfaced as generic i18n strings, not raw backend text.

---

### Retired boards (Glassdoor, Indeed, Xing, StepStone, Workday)

These five boards were retired as direct scrapers in 2026-06-21 (ADR-026). Their Rust modules (`scraping/boards/{glassdoor,indeed,xing,stepstone,workday}/`) have been deleted and they are removed from the `SCRAPERS` registry (registry count: 21 → 16). Coverage for these boards is now provided by the **Aggregator** (Adzuna/JSearch) — see the Aggregator section above.

**Why retired:** All five boards returned empty results or errors in production due to anti-bot defences (Cloudflare Bot Management, hCaptcha, JS fingerprinting). Self-scraping was a losing maintenance battle with no reliable fix short of residential proxies or managed actors, neither of which fit the local-first, bring-your-own-key model.

**What is deliberately KEPT (dormant):**

- `scraping/scrape_url/mod.rs` — `resolve()`, `try_workday()`, `canonical_job_url()` (Indeed URL resolver): used by the browser extension single-job import flow; kept because the import resolvers are pure URL transforms, not authenticated scrape loops.
- `scraping/board_login/` and `credentials/` machinery: dormant (no active scrapers need them for these boards, but the infrastructure supports future use).
- `commands/boards.rs` `boards_list()`: trimmed to `["linkedin"]` — indeed/xing/glassdoor were removed because their in-app login fed nothing after scraping removal.
- `CredentialSetSchema` / `CredentialBoardSchema` in shared schemas: untouched (dormant).
- Privacy "clear all" (`privacy_reset_app`): still disconnects the retired boards to wipe any lingering sessions from before the migration.

**Endpoint reconnaissance notes (archived):** The verified endpoint data (selectors, query params, field mappings) captured in the previous version of this document was accurate as of 2026-06-20. It is no longer maintained here since these boards have no active scraper. Refer to the ADR-026 for the retirement rationale.

---

### LinkedIn

**Source:** `apps/desktop/src-tauri/src/scraping/linkedin/api_client/mod.rs`

#### Current scraper

- **URL:** `https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search`
- **Mode:** HTTP (reqwest with optional session cookies)
- **Selectors:** `li` (card), `a.base-card__full-link`, `[data-entity-urn]`, `.base-search-card__title`, `.base-search-card__subtitle`, `.job-search-card__location`, `time[datetime]`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search`
- **Query params:** `keywords` (required), `location` (free-text), `start` (0-indexed, step 25), `geoId`, `distance`, `f_JT` (job type), `f_WT` (work type: 1=on-site/2=remote/3=hybrid), `f_E` (experience), `f_TPR` (time posted), `f_EA` (easy apply), `sortBy`
- **Pagination:** `start` increments by 25; returns 10 jobs per call
- **Auth required:** No auth for guest endpoint; optional `li_at` + `JSESSIONID` cookies for higher limits
- **Response format:** HTML `<li>` job cards (gzip compressed); NOT JSON
- **Key data fields:**
  - Stable ID: `data-entity-urn` attribute (split on `:` for numeric ID)
  - Title: `h3.base-search-card__title` (fallback `.job-card-container__title`)
  - Company: `h4.base-search-card__subtitle`
  - Location: `span.job-search-card__location`
  - Posted date: `time[datetime]` attribute (ISO date); fallback to relative text
  - URL: `a.base-card__full-link` href (strip query params)
  - Description: empty at listing stage; requires separate job-detail fetch
  - Salary: sometimes present on cards; not currently extracted
- **Anti-bot:** Rate limiting. Repo implements: internal `RateLimiter`, jittered delays (300–600 ms between pages, 500–1000 ms between pagination), geoId soft-block detection with retry-without-geoId fallback, User-Agent spoofing. No CAPTCHA on guest endpoint observed.

#### Recommendation

Confirmed solid. Monitor HTML class name drift every 2–3 months. Cache geoId lookups from the typeahead endpoint.

---

### YCombinator / Hacker News

**Source:** `apps/desktop/src-tauri/src/scraping/boards/ycombinator/`

#### Current scraper

- **URL:** `https://hacker-news.firebaseio.com/v0/jobstories.json`; `https://hacker-news.firebaseio.com/v0/item/{id}.json`
- **Mode:** HTTP (reqwest)
- **Fields:** `id`, `type='job'`, `title`, `url`, `text`, `by`, `time` (Unix seconds)

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://hacker-news.firebaseio.com/v0/jobstories.json` (returns ~31 IDs)
- **Query params:** None; client-side keyword filter on `title` + `text`
- **Pagination:** None; single array; scraper over-fetches `limit*3`, caps at `limit`
- **Auth required:** None; public API; CORS: `*`
- **Response format:** JSON — `jobstories.json`: `array<i64>`; `item/{id}.json`: `{by, id, score, time, title, type, url, text}`
- **Key data fields:** title, company (via `parse_company` from title or `by`), url, id (`ycombinator:{id}`), `posted_at` (`time * 1000`), description (`text`)
- **Anti-bot:** None; no rate limits observed (5 rapid GETs: 200 OK)
- **Note:** `workatastartup.com/api/*` deprecated, returns 404. Firebase endpoint is canonical and stable 5+ years.

#### Recommendation

No changes needed. Stable and fully functional.

---

### Remotive

**Source:** `apps/desktop/src-tauri/src/scraping/boards/remotive/mod.rs`

#### Current scraper

- **URL:** `https://remotive.com/api/remote-jobs`
- **Mode:** HTTP (reqwest JSON)
- **Fields:** `id`, `url`, `title`, `company_name`, `candidate_required_location`, `publication_date`, `salary`, `tags`, `description` (HTML)

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://remotive.com/api/remote-jobs`
- **Query params:** `search` (keyword, URL-encoded), `category` (slug), `company_name`, `limit`
- **Pagination:** None; returns all matching results in one response
- **Auth required:** None; public API
- **Response format:** JSON — top-level keys: `job-count`, `total-job-count`, `jobs[]`; each job is a flat object with 13 fields
- **Key data fields:**
  - Stable ID: `job.id` (integer)
  - Title: `job.title`
  - Company: `job.company_name`
  - Location: `job.candidate_required_location` (optional)
  - URL: `job.url`
  - Posted: `job.publication_date` (ISO 8601; note: 24-hour delay vs actual post time)
  - Description: `job.description` (HTML, stripped by scraper)
  - Requirements: `job.tags` (array — mapped to `requirements`)
  - Salary: `job.salary` (string, e.g. `"$50k-70k/year"`) — NOT currently parsed by scraper
  - Also available: `job.job_type`, `job.category`, `job.company_logo`
- **Anti-bot:** Rate limit: >2×/minute → blocked. Remotive recommends max 4 requests/day. No Cloudflare/captcha.

#### Recommendation

No endpoint changes. Consider parsing `job.salary` string if salary filtering is needed. Field mapping is otherwise complete and correct.

---

### RemoteOK

**Source:** `apps/desktop/src-tauri/src/scraping/boards/remoteok/mod.rs`

#### Current scraper

- **URL:** `https://remoteok.com/api`
- **Mode:** HTTP (reqwest)
- **Fields:** `id`, `slug`, `position`, `company`, `location`, `tags[]`, `description`, `url`, `apply_url`, `date`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://remoteok.com/api` (also mirrors at `remoteok.io/api`)
- **Query params:** None; returns entire feed; keyword/location filtering is client-side
- **Pagination:** None; single JSON array; typically 500 KB–2 MB; scraper enforces 8 MB cap
- **Auth required:** None; CORS: `*`; no rate limiting observed
- **Response format:** JSON array of two variants: `RemoteOkItem::Job {}` and `RemoteOkItem::Legend { slug }` (first metadata row, skipped by scraper)
- **Key data fields:**
  - Title: `position`
  - Company: `company`
  - Location: `location` (optional)
  - URL: `url` (fallback: `apply_url`, then synthetic `remoteok.com/remote-jobs/{slug}`)
  - Stable ID: `id`
  - Posted: `date` (RFC3339)
  - Description: `description` (HTML)
  - Salary: `salary_min` / `salary_max` (integers) — NOT currently parsed by scraper
- **Anti-bot:** None detected; 100% reliability per FreePublicAPIs; CORS enabled

#### Recommendation

Add `salary_min`/`salary_max` parsing if salary is needed (fields exist in API, ignored in current scraper). Consider caching the full feed (1-hour TTL) to avoid re-fetching on every query.

---

### We Work Remotely (WWR)

**Source:** `apps/desktop/src-tauri/src/scraping/boards/wwr/`

#### Current scraper

- **URL:** `https://weworkremotely.com/remote-jobs.rss`
- **Mode:** HTTP (RSS XML)
- **Fields:** `<title>`, `<link>`, `<guid>`, `<pubDate>`, `<description>`, `<region>`, `<category>`, `<type>`, `<skills>`, `<expires_at>`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://weworkremotely.com/remote-jobs.rss` (main); category-specific: `https://weworkremotely.com/categories/remote-{category}-jobs.rss`
- **Query params:** None; RSS is category-based only; no keyword search in feed
- **Pagination:** None; ~50–100 recent items per feed; TTL=60 s
- **Auth required:** None; public RSS; JSON API `/api/v1/remote-jobs` requires Bearer token (employer-posting use only, not for search)
- **Response format:** RSS 2.0 XML with custom elements: `<region>`, `<country>`, `<state>`, `<skills>`, `<type>`, `<expires_at>`
- **Key data fields:**
  - Title format: `"Company: Job Title"` — company inferred by splitting on `": "` (splitn(2))
  - Location: `<region>` (e.g. `"Anywhere in the World"`, `"Berlin"`)
  - URL: `<link>`
  - Stable ID: `<guid>` (permalink)
  - Posted: `<pubDate>` (RFC 2822)
  - Description: `<description>` (HTML-encoded; contains salary as plain text — not parsed)
  - Type: `<type>` (Full-Time/Contract)
  - Skills: `<skills>` (comma-separated)
  - Expiry: `<expires_at>`
- **Anti-bot:** None; public RSS; no Cloudflare/captcha/rate-limit headers observed

#### Recommendation

Confirmed solid. Consider subscribing to category-specific RSS feeds for better relevance. Parse `<expires_at>` to filter stale listings. Extract salary from `<description>` HTML via regex if needed.

---

### Arbeitnow

**Source:** `apps/desktop/src-tauri/src/scraping/boards/arbeitnow/mod.rs`

#### Current scraper

- **URL:** `https://www.arbeitnow.com/api/job-board-api?page={page}`
- **Mode:** HTTP (reqwest, no browser)
- **Fields:** `data[].{slug, company_name, title, description, remote, url, tags, job_types, location, created_at}`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://www.arbeitnow.com/api/job-board-api`
- **Query params:** `page` (integer, default 1); optional `visa_sponsorship=true`; no keyword/location params
- **Pagination:** `?page={n}`; response includes `links.next` (optional); ~20 jobs/page
- **Auth required:** None; fully public
- **Response format:** JSON `{ data: [], links: { next?: url } }`
- **Key data fields:**
  - Title: `data[].title`
  - Company: `data[].company_name`
  - Location: `data[].location` (optional)
  - URL: `data[].url`
  - Stable ID: `data[].slug`
  - Posted: `data[].created_at` (Unix seconds; ×1000 for ms)
  - Description: `data[].description` (HTML, stripped)
  - Requirements/tags: `data[].tags` (array)
  - Remote: `data[].remote` (boolean)
  - Salary: ABSENT from API; embedded only in HTML description text
- **Anti-bot:** None; CORS enabled; no rate limits observed

#### Recommendation

Implementation is production-ready. `visa_sponsorship` filter not used but available. Client-side keyword filtering is unavoidable (API has no server-side search). Consider adding `date_filter` and `location` post-fetch filtering.

---

### Berlin Startup Jobs

**Source:** `apps/desktop/src-tauri/src/scraping/boards/berlinstartupjobs/`

#### Current scraper

- **URL:** `https://berlinstartupjobs.com/feed/`
- **Mode:** HTTP (reqwest + `feed_rs` RSS parser)
- **Fields:** `entry.title`, `entry.links[0].href`, `entry.content.body`, `entry.published`, `entry.categories[].term`, `entry.id`

#### Verified endpoint (2026-06-19)

- **Search URL:** `https://berlinstartupjobs.com/feed/`
- **Query params:** None; full feed, no keyword/location filtering at server
- **Pagination:** None; all items returned; descending by `pubDate`
- **Auth required:** None; public RSS
- **Response format:** RSS 2.0 XML (`dc:creator`, `post-id` custom element)
- **Key data fields:**
  - Title format: `"Job Title // Company Name"` (also `"Title at Company"`); company extracted by regex
  - Location: hardcoded to `"Berlin"` (not in feed)
  - URL: `<link>`
  - Stable ID: `<guid>` (or `<post-id>`)
  - Posted: `<pubDate>` (RFC 2822)
  - Description: `<description>` (HTML snippet, stripped)
  - Salary: NOT in feed
- **Anti-bot:** None

#### Recommendation

Title-split regex in repo (`' at (.+)$'`) handles `"Title at Company"` but not `"Title // Company"`. Update to `r'(?:\s+at\s+|//)(.+)$'` to handle both separators confirmed in live feed.

---

### German Tech Jobs

**Source:** `apps/desktop/src-tauri/src/scraping/boards/germantechjobs/`

#### Current scraper

- **URL:** `https://germantechjobs.de/job_feed.xml` (fixed; old `/rss` endpoint returns HTTP 403)
- **Mode:** HTTP (custom XML feed, non-RSS)
- **Parser:** Regex-per-block (`parse_feed()`) using module-level `LazyLock<Regex>` statics (mirrors Personio pattern); no longer uses `feed_rs` (the endpoint is a custom `<jobs><job>…</job></jobs>` schema, not RSS/Atom)

#### Verified endpoint (2026-06-20)

- **Working URL:** `https://germantechjobs.de/job_feed.xml` (HTTP 200, verified)
- **Alternate:** `https://germantechjobs.de/job_feed_stelleninserate_de.xml` (HTTP 200, verified)
- **Query params:** None; full feed; client-side keyword/location filtering
- **Pagination:** None; single monolithic feed (can reach ~10 MB)
- **Auth required:** None
- **Response format:** Custom XML (NOT RSS/Atom): `<jobs>` root containing `<job>` elements with tags: `<id>`, `<title>`, `<company>`, `<company-name>`, `<salary>`, `<location>`, `<city>`, `<region>`, `<country>`, `<postal_code>`, `<url>`, `<link>`, `<apply_url>`, `<pubdate>`, `<description>` (CDATA)
- **Key data fields:**
  - Title: `<title>` (e.g. `"Junior Java Developer (m/w/d)"`)
  - Company: `<company>` or `<company-name>` (prefer `<company-name>` if both present)
  - Salary: `<salary>` field present (e.g. `"50.000 - 65.000 €"`) — structured value, not embedded in HTML
  - Location: `<location>` (preferred), `<city>`, `<region>` (fallbacks)
  - URL: `<url>` or `<link>` or `<apply_url>` (tried in that order)
  - Stable ID: `<id>` (e.g. `"5fe377e2d2afdd001764761a-W25"`) or regex fallback to `<link>` value
  - Posted: `<pubdate>` (format: `DD.MM.YYYY`, parsed to timestamp)
  - Description: `<description>` CDATA (HTML-stripped)
- **Anti-bot:** None on working feed endpoints

#### Recommendation

Parser is now correct and handles the custom XML schema. Monitor the endpoint for availability; the old `/rss` endpoint may have been discontinued. No further code changes needed unless the feed schema drifts.

---

### Greenhouse

**Source:** `apps/desktop/src-tauri/src/scraping/boards/greenhouse/mod.rs`

#### Current scraper

- **URL:** `https://boards-api.greenhouse.io/v1/boards/{company_slug}/jobs?content=true`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out capped at 50 companies per scrape run
- **Fields:** `jobs[].{id, title, absolute_url, location.name, content, updated_at}`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://boards-api.greenhouse.io/v1/boards/{board_token}/jobs`
- **Query params:** `content=true` (include HTML description); `questions=true`; `pay_transparency=true` (include salary bands)
- **Pagination:** None; returns ALL jobs for the company in one response; `meta.total` in response
- **Auth required:** None; public API; `board_token` is the public company slug (e.g. `airbnb`, `stripe`)
- **Response format:** JSON `{ jobs: [...], meta: { total: N } }`
- **Key data fields:**
  - Stable ID: `job.id` (integer)
  - Title: `job.title`
  - Company: inferred from `board_token` (NOT in response)
  - Location: `job.location.name`
  - URL: `job.absolute_url`
  - Posted: `job.updated_at` (ISO 8601); also `job.first_published`
  - Description: `job.content` (HTML, HTML-entity encoded; requires entity decode + strip)
  - Salary: `job.pay_input_ranges[]` with `{currency, min_value, max_value}` when `?pay_transparency=true` — NOT currently fetched
- **Anti-bot:** None; intentionally public; no Cloudflare/captcha; CORS enabled
- **Live verification:** Airbnb (`/boards/airbnb/jobs`) returned EUR €71K–€84K salary for a Berlin role. Stripe (`/boards/stripe/jobs`) returned results with no salary data.

#### Recommendation

Add `?pay_transparency=true` and parse `job.pay_input_ranges` for salary. Use `job.first_published` for the canonical posted date in addition to `updated_at`.

---

### Lever

**Source:** `apps/desktop/src-tauri/src/scraping/boards/lever/mod.rs`

#### Current scraper

- **URL:** `https://api.lever.co/v0/postings/{company}?mode=json`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out (no cap); partial failure isolation per company
- **Fields:** `id`, `text` (title), `hostedUrl`, `categories.location`, `descriptionPlain`, `createdAt` (ms)

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://api.lever.co/v0/postings/{company}?mode=json`
- **Query params:** `mode=json` (required); `company` is URL path segment (slug from `jobs.lever.co/{company}`)
- **Pagination:** None; returns all postings for the company in one response
- **Auth required:** None; company slug is the only identifier
- **Response format:** JSON array of posting objects; `{"ok": false, "error": "…"}` on invalid slug
- **Key data fields:**
  - Stable ID: `id` (UUID)
  - Title: `text`
  - Company: inferred from URL slug (NOT in response)
  - Location: `categories.location`
  - URL: `hostedUrl`
  - Posted: `createdAt` (Unix ms)
  - Description: `descriptionPlain` (or `description` for HTML)
  - Salary: NOT present in API
- **Anti-bot:** None; HSTS + XSS headers only; no Cloudflare/captcha/rate-limit
- **Single-job detail:** `https://api.lever.co/v0/postings/{company}/{job_id}` (used by `scrape_url` resolver in `scraping/scrape_url/mod.rs`)

#### Recommendation

Confirmed solid. No changes to search endpoint. Salary unavailable from API.

---

---

### SmartRecruiters

**Source:** `apps/desktop/src-tauri/src/scraping/boards/smartrecruiters/`

#### Current scraper

- **URL:** `https://api.smartrecruiters.com/v1/companies/{companyIdentifier}/postings?limit=100` (or with `?q={keyword}` for keyword search)
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped with **optional keyword search via `?q` param** — scraper iterates `BoardSearchInput.companies[]` with per-company fan-out capped at 20 companies per scrape run; per-company results deduplicated
- **Fields:** `content[].{id, name, location, releasedDate}`; detail: `jobAd.sections` (HashMap)

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://api.smartrecruiters.com/v1/companies/{companyIdentifier}/postings`
- **Query params:** `limit` (max 100), `offset`, `q` (full-text search), `country`, `region`, `city`, `department`, `releasedAfter` (ISO 8601), `destination`, `locationType`, `language`, `customField.*`
- **Pagination:** `offset`/`limit`; response includes `totalFound`
- **Auth required:** None; public Posting API by design
- **Response format:** JSON `{ offset, limit, totalFound, content[]: Posting }`; detail: `{ jobAd: { sections: { [title]: { title, text } } } }`
- **Key data fields:**
  - Stable ID: `Posting.id` (string)
  - Title: `Posting.name`
  - Company: `Posting.company.name` (or from URL `companyIdentifier`)
  - Location: `Posting.location.{city, country, remote}`
  - URL: `https://jobs.smartrecruiters.com/{company}/{posting.id}`
  - Posted: `Posting.releasedDate` (ISO 8601)
  - Description: concatenate `jobAd.sections[*].title + strip_html(section.text)`
  - Salary: `DetailResp.compensation.{min, max, currency, period}` — NOT currently parsed by scraper
- **Anti-bot:** Rate limit 10 req/s (standard); Cloudflare + DataDome on application/registration flows only — NOT on Posting API. `X-RateLimit-Limit`/`X-RateLimit-Remaining` headers on every response.

#### Recommendation

Add `DetailResp.compensation` parsing for salary. For exhaustive scrapes add `offset` loop (current code caps at `limit=100`; fine for most companies). The 150–350 ms sleep between detail fetches is prudent.

---

### Personio

**Source:** `apps/desktop/src-tauri/src/scraping/boards/personio/mod.rs`

#### Current scraper

- **URL:** `https://{company}.jobs.personio.de/xml`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out (no cap); SSRF guard: validates company slug as DNS label; consistent job IDs across ingestion paths via `personio::make_job_id`
- **Method:** Regex-based XML parsing: `POSITION_RE`, `ID_RE`, `NAME_RE`, `OFFICE_RE`, `JOBDESC_BLOCK_RE` / `JOBDESC_SINGULAR_RE`, `DESC_RE`, `CREATED_RE`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://{company}.jobs.personio.de/xml` (primary) or `https://{company}.jobs.personio.com/xml` (identical content)
- **Query params:** `language=en|de|fr|es|nl|it|pt` (optional; defaults to German)
- **Pagination:** None; single response with all published positions
- **Auth required:** None; publicly accessible given the company subdomain
- **Response format:** XML — root `<workzag-jobs>` containing `<position>` children
- **Key data fields per `<position>`:**
  - Stable ID: `<id>` (integer)
  - Title: `<name>`
  - Location: `<office>` (primary); `<additionalOffices>` (secondary)
  - Division: `<subcompany>`, `<department>`, `<recruitingCategory>`
  - Employment type: `<employmentType>` (permanent/intern/trainee/freelance)
  - Seniority: `<seniority>` (entry-level/experienced/executive/student)
  - Schedule: `<schedule>` (full-time/part-time)
  - Posted: `<createdAt>` (ISO 8601)
  - Description: `<jobDescriptions>` → `<jobDescription>` → `<value>` (HTML/CDATA; multiple values per position)
  - Salary: ABSENT from public XML feed
- **Anti-bot:** None; CORS `access-control-allow-origin: *`
- **Canonical job URL constructed as:** `https://{company}.jobs.personio.de/job/{id}`

#### Recommendation

Implementation is production-ready. Scraper correctly handles both `<jobDescriptions>` (2025+ plural) and legacy `<jobDescription>` (singular) formats. Salary unavailable from public feed.

---

### Recruitee

**Source:** `apps/desktop/src-tauri/src/scraping/boards/recruitee/`

#### Current scraper

- **URL:** `https://{company}.recruitee.com/api/offers/`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out (no cap); SSRF guard: validates company slug as DNS label
- **Fields:** `offers[].{id, title, description, requirements, careers_url, city, country, remote, created_at, company_name, slug}`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://{company}.recruitee.com/api/offers/`
- **Query params:** Optional `?department={id}`, `?tag={id}` — not used by current scraper
- **Pagination:** None; complete `offers[]` array in one response
- **Auth required:** None; public Careers Site API for third-party embedding
- **Response format:** JSON `{ offers: [...] }`
- **Key data fields:**
  - Stable ID: `id` (integer) + `slug` (string)
  - Title: `title`
  - Company: `company_name` (optional; fallback to query param company)
  - Location: `city` + `country` (both optional; combined with `", "`)
  - URL: `careers_url`
  - Posted: `created_at` (RFC3339, parsed to timestamp ms)
  - Description: `description` (HTML, stripped)
  - Requirements: `requirements` (HTML, combined with description in scraper)
  - Remote: `remote` (boolean)
  - Salary: `salary` field EXISTS in API (`{min, max, currency, period}`) — NOT currently parsed
- **Anti-bot:** None; no rate limits documented

#### Recommendation

Implementation correct and working. Add `salary` object parsing if needed. Consider exposing `?department` filter in `BoardSearchInput`.

---

---

### Ashby

**Source:** `apps/desktop/src-tauri/src/scraping/boards/ashby/mod.rs`

#### Current scraper

- **URL:** `https://api.ashbyhq.com/posting-api/job-board/{clientname}?includeCompensation=true`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out capped at 50 companies per scrape run
- **Fields:** `jobs[].{id, title, location, isRemote, jobUrl, descriptionPlain, publishedAt, department, team}`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://api.ashbyhq.com/posting-api/job-board/{clientname}`
- **Query params:** `clientname` (path param, company's Ashby slug, e.g. `Ramp`, `Notion`); `includeCompensation=true` (includes salary data)
- **Pagination:** None; all published jobs returned in one response (verified: Ramp=112, Notion=148 jobs)
- **Auth required:** None; CORS `*`; publicly callable
- **Response format:** JSON `{ apiVersion: "1", jobs: [...] }`
- **Key data fields:**
  - Stable ID: `job.id` (UUID; stable)
  - Title: `job.title`
  - Company: from `clientname` (NOT in response)
  - Location: `job.location`
  - Remote: `job.isRemote` (boolean)
  - URL: `job.jobUrl` (`https://jobs.ashbyhq.com/{clientname}/{id}`)
  - Posted: `job.publishedAt` (RFC3339 with ms precision)
  - Description: `job.descriptionPlain` (string, may be null)
  - Salary (when `includeCompensation=true`): `job.compensation.compensationTierSummary` (string, e.g. `"$211.4K – $290.6K • Offers Equity"`); `job.compensation.scrapeableCompensationSalarySummary`; `job.compensation.compensationTiers[].components[].{minValue, maxValue, currencyCode}` — NOT currently deserialized by scraper
- **Anti-bot:** Cloudflare proxied but fully open (Cache-Control: public; no rate limits; no captcha)
- **Live verification:** Ramp salary `$211.4K – $290.6K` confirmed with `?includeCompensation=true`

#### Recommendation

Implementation confirmed working. `includeCompensation=true` is already in the URL but `compensation` is never deserialized — add `Compensation` struct to expose salary if needed.

---

### Bundesagentur für Arbeit (Arbeitsagentur)

**Source:** `apps/desktop/src-tauri/src/scraping/boards/arbeitsagentur/`

#### Current scraper

- **URL:** `https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4/jobs` (list); `/pc/v4/jobdetails/{hash}` (detail)
- **Mode:** HTTP (reqwest via `fetch_json`)
- **Fields:** `stellenangebote[].{refnr, titel, beruf, arbeitgeber, arbeitsort, aktuelleVeroeffentlichungsdatum, externeUrl, hashId}`

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4/jobs`
- **Query params:** `was` (keyword), `wo` (location), `page` (1-indexed), `size` (25 default); optional: `umkreis` (radius km), `arbeitszeit` (vz/tz/snw/ho/mj), `angebotsart`, `befristung`, `veroeffentlichtseit` (0–100 days)
- **Required header:** `X-API-Key: jobboerse-jobsuche` (shared public key used by official frontend)
- **Pagination:** `page` (1-indexed) + `size`; loop breaks when `items.len() < size`; max 10 pages in caller
- **Auth required:** No login; `X-API-Key` header required (publicly known)
- **Response format:** JSON `{ stellenangebote: [...], maxErgebnisse: N }`
- **Key data fields:**
  - Stable ID: `refnr` (prefixed `arbeitsagentur:{refnr}`)
  - Title: `titel` (or `beruf` as fallback)
  - Company: `arbeitgeber` (fallback `"Unbekannt"`)
  - Location: `arbeitsort.ort` + `arbeitsort.region` + `arbeitsort.land` (comma-joined)
  - URL: `externeUrl` (or constructed from `hashId`)
  - Posted: `aktuelleVeroeffentlichungsdatum` (RFC3339, to ms)
  - Description: from detail endpoint — `stellenangebotsBeschreibung` + `arbeitgeberdarstellung` (HTML-stripped)
  - Salary: NOT exposed in either list or detail API
- **Anti-bot:** Detail endpoint (`/jobdetails/{hash}`) returns 404 for non-browser clients (bot filter). Scraper handles gracefully: detail fetch is wrapped in `.ok().flatten()`; on 404 the job is still emitted using list-only data (description=None). No blocking on list endpoint. Rate limiting: 700 ms + random 0–500 ms per page.

#### Recommendation

Implementation is current and handles the detail-endpoint bot filter correctly. `size=25` is conservative (API supports up to 100); increase if throughput is a bottleneck. Watch `bundesAPI/jobsuche-api` for v4 deprecation notices.

---

### Pinpoint

**Source:** `apps/desktop/src-tauri/src/scraping/boards/pinpoint/mod.rs`

#### Current scraper

- **URL:** `https://{slug}.pinpointhq.com/postings.json`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out capped at 50 companies per scrape run; SSRF guard validates the slug as a DNS label
- **Fields:** `data[].{title, url, location.{name, city, province}}`

#### Unverified endpoint (ported 2026-07-01)

- **Search URL:** `https://{slug}.pinpointhq.com/postings.json` — reconnaissance-ported from `santifer/career-ops` (MIT), `providers/pinpoint.mjs`; **not re-verified live** in this environment (no outbound network access)
- **Response format (per career-ops):** JSON `{ data: [...] }`
- **Key data fields:** Title: `title`; Location: `location.name` else `[city, province].join(", ")`; URL: `url` (required `https:`, no host allowlist — display-only, doubles as the dedup key since the response has no stable id)
- **Anti-bot:** Unknown — not observed live

#### Recommendation

Re-verify the response shape against a live Pinpoint-hosted tenant before shipping. No stable id field in the documented shape — the scraper uses the posting URL as both id and dedup key.

---

### Rippling

**Source:** `apps/desktop/src-tauri/src/scraping/boards/rippling/mod.rs`

#### Current scraper

- **URL:** `https://api.rippling.com/platform/api/ats/v1/board/{slug}/jobs`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out capped at 50 companies per scrape run; slug is a URL path segment (percent-encoded), not a hostname
- **Fields:** top-level array `[{uuid, name, url, workLocation}]`

#### Unverified endpoint (ported 2026-07-01)

- **Search URL:** `https://api.rippling.com/platform/api/ats/v1/board/{slug}/jobs` — reconnaissance-ported from `santifer/career-ops` (MIT), `providers/rippling.mjs`; **not re-verified live** in this environment (no outbound network access)
- **Response format (per career-ops):** JSON top-level array of job objects
- **Key data fields:** Stable ID: `uuid`; Title: `name`; Location: `workLocation.label` (object form) or `workLocation` (bare string); URL: `url` — **host-locked to `ats.rippling.com`** (dropped if off-host or malformed, since the response is not otherwise host-constrained)
- **Anti-bot:** Unknown — not observed live

#### Recommendation

Re-verify the response shape against a live Rippling-hosted tenant, in particular whether `workLocation` is ever the bare-string form vs. always an object.

---

### Breezy HR

**Source:** `apps/desktop/src-tauri/src/scraping/boards/breezy/mod.rs`

#### Current scraper

- **URL:** `https://{slug}.breezy.hr/json`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out capped at 50 companies per scrape run; SSRF guard validates the slug as a DNS label
- **Fields:** top-level array `[{name, url, published_date, location.{name, city, state, country.name, is_remote}}]`

#### Unverified endpoint (ported 2026-07-01)

- **Search URL:** `https://{slug}.breezy.hr/json` — reconnaissance-ported from `santifer/career-ops` (MIT), `providers/breezy.mjs`; **not re-verified live** in this environment (no outbound network access)
- **Response format (per career-ops):** JSON top-level array of job objects
- **Key data fields:** Title: `name`; Location: `location.name` else `[city, state, country.name].join(", ")`, `", Remote"` appended when `is_remote` and not already present; Posted: `published_date` (RFC3339 or bare `YYYY-MM-DD`); URL: `url` (required `https:`, no host allowlist — display-only, doubles as the dedup key since the response has no stable id)
- **Anti-bot:** Unknown — not observed live

#### Recommendation

Re-verify the response shape against a live Breezy-hosted tenant before shipping. No stable id field in the documented shape — the scraper uses the posting URL as both id and dedup key.

---

### BambooHR

**Source:** `apps/desktop/src-tauri/src/scraping/boards/bamboohr/mod.rs`

#### Current scraper

- **URL:** `https://{slug}.bamboohr.com/careers/list`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out capped at 50 companies per scrape run; SSRF guard validates the slug as a DNS label
- **Fields:** `result[].{id, jobOpeningName, location.{city, state}, isRemote}`

#### Unverified endpoint (ported 2026-07-01)

- **Search URL:** `https://{slug}.bamboohr.com/careers/list` — reconnaissance-ported from `santifer/career-ops` (MIT), `providers/bamboohr.mjs`; **not re-verified live** in this environment (no outbound network access)
- **Response format (per career-ops):** JSON `{ result: [...] }`
- **Key data fields:** Stable ID: `id` (accepted as either a JSON number or string); Title: `jobOpeningName`; Location: `[city, state]` joined, `"Remote"` appended when `isRemote`; URL: constructed as `https://{slug}.bamboohr.com/careers/{id}` (not returned by the API); no description/posted-date field in the list endpoint
- **Anti-bot:** Unknown — not observed live

#### Recommendation

Re-verify the response shape against a live BambooHR-hosted tenant before shipping, in particular the actual JSON type of `id`.

---

### The Muse

**Source:** `apps/desktop/src-tauri/src/scraping/boards/themuse/mod.rs`

#### Current scraper

- **URL:** `https://www.themuse.com/api/public/jobs?page={n}` (0-indexed)
- **Mode:** HTTP (reqwest)
- **Scope:** Keyword aggregator — **no server-side keyword param** (only `category`/`level`/`location`/`company`/`page`); filtered client-side over title+company, same pattern as Remotive/RemoteOK/Arbeitnow; bounded to 5 pages (`BoardSearchInput.pages`, clamped)
- **Fields:** `results[].{name, refs.landing_page, company.name, locations[0].name}`

#### Unverified endpoint (ported 2026-07-01)

- **Search URL:** `https://www.themuse.com/api/public/jobs?page={n}` — reconnaissance-ported from `santifer/career-ops` (MIT), `providers/themuse.mjs`; **not re-verified live** in this environment (no outbound network access)
- **Response format (per career-ops):** JSON `{ results: [...], page: n, page_count: N }`
- **Key data fields:** Title: `name`; Company: `company.name` else `"The Muse"`; Location: `locations[0].name` else `""`; URL: `refs.landing_page` (required `^https?://`, no host allowlist — it points at each posting's own employer/ATS site, not a themuse.com page); no stable id field, so the URL doubles as the dedup id
- **Anti-bot:** Unknown — not observed live

#### Recommendation

Re-verify the response shape against the live API before shipping, in particular whether `page_count` is always present and whether `locations` can be empty vs. absent.

---

### Workable

**Source:** `apps/desktop/src-tauri/src/scraping/boards/workable/mod.rs`

#### Current scraper

- **URL:** `https://apply.workable.com/api/v1/widget/accounts/{slug}?details=true`
- **Mode:** HTTP (reqwest)
- **Scope:** Company-scoped — **requires a company slug** (no free-text keyword search); scraper iterates `BoardSearchInput.companies[]` with per-company fan-out capped at 50 companies per scrape run; per-company URL dedup; path-segment slug guard (DNS-label-shaped character set)
- **Fields:** `{ name, jobs[].{title, shortcode, url, published_on, created_at, country, city, state, telecommuting, description} }`

#### Verified endpoint (2026-07-02)

- **Search URL:** `https://apply.workable.com/api/v1/widget/accounts/{slug}?details=true` — **live-verified** (not career-ops-ported) against slug `careers-at-sleek` (55 jobs returned)
- **Response format:** JSON `{ name, description, jobs: [...] }`
- **Key data fields:** Title: `title`; Company: top-level `name` (falls back to the slug if absent); Location: `[city, state, country]` joined, `"Remote"` appended when `telecommuting`; Posted: `published_on` (fallback `created_at`); URL: `url`, **host-locked to `apply.workable.com`**; Stable ID: `shortcode`, namespaced `workable:{slug}:{shortcode}` (a bare shortcode is only unique within one Workable tenant)
- **Anti-bot:** None observed during live verification

#### Recommendation

Confirmed working via a real request. Each job row is deserialized independently (`rows_to_jobs`) so a single malformed row can't zero out a whole company's results.

---

### Comeet

**Source:** `apps/desktop/src-tauri/src/scraping/boards/comeet/mod.rs`

#### Current scraper

- **URL:** `https://www.comeet.co/careers-api/2.0/company/{uid}/positions?token={token}`
- **Mode:** HTTP (reqwest)
- **Scope:** Credentialed, single-company — `requires_company()` is `false` (the "company" is a fixed per-user credential, not a per-search `companies[]` input); credentials (company UID + API token) read from the OS keyring at scrape time, same `ai:`-namespaced convention as the Apify LinkedIn provider; keyless = empty (never an error)
- **Fields (career-ops spec):** `{ name, uid, url_comeet_hosted_page/url_active_page, location.{name,city,country}, time_updated }`

#### Unverified endpoint (2026-07-02)

- **Search URL:** `https://www.comeet.co/careers-api/2.0/company/{uid}/positions?token={token}` — confirmed **live** (returns HTTP 400 without real credentials) but the **response shape is unconfirmed**: built from the career-ops (MIT) field spec, not a captured live payload. Needs live-verification with a real company UID + token via the Settings UI.
- **Key data fields:** Title: `name`; URL: `url_comeet_hosted_page` (fallback `url_active_page`), **host-locked to `comeet.co`**; Location: `location.name` else `[city, country]` joined; Stable ID: `uid`; Posted: `time_updated` (Unix seconds, tolerates RFC3339 too); Company: a speculative per-row `company_name`/`company` field if present, else the configured company UID
- **Anti-bot:** Unknown — not observed live (endpoint requires real credentials to test past the 400)

#### Recommendation

Live-verify the full response shape (especially the company-name field, which is NOT in the career-ops spec handed to this implementation) once a real company UID + token is available via Settings → API Keys.

---

## Top priorities

### Needs monitoring

No active scrapers are currently blocked. The aggregator (Adzuna/JSearch) covers the five retired boards. Monitor Adzuna.de result depth for German-market roles — if thin, a dedicated German source may be warranted (tracked as a follow-up; not done in ADR-026).

### Status changes (fixed or improved)

| Board                          | Previous issue                                              | Current status                                                                    | Change                                                                                                              |
| ------------------------------ | ----------------------------------------------------------- | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| **German Tech Jobs**           | `/rss` returns HTTP 403; parser used `feed_rs`              | ✅ `/job_feed.xml` working; custom XML parser via regex blocks                    | Fixed: new endpoint + rewritten parser (non-RSS XML schema)                                                         |
| **Glassdoor**                  | Cloudflare blocks all headless sessions                     | Retired — coverage via Aggregator (ADR-026)                                       | Scraper removed from `SCRAPERS`; import URL resolver kept                                                           |
| **Indeed**                     | hCaptcha + Cloudflare at volume; login cookies unreliable   | Retired — coverage via Aggregator (ADR-026)                                       | Scraper removed; `canonical_job_url` URL resolver kept for extension import                                         |
| **Xing**                       | Cloudflare tightening; login cookies unreliable             | Retired — coverage via Aggregator (ADR-026)                                       | Scraper removed; no import resolver existed                                                                         |
| **StepStone**                  | Cloudflare/bot filter; consistent 403/timeouts from CI      | Retired — coverage via Aggregator (ADR-026)                                       | Scraper removed from `SCRAPERS`                                                                                     |
| **Workday**                    | 422 on all programmatic POSTs (Cloudflare Bot Management)   | Retired — coverage via Aggregator (ADR-026)                                       | Scraper removed; `try_workday()` URL resolver kept for extension import                                             |
| **Company-scoped ATS boards**  | Free-text keyword search unsupported; no company identifier | ✅ Company-scoped with per-company fan-out + SSRF hardening                       | Fixed: `BoardSearchInput.companies[]` + `requires_company()` declarations; skipped as `needs-company` if empty list |
| **Scrape results persistence** | Results lost when navigating away mid-scrape                | ✅ Results persist across navigation (backend `PostingsCache` is source of truth) | Fixed: throttled `invalidatePostings()` on `job.stream` event (React Query hydration on remount)                    |

### Confirmed solid (public APIs, no auth, no anti-bot)

Aggregator (Adzuna/JSearch, bring-your-own-key), LinkedIn (guest HTML), YCombinator (Firebase), Remotive, RemoteOK, We Work Remotely (RSS), Arbeitnow, Berlin Startup Jobs (RSS), Greenhouse, Lever, SmartRecruiters, Personio (XML), Recruitee, Ashby, Arbeitsagentur, Workable (live-verified 2026-07-02).

### Newly added — unverified (2026-07-01)

Pinpoint, Rippling, Breezy HR, BambooHR, The Muse were added from `santifer/career-ops` (MIT) reconnaissance without a live re-verification pass (no outbound network access in the authoring environment). Re-verify each endpoint's response shape against a real tenant/the live API before relying on it in production; see the per-board sections above.

### Newly added — unverified, credentialed (2026-07-02)

Comeet was added from the career-ops (MIT) field spec; the live endpoint 400s without real credentials, so the response shape is unconfirmed (see the Comeet section above). Workable, added the same day, IS live-verified (real request against a live tenant) and is listed under "Confirmed solid" above instead.

---

## Notes

### Skip States

The Rust scraping engine (`apps/desktop/src-tauri/src/scraping/engine/mod.rs`) reports scrape outcomes via `BoardScrapeSummary.skipped: 'needs-login' | 'needs-company'` (a closed union, not `Option<String>`):

- **`needs-login`** — board marked `AuthRequirement::Required` with no usable session (empty cookie jar or stale session via `board_login::{load_cookies, session_is_stale}`). Required boards are gated both at the UI (Start button disabled until boards are connected) and at the backend (redundant skip if the gate is somehow bypassed). Surfaced with a sign-in prompt.
- **`needs-company`** — company-scoped board (Greenhouse, Lever, Ashby, Personio, Recruitee, SmartRecruiters) with an empty or whitespace-only `companies` list in `BoardSearchInput`. The scrape form shows a "Companies" field only for boards that declare `requires_company()` (surfaced in the catalog metadata; no hardcoded list). Surfaced with a config hint (e.g., "Enter company slugs to scrape this board").

Each skip reason is surfaced separately in the scrape results UI with its own remediation message.

### chromiumoxide Warnings

`WS Invalid message: did not match any variant of untagged enum Message` warnings from chromiumoxide (v0.8.0+) are **benign**. They appear when the browser emits Chrome DevTools Protocol (CDP) events that the pinned chromiumoxide bindings don't model. The warning is cosmetic; RPC command dispatch is unaffected. This is a documented resilience improvement over chromiumoxide 0.7.0 (which could hard-panic on unknown messages). The warnings appear in logs but do not affect scraper correctness or completeness.

### Regex-based Parsing

Boards using custom XML feeds (German Tech Jobs, Personio) parse via module-level `LazyLock<Regex>` statics to compile regex patterns once per process. This pattern is preferred for low-volume feeds where `feed_rs` is not applicable (non-standard schemas). Per-tag regexes (one for `<id>`, one for `<title>`, etc.) are driven by tests via a `parse_feed(xml, scraper_id, now)` function, ensuring the real parsing path is tested, not a copied loop.
