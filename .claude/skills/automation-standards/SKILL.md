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

## External standards & best-practices (verified 2026-06-19)

### AI / LLM (cross-provider — Claude specifics live in the `claude-api` skill)

- **OWASP Top 10 for LLM Apps (2025)** — https://genai.owasp.org/llm-top-10/
  - **LLM01 Prompt Injection** (direct + indirect) — **segregate/label untrusted external content** (scraped JD/résumé text is untrusted; never concatenate raw into the instruction block); constrain model role; deterministically validate output; least-privilege tool tokens; human-in-the-loop on high-risk actions. No fool-proof fix → layer defenses.
  - **LLM05 Improper Output Handling** — treat output as untrusted; parse/validate (Zod/serde) before it reaches IPC/files/render; never `eval`/shell/SQL with raw output.
  - **LLM02 Sensitive-Info Disclosure** + **LLM07 System-Prompt Leakage** — strip PII/secrets from prompts + logs; no secrets in system prompts. **LLM06 Excessive Agency** — minimal tools/permissions; gate side-effects behind user confirmation.
- **Structured output** — prefer native tool/function-calling with constrained decoding over free-text JSON; schemas guarantee _shape_, not _values_ — still validate.
- **Streaming** — SSE is the cross-provider standard; handle partial/aborted streams, disable proxy buffering, support cancellation, prefer partial+error over silent regen.
- **Retries/idempotency** — backoff + jitter; retry only idempotent reads; idempotency key/ledger for mutating tool calls.
- **Cost/caching** — static prefix first (system prompt + tool schemas) to maximize prompt-cache hits; instrument hit-rate.
- **Evals** — version prompts; rerun a fixed eval set on every prompt/model/provider change (2026 models ship faster than your releases and silently shift behavior).

### Scraping — legality & resilience

> Scope: scrape **public, logged-out** job postings (listings/JD text), not candidate PII or auth-walled pages. Stay in that lane.

- **Public + logged-out only.** Public-data scraping isn't "unauthorized access" under the CFAA (hiQ; narrowed by Van Buren). **Never** log in, bypass CAPTCHAs, rotate IPs to evade blocks, or defeat auth — that flips CFAA + breach risk. https://www.quinnemanuel.com/the-firm/news-events/client-alert-meta-v-bright-data-significant-decision-for-web-scraping-industry/
- ⚠️ **ToS/contract is the real exposure.** _Meta v. Bright Data_ (Jan 2024): logged-OUT scrapers aren't ToS-bound; **logged-in scraping stays bound.** But 2025 _LinkedIn v. Proxycurl_ etc. show boards winning on contract — treat LinkedIn/Indeed-class sites with named anti-scraping ToS as **registry-gated, conservative**.
- ⚠️ **EU/GDPR** — "public" ≠ free to process personal data; needs lawful basis + the EDPB Opinion 28/2024 three-step legitimate-interest test (+ respect robots.txt, exclude sensitive data, minimize). Clearview fines show "public" is no shield. https://iapp.org/news/a/the-state-of-web-scraping-in-the-eu
- **robots.txt (RFC 9309)** — obey it; on 5xx/unreachable assume disallow; cache ≤24h. **Identify honestly** (stable descriptive UA, no browser spoofing to evade). Rate-limit + backoff per host; honor `Retry-After`/429. https://www.rfc-editor.org/rfc/rfc9309.html
- **Resilience** — semantic selectors (ARIA roles, `data-*`, JSON-LD `JobPosting`) over brittle CSS/xpath; version-aware fallback chains; detect drift + **fail loudly** (no silent empty results); each board isolated in the `SCRAPERS` registry. Accept anti-bot reality: **back off / disable a scraper rather than fingerprint-spoof** (spoofing is the legal red line).

**Common mistakes:** concatenating scraped/untrusted text into the instruction block (LLM01); trusting structured-output _values_ because the _shape_ validated; proxy buffering killing streaming; retrying non-idempotent tool calls; secrets in system prompts; shipping a prompt change with no eval gate; logging in / bypassing CAPTCHA-or-rate-limits to scrape; treating "public" EU personal data as free; brittle selectors with no fallback/drift alarm.
