# Automation domain (scraping + applying + AI provider)

Merged knowledge for `scraping-applier-expert` and `ai-provider-expert`. Source is authoritative for board/applier/provider counts.

## Scraping (`scraping/`)

- **Registry** — `scraping/boards/mod.rs`: `SCRAPERS` + the `Scraper` trait. `ScraperMode` = Http or Browser. Adding a board = implement `Scraper` + register; never special-case outside the registry. Board count lives in the registry — don't copy it.
- **Engine / transport** — `scraping/engine/`, `scraping/http/`, `scraping/browser` via `browser/` (chromiumoxide). LinkedIn-specific flow: `scraping/linkedin/`, login: `scraping/board_login/`.
- **Context** — `ScrapeContext` carries a **cancellation token** + progress/item callbacks. Honoring cancellation, bounded retries/backoff, and per-board rate limits are reliability requirements (ignoring the token or unbounded retries on a network loop = HIGH).
- **Selector resilience** — core boards need fallback selectors; a brittle single-selector parse on a core board is HIGH (it breaks on the next site redesign).

## Applying (`applying/`)

- **Registry** — `applying/registry/mod.rs`: `APPLIERS` + the `Applier` trait. Form filling: `form_filler/` + `selectors/`; captcha: `captcha_handler.rs`; recovery: `error_handler.rs`, `runtime.rs`.
- **Reliability** — graceful failure recovery (don't poison the queue), validation-error handling, cancellation. Autopilot orchestrates runs (`autopilot/`, `apply_helpers/`).
- **Security** — never log credentials/cookies; session handling is co-reviewed by `tauri-security-reviewer`.

## AI provider (`commands/ai_provider/`)

- **Abstraction (architectural rule — HIGH if violated)** — `ollama.rs`, `openai.rs`, `anthropic.rs`, `gemini.rs`, `cli_agent/` behind a shared interface (`mod.rs`). **No business logic depends on a provider-specific API.** Adding OpenAI/Anthropic/Gemini/Ollama/OpenRouter/LM Studio/future = **config + adapter only**.
- **Embeddings** — `documents/mod.rs`: storage + **embedding-space invalidation** when the model/space changes (stale embeddings across a model switch = HIGH).
- **Streaming** — partial responses + cancellation handled; no resource leak on cancel.
- **Prompts** — `packages/prompts` (provider-aware, locale-driven, pure TS, zero deps); reusable/composable templates.
- **Cost/token** — minimize prompt/context; reuse embedded context; pick the cheapest viable model (`performance-profiler` co-reviews hot AI paths).
