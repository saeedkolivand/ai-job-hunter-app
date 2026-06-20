# Scraping Endpoints — Job Board Reconnaissance

> **SNAPSHOT — 2026-06-20. This document is NOT auto-updated.**
>
> Job sites change markup, endpoints, and anti-bot measures frequently. Re-verify every endpoint against the live site before relying on it. Obey each site's Terms of Service; this application is a personal job-hunting tool, not a bulk aggregator or reseller. See [`SECURITY.md`](../SECURITY.md) for the project Responsible Use policy.
>
> Where a repo source file already owns a constant (URL, selector, field path), that file is cited as the authority — do not copy the literal here; read the source.

---

## Summary table

| Board               | Mode                    | Status               | Confidence | Verified endpoint                                                        |
| ------------------- | ----------------------- | -------------------- | ---------- | ------------------------------------------------------------------------ |
| Glassdoor           | Browser (HTTP BFF)      | ❌ broken scraper    | high       | `POST /job-search-next/bff/jobSearchResultsQuery`                        |
| Indeed              | Browser (chromiumoxide) | ⚠️ fragile           | high       | `https://{domain}/jobs?q=…&l=…&start=…`                                  |
| Xing                | Browser (chromiumoxide) | ⚠️ fragile           | medium     | `https://www.xing.com/jobs/search?keywords=…`                            |
| LinkedIn            | HTTP (guest)            | ✅ works             | high       | `https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search` |
| YCombinator (HN)    | HTTP (JSON API)         | ✅ works             | high       | `https://hacker-news.firebaseio.com/v0/jobstories.json`                  |
| Remotive            | HTTP (JSON API)         | ✅ works             | high       | `https://remotive.com/api/remote-jobs`                                   |
| RemoteOK            | HTTP (JSON API)         | ✅ works             | high       | `https://remoteok.com/api`                                               |
| WWR                 | HTTP (RSS)              | ✅ works             | high       | `https://weworkremotely.com/remote-jobs.rss`                             |
| Arbeitnow           | HTTP (JSON API)         | ✅ works             | high       | `https://www.arbeitnow.com/api/job-board-api`                            |
| Berlin Startup Jobs | HTTP (RSS)              | ✅ works             | high       | `https://berlinstartupjobs.com/feed/`                                    |
| German Tech Jobs    | HTTP (RSS/XML)          | ❌ /rss → 403        | high       | `https://germantechjobs.de/job_feed.xml` (working)                       |
| Greenhouse          | HTTP (JSON API)         | ✅ works             | high       | `https://boards-api.greenhouse.io/v1/boards/{slug}/jobs`                 |
| Lever               | HTTP (JSON API)         | ✅ works             | high       | `https://api.lever.co/v0/postings/{company}?mode=json`                   |
| StepStone           | HTTP (ld+json)          | ⚠️ fragile           | medium     | `https://www.stepstone.de/jobs/{query}?page={n}`                         |
| SmartRecruiters     | HTTP (JSON API)         | ✅ works             | high       | `https://api.smartrecruiters.com/v1/companies/{id}/postings`             |
| Personio            | HTTP (XML feed)         | ✅ works             | high       | `https://{company}.jobs.personio.de/xml`                                 |
| Recruitee           | HTTP (JSON API)         | ✅ works             | high       | `https://{company}.recruitee.com/api/offers/`                            |
| Workday             | HTTP (JSON)             | ❌ Cloudflare blocks | high       | `POST …/wday/cxs/{tenant}/{site}/jobs` (422 on all programmatic POSTs)   |
| Ashby               | HTTP (JSON API)         | ✅ works             | high       | `https://api.ashbyhq.com/posting-api/job-board/{clientname}`             |
| Arbeitsagentur      | HTTP (JSON API)         | ✅ works             | high       | `https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4/jobs`   |

**Status legend:** ✅ works · ⚠️ fragile (anti-bot or stale selectors; works but needs monitoring) · ❌ broken (endpoint returns errors or scraper logic is stale and requires code changes)

---

## Per-board details

### Glassdoor

**Source:** `apps/tauri/src-tauri/src/scraping/boards/glassdoor/` (currently scrapes legacy HTML)

#### Current scraper

- **URL (stale):** `https://www.glassdoor.com/Job/jobs.htm?sc.keyword={query}&locKeyword={location}&p={page}`
- **Mode:** Browser (chromiumoxide)
- **Selectors (stale):** `li[data-test='jobListing']`, `a[data-test='job-link']`, `div[data-test='employer-name']`, etc.
- **Staleness:** CRITICAL. Glassdoor migrated to a React/Next.js SPA. The HTML shell contains no job data; all jobs are loaded client-side via a JSON BFF API. Legacy CSS selectors return nothing.

#### Verified endpoint (2026-06-20)

- **Search URL:** `POST https://www.glassdoor.com/job-search-next/bff/jobSearchResultsQuery`
- **Request body (JSON):** `keyword`, `locationId` (numeric Glassdoor ID, not a city string), `locationType` (CITY/STATE/COUNTRY/METRO), `pageNumber` (0-indexed), `pageCursor` (opaque string from prior response), `numJobsToShow` (30), `filterParams[]`
- **Pagination:** `pageNumber` + `pageCursor` from `response.data.jobListings.paginationCursors`; max ~30 pages per query
- **Auth required:** Session cookies required for reliable access (`___cf_bm`, `gd_user_id`). Obtain by first GETing `https://www.glassdoor.com/`. Without auth: 10–20 pages/IP/day before Cloudflare challenge.
- **Response format:** JSON — `response.data.jobListings.jobListings[].jobview`
- **Key data fields:**
  - Title: `jobview.header.jobTitleText`
  - Company: `jobview.header.employerNameFromSearch`
  - Location: `jobview.header.locationName`
  - Stable ID: `jobview.job.listingId`
  - URL: `jobview.header.seoJobLink`
  - Age: `jobview.header.ageInDays`
  - Salary (estimated percentiles): `jobview.header.payPeriodAdjustedPay.p10` / `.p90`
  - Description: secondary call to `GET /job-listing/api/job-details?jobListingId={id}`
- **Anti-bot:** Cloudflare Bot Management with TLS fingerprinting. Standard HTTP clients (plain `reqwest`) are rejected. Requires `curl_cffi`-equivalent with TLS impersonation or a full headless browser with anti-detection flags. Robots.txt disallows `/Jobs`, `/Salaries`, `/Interview`.

#### Recommendation

Rewrite: replace `jobs.htm` + CSS selectors with `POST /job-search-next/bff/jobSearchResultsQuery`. Implement session-cookie seeding before scraping. Use pagination cursors, not sequential page numbers. Add TLS impersonation or harden the existing browser approach.

---

### Indeed

**Source:** `apps/tauri/src-tauri/src/scraping/boards/indeed/`

#### Current scraper

- **URL:** `https://{domain}/jobs?q={query}&l={location}&start={page*10}`
- **Mode:** Browser (chromiumoxide with persistent profile + cookie-based auth)
- **Selectors:** `div.job_seen_beacon`, `h2.jobTitle span[title]`, `[data-testid='company-name']`, `[data-testid='text-location']`, `div.job-snippet`, `data-jk` attribute
- **Staleness:** Moderate-high. Scraper has both modern `data-testid` attributes and legacy class selectors. Legacy `.companyName`/`.companyLocation` classes churn frequently. No explicit wait-for-selector; relies on implicit delays.

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://www.indeed.com/jobs` (locale variants: `de.indeed.com`, `uk.indeed.com`, etc.)
- **Query params:** `q` (search, `+`-encoded), `l` (location, `+`-encoded), `start` (offset: 0, 10, 20 … max ~1000)
- **Pagination:** `start` offset, step 10; ~100 pages max
- **Auth required:** No anonymous auth, but browser+cookies required to bypass Cloudflare/fingerprinting and hCaptcha escalation at volume
- **Response format:** Server-rendered HTML + React injection; must wait for `[data-testid='jobsearch-ResultsList']`
- **Key data fields:**
  - Title: `h2.jobTitle > span[title]` (or `a span`)
  - Company: `[data-testid='company-name']` (fallback `span.companyName`)
  - Location: `[data-testid='text-location']` (fallback `div.companyLocation`)
  - Snippet: `div[data-testid='job-snippet']`
  - Stable ID: `data-jk` attribute (fallback: regex extract `?jk=…` from href)
  - Posted date: `span.date` — NOT currently extracted; always `None`
  - Salary: `[data-testid='attribute_snippet_testid']` — NOT currently extracted
- **Anti-bot:** Cloudflare Enterprise + JavaScript fingerprinting + hCaptcha on volume. Repo correctly uses chromiumoxide + persistent profile + `--disable-blink-features=AutomationControlled`. No hCaptcha solver present.

#### Recommendation

Prioritize `data-testid` selectors over class names (class names hash frequently). Explicitly extract `data-jk` before regex fallback. Add `span.date` parsing for `posted_at`. Add an explicit wait-for-selector for `[data-testid='jobsearch-ResultsList']`.

---

### Xing

**Source:** `apps/tauri/src-tauri/src/scraping/boards/xing/`

#### Current scraper

- **URL:** `https://www.xing.com/jobs/search?keywords={query}&location={location}&page={page}`
- **Mode:** Browser (chromiumoxide) with authenticated cookies
- **Selectors:** `article[data-testid='job-search-result']`, `[data-testid='job-title']`, `[data-testid='job-company-name']`, `[data-testid='job-location']`; fallbacks: `h2.job-teaser__title`, `.companyName`, `p.location`
- **Staleness:** Unknown — selectors cannot be confirmed live due to access restrictions. `data-testid` selectors are resilient; class fallbacks are fragile.

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://www.xing.com/jobs/search`
- **Query params:** `keywords`, `location`, `page` (1-indexed)
- **Pagination:** Numeric `page`; max 5 pages (clamped in scraper); stops on 0 results or non-2xx
- **Auth required:** YES. Login via `https://login.xing.com/login`; cookies persisted to `browser-state/xing/cookies.json`
- **Response format:** Server-rendered HTML, parsed with CSS selectors
- **Key data fields:**
  - Title: `[data-testid='job-title']` (fallback `h2[class*='title']`)
  - Company: `[data-testid='job-company-name']` (fallback `.companyName`)
  - Location: `[data-testid='job-location']` (fallback `p.location`)
  - URL: `a[href*='/jobs/']`; href may be relative — reconstructed as `https://www.xing.com{href}`
  - Stable ID: last path segment of job URL (e.g. `software-engineer-abc123`)
  - Posted date: NOT extracted (always `None`)
  - Description: NOT extracted (always `None`)
  - Salary: NOT extracted
- **Anti-bot:** Cloudflare (stricter on AI scraping since July 2025 per third-party reports); chromiumoxide browser approach may still work; no captcha solver present

#### Recommendation

Verify live selectors immediately in an authenticated browser session. Add `description` and `posted_at` extraction. Confirm whether unauthenticated access still redirects to login. Monitor for Cloudflare TLS fingerprinting escalation.

---

### LinkedIn

**Source:** `apps/tauri/src-tauri/src/scraping/linkedin/api_client/mod.rs`

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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/ycombinator/`

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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/remotive/mod.rs`

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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/remoteok/mod.rs`

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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/wwr/`

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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/arbeitnow/mod.rs`

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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/berlinstartupjobs/`

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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/germantechjobs/`

#### Current scraper

- **URL:** `https://germantechjobs.de/rss` — **NOW BROKEN (HTTP 403)**
- **Mode:** HTTP (RSS/XML, `feed_rs`)

#### Verified endpoint (2026-06-20)

- **Working URL:** `https://germantechjobs.de/job_feed.xml` (HTTP 200, verified)
- **Alternate:** `https://germantechjobs.de/job_feed_stelleninserate_de.xml` (HTTP 200, verified)
- **Query params:** None; full feed; client-side keyword/location filtering
- **Pagination:** None; single monolithic feed (can reach ~10 MB)
- **Auth required:** None
- **Response format:** XML (RSS/Atom via `feed_rs`); custom elements: `<company>`, `<company-name>`, `<salary>`, `<location>`, `<city>`, `<region>`, `<country>`, `<postal_code>`, `<job-type>`, `<apply_url>`, `<ispromoted>`, `<cpc>`
- **Key data fields:**
  - Title: `<title>` (e.g. `"Junior Java Developer (m/w/d)"`)
  - Company: `<company>` / `<company-name>` (e.g. `"Senacor Technologies AG"`)
  - Salary: `<salary>` field present (e.g. `"50.000 - 65.000 €"`) — structured, not embedded in HTML
  - Location: `<location>`, `<city>`, `<region>`
  - URL: `<link>` / `<apply_url>`
  - Stable ID: `<id>` (e.g. `"5fe377e2d2afdd001764761a-W25"`)
  - Posted: `<pubdate>` (format: `DD.MM.YYYY`)
  - Description: `<description>` CDATA
- **Anti-bot:** None on working feed endpoints

#### Recommendation

**CRITICAL fix:** Update URL from `https://germantechjobs.de/rss` to `https://germantechjobs.de/job_feed.xml`. Schema is identical; parser logic requires no changes. Optionally add a fallback: try `/rss` → on 403 retry `/job_feed.xml`.

---

### Greenhouse

**Source:** `apps/tauri/src-tauri/src/scraping/boards/greenhouse/mod.rs`

#### Current scraper

- **URL:** `https://boards-api.greenhouse.io/v1/boards/{company_slug}/jobs?content=true`
- **Mode:** HTTP (reqwest)
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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/lever/mod.rs`

#### Current scraper

- **URL:** `https://api.lever.co/v0/postings/{company}?mode=json`
- **Mode:** HTTP (reqwest)
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

### StepStone

**Source:** `apps/tauri/src-tauri/src/scraping/boards/stepstone/mod.rs` (URL construction lines 88–101; ld+json parse lines 131–159)

#### Current scraper

- **URL pattern:** `https://www.stepstone.de/jobs/{query}?page={n}` or `https://www.stepstone.de/jobs/{query}/in-{location}?page={n}`
- **Mode:** HTTP (reqwest with rustls; no JS rendering)
- **Method:** Parse `<script type="application/ld+json">` blocks (schema.org `JobPosting`)

#### Verified endpoint (2026-06-20)

- **Search URL:** `https://www.stepstone.de/jobs/{urlencoded_query}?page={n}` (keyword-only) or with `/in-{urlencoded_location}` segment
- **Query params:** `page` (1-indexed); max 5 pages (clamped in scraper)
- **Pagination:** `?page=N`; stops on non-200 or no new ld+json items found
- **Auth required:** Anonymous; standard desktop User-Agent; `Accept-Language: de-DE`
- **Response format:** HTML with embedded `application/ld+json` (schema.org `JobPosting`)
- **Key data fields:**
  - Title: `ld+json.title` (required)
  - Company: `ld+json.hiringOrganization.name` (default `"Unknown"`)
  - Location: `ld+json.jobLocation.address.addressLocality` + `addressCountry`
  - URL: `ld+json.url`
  - Stable ID: extracted from URL — regex `[?&]ID=([^&]+)` (uppercase param), fallback `(\d{6,})` (6+ digit path segment)
  - Posted: `ld+json.datePosted` (RFC3339)
  - Description: `ld+json.description` (HTML, stripped)
  - Salary: `ld+json.baseSalary` — NOT currently extracted
- **Anti-bot:** CONFIRMED FRAGILE. Test file (line 109) documents: `"StepStone is bot-sensitive (timeout/403 from certain IPs / CI)"`. WebFetch attempts timed out (>60 s). Likely Cloudflare. Scraper mitigates: per-host rate limiting, 900–1500 ms jitter between pages. No captcha solver.

#### Recommendation

URL structure is correct and working (when not blocked). Add `ld+json.baseSalary.minValue`/`.maxValue`/`.currency` extraction. Keep test marked `#[ignore]` in CI. If IP-level blocking becomes consistent, consider residential proxy or increasing jitter.

---

### SmartRecruiters

**Source:** `apps/tauri/src-tauri/src/scraping/boards/smartrecruiters/`

#### Current scraper

- **URL:** `https://api.smartrecruiters.com/v1/companies/{companyIdentifier}/postings?limit=100`
- **Mode:** HTTP (reqwest)
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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/personio/mod.rs`

#### Current scraper

- **URL:** `https://{company}.jobs.personio.de/xml`
- **Mode:** HTTP (reqwest)
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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/recruitee/`

#### Current scraper

- **URL:** `https://{company}.recruitee.com/api/offers/`
- **Mode:** HTTP (reqwest)
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

### Workday

**Source:** `apps/tauri/src-tauri/src/scraping/boards/workday/mod.rs` + `test.rs`

#### Current scraper

- **URL:** `POST https://{tenant}.{wd_server}.myworkdayjobs.com/wday/cxs/{tenant}/{site}/jobs`
- **Mode:** HTTP (reqwest)
- **Request body:** `{ appliedFacets: {}, searchText: "", limit: 20, offset: page*20 }`

#### Verified endpoint (2026-06-20)

- **Search URL:** `POST https://{tenant}.{wd_server}.myworkdayjobs.com/wday/cxs/{tenant}/{site}/jobs`
- **Query params:** None (JSON body only); `{wd_server}` varies per tenant (wd1, wd3, wd5, …); tenant/site extracted from company career URL or colon-delimited hint (e.g. `amazon:External:wd1`)
- **Pagination:** `offset` increment by `limit` (20); stop when `jobPostings` empty or `offset >= total`; hard cap: 10,000 results per query
- **Auth required:** Anonymous (no API key); BUT Cloudflare Bot Management is the blocker
- **Response format:** JSON `{ jobPostings: [{title, externalPath, locationsText, postedOn, bulletFields}], total: int }`; detail GET returns `{ jobPostingInfo: { jobDescription (HTML), jobPostingId } }`
- **Key data fields:**
  - Title: `jobPostings[].title`
  - Location: `jobPostings[].locationsText` (optional)
  - Stable ID: derived from `externalPath` path segment; prefixed `workday:{external_id}`
  - Posted: `jobPostings[].postedOn` (RFC3339)
  - Description: detail endpoint `jobPostingInfo.jobDescription` (HTML, stripped)
  - Company: from tenant slug
  - Salary: NOT exposed in either endpoint
- **Anti-bot:** CONFIRMED BLOCKER. Test file explicitly documents: `"Workday CXS endpoints are protected by Cloudflare Bot Management (__cf_bm cookie requires a JS challenge). All programmatic POSTs return 422 regardless of tenant, body, or headers."` Implementation is correct but currently non-functional for HTTP-only clients.

#### Recommendation

The scraper implementation is architecturally correct but blocked by Cloudflare. Unblock options: (A) shift to headless browser (Chromium) to obtain `__cf_bm` cookie then reuse for CXS API calls; (B) use a residential proxy service that solves the JS challenge; (C) use Apify's Workday actor via API. No changes to parsing logic needed — only the network layer.

---

### Ashby

**Source:** `apps/tauri/src-tauri/src/scraping/boards/ashby/mod.rs`

#### Current scraper

- **URL:** `https://api.ashbyhq.com/posting-api/job-board/{clientname}?includeCompensation=true`
- **Mode:** HTTP (reqwest)
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

**Source:** `apps/tauri/src-tauri/src/scraping/boards/arbeitsagentur/`

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

## Top priorities

### Needs code fixes (blocked or broken)

| Board                | Issue                                                                                                                                              | Effort |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| **Glassdoor**        | Legacy `jobs.htm` + CSS selectors completely stale; must migrate to `POST /job-search-next/bff/jobSearchResultsQuery` JSON API + TLS impersonation | High   |
| **German Tech Jobs** | `/rss` returns HTTP 403; swap URL to `/job_feed.xml` — one-line fix, parser unchanged                                                              | Low    |
| **Workday**          | All programmatic POSTs return 422 (Cloudflare Bot Management); needs headless browser, residential proxy, or third-party actor                     | High   |

### Confirmed fragile (monitor, may need attention)

| Board         | Risk                                                                                                         |
| ------------- | ------------------------------------------------------------------------------------------------------------ |
| **StepStone** | Cloudflare/bot filter causes timeouts/403 from datacenter IPs and CI; works from desktop                     |
| **Indeed**    | `data-testid` selectors reliable; legacy class selectors (`.companyName`) fragile; hCaptcha if volume spikes |
| **Xing**      | Cannot confirm live selectors; Cloudflare tightened in July 2025; login session required                     |

### Confirmed solid (public APIs, no auth, no anti-bot)

LinkedIn (guest HTML), YCombinator (Firebase), Remotive, RemoteOK, We Work Remotely (RSS), Arbeitnow, Berlin Startup Jobs (RSS), Greenhouse, Lever, SmartRecruiters, Personio (XML), Recruitee, Ashby, Arbeitsagentur.
