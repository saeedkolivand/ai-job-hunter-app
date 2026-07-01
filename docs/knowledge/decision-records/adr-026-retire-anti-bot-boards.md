# ADR-026: Retire self-scraping anti-bot boards; cover via aggregator; keep single-job import

Last updated: 2026-06-21

**Status:** Accepted

## Context

Five job boards — Glassdoor, Indeed, Xing, StepStone, and Workday — were implemented as direct HTTP/browser scrapers in the `SCRAPERS` registry. All five share the same failure mode: anti-bot infrastructure (Cloudflare Bot Management, hCaptcha, JS fingerprinting) that returns empty results or errors on every programmatic access, regardless of auth state:

- **Glassdoor:** Cloudflare Bot Management with TLS fingerprinting blocks all headless Chromium sessions, even when carrying a persisted login profile.
- **Indeed:** hCaptcha + Cloudflare Enterprise at volume; login cookies bought a brief window but were increasingly unreliable.
- **Xing:** Cloudflare tightened on AI/scraping traffic since mid-2025; cookies provided limited mitigation.
- **StepStone:** Bot filter causes consistent 403/timeouts from datacenter IPs; passes from a real desktop browser but never from CI or the app's HTTP client.
- **Workday:** All programmatic POSTs to the CXS endpoint return 422 — Cloudflare Bot Management requires a JS-solved `__cf_bm` cookie that no HTTP-only client can obtain.

The scraper implementations were correct at the protocol level. The failure was structural: self-scraping these boards is a losing maintenance battle. The unblock options (residential proxies, managed actors, Puppeteer-in-a-box) all conflict with the project's local-first, bring-your-own-key model and would introduce ongoing per-request cost or third-party dependency that users cannot control.

Meanwhile, the Adzuna/JSearch aggregator (introduced in PR #465) already covers this category of board with a simple API call under the user's own API keys.

## Decision

Remove the five boards from the `SCRAPERS` registry and delete their Rust scraper modules. The registry goes from 21 → 16 boards. Coverage for these boards is provided by the existing Aggregator board (Adzuna primary / JSearch paid fallback).

### What is deliberately KEPT (dormant or active)

These items were considered for removal but kept for good reasons:

| Item                                                                                                    | Kept reason                                                                                                                                                                                                                                                   |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `scraping/scrape_url/mod.rs`: `resolve()`, `canonical_job_url()` (Indeed URL resolver), `try_workday()` | Used by the browser extension single-job import flow. These are pure URL transforms with no authenticated scrape loop — they resolve a board URL to a canonical job URL and hand off to the app. Removing them would break extension import for those boards. |
| `scraping/board_login/` and `credentials/` machinery                                                    | Infrastructure, not board-specific. LinkedIn still uses `board_login`. Keeping the machinery avoids a larger surgery.                                                                                                                                         |
| `commands/boards.rs` `boards_list()`                                                                    | Trimmed to `["linkedin"]` — indeed/xing/glassdoor login no longer feeds anything active. But the command and infrastructure remain.                                                                                                                           |
| `CredentialSetSchema` / `CredentialBoardSchema` in shared schemas                                       | Dormant; keeping them avoids a schema migration that would serve no current purpose.                                                                                                                                                                          |
| Privacy "clear all" (`privacy_reset_app`) disconnect list                                               | Still includes the retired board IDs to wipe any lingering session cookies from before the migration. Deliberate: a user who had sessions pre-migration should get a clean slate on reset.                                                                    |
| Translation keys `jobs.boards.indeed`, `jobs.boards.xing`, `jobs.boards.glassdoor`                      | Kept because they are still referenced from test files that exercise credential/IPC machinery (not scrapeability). `jobs.faq.indeedWrongCountry` was removed (the FAQ key was Only used in the scraping UI).                                                  |

### What is removed

- Rust modules: `scraping/boards/{glassdoor,indeed,xing,stepstone,workday}/` (mod.rs + test.rs)
- Registry entries: 5 `pub mod`, 5 `pub use`, 5 `SCRAPERS` entries in `scraping/boards/mod.rs`
- Shared schema: `BOARD_IDS` entries for `indeed`, `stepstone`, `xing`, `workday` (glassdoor was never in `BOARD_IDS`)
- `StepStone` per-host rate-limiter branch in `scraping/rate_limiter/mod.rs`
- `locale` field: fully removed — `ScrapeBoardsRequestSchema` (shared), `BoardSearchInput` (Rust), the generated `ipc_contracts/scrape.rs`, `commands/scrape.rs` and `autopilot_helpers` passthroughs, and ~18 Rust test fixtures. Was an Indeed-region-only field; the aggregator localises via `country_code` (which is kept).
- Renderer: Indeed region dropdown, `locale` form field, `AUTH_BENEFITS` entries for indeed/xing, `BOARD_STYLE` map entries for indeed/xing/glassdoor, `boards_list()` trimmed to linkedin-only
- Translation keys: `jobs.boards.workday`, `jobs.boards.stepstone`, entire `jobs.regions` block, `jobs.region`, `jobs.selectRegion`, `jobs.faq.indeedWrongCountry`

### Unknown-board graceful-skip

`scraping/boards::get()` returns `Option<&dyn Scraper>`. A retired board ID resolves to `None`, which `scrape_boards_with_resolver` converts to `Err("Unknown board: …")` recorded as an error summary — no panic. A persisted Autopilot referencing a retired board continues with the rest of its boards and surfaces an error entry for that one.

## Alternatives considered

| Alternative                                                                                   | Why rejected                                                                                                                                                                         |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Keep scraping with stealth tooling (puppeteer-extra stealth, TLS impersonation via curl_cffi) | Introduces heavy runtime dependencies; stealth measures have a half-life (platforms adapt); does not fit the local-first bring-your-own-key model.                                   |
| Residential proxy layer                                                                       | Per-request cost and third-party dependency users cannot control; violates the local-first model.                                                                                    |
| Full scraper rebuild per board                                                                | Would face the same bot walls immediately. The block is structural, not a selector/parsing bug.                                                                                      |
| Keep scraping but gate on "optional" auth                                                     | Already tried for Indeed/Xing/Glassdoor; the outcome is consistently empty results regardless of auth state because the block is at the IP/fingerprint layer, not the session layer. |
| Apify or managed scraping service                                                             | Per-request cost; external dependency; violates the "no AI Job Hunter server in the path" privacy contract.                                                                          |

## Consequences

- **Registry shrinks to 16 boards.** All active scrapers are either public APIs (LinkedIn, YCombinator, Remotive, RemoteOK, WWR, Arbeitnow, BerlinStartupJobs, GermanTechJobs), company-scoped ATS APIs (Greenhouse, Lever, Ashby, Personio, Recruitee, SmartRecruiters), or the Aggregator.
- **Aggregator coverage depends on Adzuna/JSearch keys.** Users without keys see empty results for these boards, same as before (the scrapers were also returning empty). Users with keys get better coverage than the scraper ever provided.
- **German-market follow-up.** Adzuna.de depth for German-language roles should be validated. If thin, a dedicated German-market source (e.g. a StepStone aggregator API if one becomes available, or a supplementary board) may be warranted. This is NOT done in this change.
- **Extension import unaffected.** The single-job import flow (browser extension → URL resolver → scrape_url) keeps working for Indeed and Workday URLs because the URL resolvers are pure transforms, not authenticated scrape loops.
- **No schema migration.** `z.array(z.enum(BOARD_IDS))` collapses to `Vec<String>` in the IPC-generated Rust, so removing enum members produces a byte-identical `ipc_contracts/scrape.rs` after `pnpm gen:ipc`.

## Related

- `apps/desktop/src-tauri/src/scraping/boards/mod.rs` — board registry (`SCRAPERS`)
- `apps/desktop/src-tauri/src/scraping/scrape_url/mod.rs` — kept URL resolvers for Indeed + Workday
- `apps/desktop/src-tauri/src/scraping/boards/aggregator/` — Adzuna/JSearch provider registry
- `packages/shared/src/schemas/index.ts` — `BOARD_IDS` (4 retired IDs removed; glassdoor was never present)
- `docs/SCRAPING_ENDPOINTS.md` — per-board endpoint reconnaissance; retired boards noted
- ADR-019 (performance profile) — mentions scraping concurrency constraints
