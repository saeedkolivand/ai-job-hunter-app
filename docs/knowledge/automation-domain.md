# Automation domain (scraping + apply assistant + AI provider)

Last updated: 2026-06-13

Merged knowledge for `scraping-applier-expert` and `ai-provider-expert`. Source is authoritative for board/provider counts.

## Scraping (`scraping/`)

- **Registry** — `scraping/boards/mod.rs`: `SCRAPERS` + the `Scraper` trait. `ScraperMode` = Http or Browser. Adding a board = implement `Scraper` + register; never special-case outside the registry. Board count lives in the registry — don't copy it.
- **Engine / transport** — scraping transport is HTTP via `scraping/engine/` and `scraping/http/`; all current boards use this path. Chromium ([chromiumoxide][chromiumoxide]) is used exclusively by `scraping/board_login/` for manual login + encrypted cookie import (reads Chromium's v10/v11 stores, decrypts, writes scraper-consumable artifacts via `import_cookies`; see `commands/boards.rs: boards_import_cookies`). LinkedIn-specific flow: `scraping/linkedin/` (HTTP clients).
- **Context** — `ScrapeContext` carries a **cancellation token** + progress/item callbacks. Honoring cancellation, bounded retries/backoff, and per-board rate limits are reliability requirements (ignoring the token or unbounded retries on a network loop = HIGH).
- **Selector resilience** — core boards need fallback selectors; a brittle single-selector parse on a core board is HIGH (it breaks on the next site redesign).

## Apply assistant (no auto-apply engine)

- **Decision (2026-06, PR #7 of the UX backlog):** the browser-automation apply engine (`applying/` — board appliers, `captcha_handler`, `form_filler`, the `APPLIERS` registry — plus `commands/apply`, `apply_helpers/`, and the apply IPC contract) was **removed**. The app is an **apply assistant**, not an auto-applier: selector drift, captcha, and per-board form logic made an auto-submit engine too costly to maintain, and it was never user-facing ("Coming Soon"). `chromiumoxide` stays — scraping uses it.
- **What the assistant does** — from a found or scraped job the user tailors a résumé + cover letter (`renderer/features/autopilot/.../ApplyJobModal`, `renderer/store/generation-store/`), gets résumé-grounded application answers, opens the posting in the browser, and submits it themselves. Autopilot (`autopilot/`, `autopilot_scheduler`) **finds → ranks → notifies**; it never submits. Found-jobs PostingRow "Tailor" seeds the AI Generate workspace for any board.
- **Autopilot scheduling** — **clock-anchored** (PR #266; was interval-based). `autopilot_scheduler.rs` computes the most-recent scheduled occurrence in local time (`chrono::Local`) via `last_occurrence_ms` / `is_due`; on launch it catches up to any missed run (single catch-up, no double-fire). Frequency modes: `daily` = HH:MM; `twice_daily` = HH:MM + 12 h; `hourly` = :MM; `manual` = no scheduling. Fields `scheduleHour` (0–23) and `scheduleMinute` (0–59) on the autopilot model — Zod-validated client-side, range-guarded in the store. Legacy records default to 09:00.
- **"Applied" tracking** — derived, never auto-set: a found job is "applied" when a saved generation's `jobUrl` matches it (`commands/autopilot.rs: enrich_applied`). Each autopilot keeps an optional **base cover letter** (`coverLetter`) the assistant tailors per job.
- **Security** — never log credentials/cookies; board session handling for **scraping** is co-reviewed by `tauri-security-reviewer`.

## AI provider (`commands/ai_provider/`)

- **Abstraction (architectural rule — HIGH if violated)** — `ollama.rs`, `openai.rs`, `anthropic.rs`, `gemini.rs`, `cli_agent/` behind a shared interface (`mod.rs`). **No business logic depends on a provider-specific API.** Adding OpenAI/Anthropic/Gemini/Ollama/OpenRouter/LM Studio/future = **config + adapter only**.
- **Embeddings & match scoring** — `documents/mod.rs`: document + posting-vector storage. Match-score result cache with self-invalidating composite key (provider + model + semantic_enabled + formula_version + job_text_hash). See [ADR-017](decision-records/adr-017-persisted-self-invalidating-match-score-caches.md) for the caching strategy. Embedding-space invalidation when the model/space changes (stale embeddings = HIGH). `posting_vector_or_embed()` is the resolver; `match_resume.rs` wraps results in the cache.
- **Streaming / thinking normalization** — every provider maps reasoning to `ai:stream { delta, thinking:true }`; inline `<think>` blocks for local models are split by `renderer/lib/generate/think-split.ts: createThinkSplitter`. See [ADR-005](decision-records/adr-005-universal-thinking-normalization.md).
- **Generation session store** — `renderer/store/generation-store/` ([Zustand][zustand]), keyed by context id, survives navigation/close. See [ADR-006](decision-records/adr-006-generation-session-store.md).
- **Prompts** — `packages/prompts` (provider-aware, locale-driven, pure [TypeScript][typescript], zero deps); reusable/composable templates.
- **Cost/token** — minimize prompt/context; reuse embedded context; pick the cheapest viable model (`performance-profiler` co-reviews hot AI paths).

[chromiumoxide]: https://github.com/mattsse/chromiumoxide
[zustand]: https://github.com/pmndrs/zustand
[typescript]: https://www.typescriptlang.org
