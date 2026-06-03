# Automation domain (scraping + apply assistant + AI provider)

Last updated: 2026-06-03

Merged knowledge for `scraping-applier-expert` and `ai-provider-expert`. Source is authoritative for board/provider counts.

## Scraping (`scraping/`)

- **Registry** — `scraping/boards/mod.rs`: `SCRAPERS` + the `Scraper` trait. `ScraperMode` = Http or Browser. Adding a board = implement `Scraper` + register; never special-case outside the registry. Board count lives in the registry — don't copy it.
- **Engine / transport** — `scraping/engine/`, `scraping/http/`, `scraping/browser` via `browser/` ([chromiumoxide][chromiumoxide]). LinkedIn-specific flow: `scraping/linkedin/`, login: `scraping/board_login/`.
- **Context** — `ScrapeContext` carries a **cancellation token** + progress/item callbacks. Honoring cancellation, bounded retries/backoff, and per-board rate limits are reliability requirements (ignoring the token or unbounded retries on a network loop = HIGH).
- **Selector resilience** — core boards need fallback selectors; a brittle single-selector parse on a core board is HIGH (it breaks on the next site redesign).

## Apply assistant (no auto-apply engine)

- **Decision (2026-06, PR #7 of the UX backlog):** the browser-automation apply engine (`applying/` — board appliers, `captcha_handler`, `form_filler`, the `APPLIERS` registry — plus `commands/apply`, `apply_helpers/`, and the apply IPC contract) was **removed**. The app is an **apply assistant**, not an auto-applier: selector drift, captcha, and per-board form logic made an auto-submit engine too costly to maintain, and it was never user-facing ("Coming Soon"). `chromiumoxide` stays — scraping uses it.
- **What the assistant does** — from a found or scraped job the user tailors a résumé + cover letter (`renderer/features/autopilot/.../ApplyJobModal`, `renderer/store/generation-store/`), gets résumé-grounded application answers, opens the posting in the browser, and submits it themselves. Autopilot (`autopilot/`, `autopilot_scheduler`) **finds → ranks → notifies**; it never submits. Found-jobs PostingRow "Tailor" seeds the AI Generate workspace for any board.
- **"Applied" tracking** — derived, never auto-set: a found job is "applied" when a saved generation's `jobUrl` matches it (`commands/autopilot.rs: enrich_applied`). Each autopilot keeps an optional **base cover letter** (`coverLetter`) the assistant tailors per job.
- **Security** — never log credentials/cookies; board session handling for **scraping** is co-reviewed by `tauri-security-reviewer`.

## AI provider (`commands/ai_provider/`)

- **Abstraction (architectural rule — HIGH if violated)** — `ollama.rs`, `openai.rs`, `anthropic.rs`, `gemini.rs`, `cli_agent/` behind a shared interface (`mod.rs`). **No business logic depends on a provider-specific API.** Adding OpenAI/Anthropic/Gemini/Ollama/OpenRouter/LM Studio/future = **config + adapter only**.
- **Embeddings** — `documents/mod.rs`: storage + **embedding-space invalidation** when the model/space changes (stale embeddings across a model switch = HIGH).
- **Streaming / thinking normalization** — every provider maps reasoning to `ai:stream { delta, thinking:true }`; inline `<think>` blocks for local models are split by `renderer/lib/generate/think-split.ts: createThinkSplitter`. See [ADR-005](decision-records/adr-005-universal-thinking-normalization.md).
- **Generation session store** — `renderer/store/generation-store/` ([Zustand][zustand]), keyed by context id, survives navigation/close. See [ADR-006](decision-records/adr-006-generation-session-store.md).
- **Prompts** — `packages/prompts` (provider-aware, locale-driven, pure [TypeScript][typescript], zero deps); reusable/composable templates.
- **Cost/token** — minimize prompt/context; reuse embedded context; pick the cheapest viable model (`performance-profiler` co-reviews hot AI paths).

[chromiumoxide]: https://github.com/mattsse/chromiumoxide
[zustand]: https://github.com/pmndrs/zustand
[typescript]: https://www.typescriptlang.org
