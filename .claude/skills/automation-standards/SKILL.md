---
name: automation-standards
description: Scraping + AI-provider standards — Scraper/Applier traits & registries, selector resilience, rate-limiting/cancellation, and the provider-abstraction (zero-change) rule for embeddings/streaming/prompts. Load for changes under scraping/, applying/, ai_provider/, packages/prompts, documents/embed.
---

# Automation standards (scraping + AI-provider)

Authoritative: `docs/knowledge/automation-domain.md`.

## Scraping / applying

- Register via the registries — `scraping/boards/mod.rs` (`SCRAPERS`, `Scraper` trait, `ScraperMode` Http/Browser) and `applying/registry/mod.rs` (`APPLIERS`, `Applier` trait). Don't special-case boards outside the registry.
- **Selector resilience** — core boards need fallback selectors; a brittle single-selector parse on a core board is HIGH.
- **Reliability** — honor the cancellation token in `ScrapeContext`/`ApplyContext`; bounded retries with backoff; per-board rate limits; graceful failure recovery (don't poison the queue).
- **Sessions/cookies** — handled safely; never log credentials/cookies (security lens → `tauri-security-reviewer`).

## AI provider (the architectural rule — HIGH if violated)

- **No business logic depends on provider-specific APIs.** All providers implement a shared interface; adding OpenAI/Anthropic/Gemini/Ollama/OpenRouter/LM Studio = **config + adapter only**.
- **Embeddings** — versioned; on model/space change, invalidation must run (stale embeddings are HIGH).
- **Streaming** — partial responses + cancellation handled; no leaks on cancel.
- **Prompts** (`packages/prompts`) — provider-aware + locale-driven, pure TS, zero deps; reusable/composable templates.
- **Cost** — minimize token/context; pick the cheapest viable model.
