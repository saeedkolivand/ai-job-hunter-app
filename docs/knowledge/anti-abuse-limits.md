# Anti-Abuse Rate & Concurrency Limits

Last updated: 2026-07-16

Canonical source: `apps/desktop/src-tauri/src/limits/mod.rs`

## Overview

The `limits` module provides **in-memory rate limiting + concurrency control** on expensive, abuse-prone operations:

- `ai_generate`: AI inference (cost, latency)
- `ai_research` bucket: Web research lookups (`ai_lookup_salary`, `ai_research_company`, `ai_research_answer`)
- `agent_run`: Agentic loop command (fans out into multiple turns)
- `scrape_board` / `scrape_url`: Web scraping (target rate-limits, IP bans)

The limiter is **process-scoped** (in-memory; resets on app restart) and operates at the **command boundary** (right after deserialization, before business logic). **Multi-board batch limit**: server-side cap enforced by `max_boards_per_batch()` in `apps/desktop/src-tauri/src/scraping/engine/mod.rs` prevents unbounded request amplification from crafted IPC payloads. The engine-level bound scales automatically as new boards are added to the registry (see `max_boards_per_batch()` source; no `scraping/engine` code edit required). Note: the shared Zod schemas in `packages/shared/src/schemas/index.ts` (ScrapeBoardsRequestSchema `.max(BOARD_IDS.length)`) independently bound request size at the IPC boundary; scaling is subject to both limits.

## Design

### Components

**`Limiter`** (process-scoped, sync):

```rust
pub struct Limiter {
    per_command: Mutex<HashMap<&'static str, CommandState>>,
    per_provider_day: Mutex<HashMap<(u64, String), u32>>,
}

pub struct ConcurrencyGuard {
    command: &'static str,
    limiter: Arc<Limiter>,
}
```

Three independent guards (all process-local, reset on restart):

1. **Sliding-window request-rate cap** — at most `max_requests` accepted starts of a given command within the last [`RATE_WINDOW`] (60 seconds). Old timestamps age out, so it is a true rolling window.
2. **Concurrency cap** — at most `max_concurrent` in-flight calls of a command. Acquired as an RAII [`ConcurrencyGuard`] that decrements the live count on drop, so a panicking/early-returning handler can never leak a slot.
3. **Per-provider daily request ceiling** — a generous runaway-cost backstop: at most `PROVIDER_DAILY_MAX` accepted AI requests per provider per UTC day (reset at midnight UTC).

Defaults are intentionally **generous** so normal interactive use never trips them; they exist to stop pathological loops, not to throttle a human.

### Error

When a limit is exceeded, the command returns `AppError::RateLimited(message)`:

```rust
pub enum AppError {
    // ...
    #[error("{0}")]
    RateLimited(String), // e.g., "Rate limit reached for ai_generate: max 20 requests per 60s. Try again shortly."
}
```

The variant's code string is `"RATE_LIMITED"` (line 67 of `error.rs`), and it is marked retriable (line 75), so the renderer can catch it, display a user-facing message, and retry after a delay.

## Usage

### AI Commands

Applied to `ai_generate` and the `ai_research` bucket in `commands/ai.rs` (lines 38–66):

```rust
#[tauri::command]
pub async fn ai_generate(
    request: GenerateRequest,
    limiter: State<'_, Arc<crate::limits::Limiter>>,
) -> AppResult<GenerateResponse> {
    // Acquire concurrency slot for the rate cap
    let _guard = limiter.acquire(
        "ai_generate",
        AI_GENERATE_RATE_MAX,
        AI_GENERATE_CONCURRENCY_MAX,
    )?;

    // Charge the provider's daily ceiling
    limiter.charge_provider_daily(&request.provider, PROVIDER_DAILY_MAX)?;

    // Proceed with generation (guard is held until function returns)
    // ...
}
```

The `_guard` is an RAII [`ConcurrencyGuard`] — when it drops (at function end or on early `?`), the in-flight count is decremented automatically.

### Scraping Commands

Applied to `scrape_board` and `scrape_url` in `commands/scrape.rs` (lines 65–72, 307–316):

```rust
#[tauri::command]
pub async fn scrape_board(
    request: ScrapeBoardRequest,
    limiter: State<'_, Arc<crate::limits::Limiter>>,
) -> AppResult<ScrapeBoardResponse> {
    // Acquire concurrency slot
    let _guard = limiter.acquire(
        "scrape_board",
        SCRAPE_RATE_MAX,
        SCRAPE_CONCURRENCY_MAX,
    )?;

    // Engine enforces multi-board batch cap via max_boards_per_batch() in scraping/engine/mod.rs
    // (CWE-770 defense)
    // ...
}
```

### Agent Run Command

Applied to `agent_run` in `commands/agent.rs` (lines 68–69):

```rust
let _guard = limiter.acquire(
    "agent_run",
    AGENT_RUN_RATE_MAX,
    AGENT_RUN_CONCURRENCY_MAX,
)?;
```

One run fans out into several provider requests; each is separately charged against the daily ceiling.

## Configuration

**Current constants** (in `limits/mod.rs`, lines 45–77):

| Constant                      | Value | Rationale                                                           |
| ----------------------------- | ----- | ------------------------------------------------------------------- |
| `AI_GENERATE_RATE_MAX`        | 20    | 20 per 60s; prevents request storms                                 |
| `AI_GENERATE_CONCURRENCY_MAX` | 3     | At most 3 in-flight; prevents cost spike                            |
| `AI_RESEARCH_RATE_MAX`        | 20    | Shared by ai_lookup_salary, ai_research_company, ai_research_answer |
| `AI_RESEARCH_CONCURRENCY_MAX` | 3     | Shared research-bucket concurrency                                  |
| `SCRAPE_RATE_MAX`             | 30    | 30 per 60s; respect target rate-limits                              |
| `SCRAPE_CONCURRENCY_MAX`      | 2     | At most 2 in-flight; low parallelism                                |
| `AGENT_RUN_RATE_MAX`          | 10    | 10 per 60s; each run fans out                                       |
| `AGENT_RUN_CONCURRENCY_MAX`   | 2     | At most 2 in-flight agentic loops                                   |
| `RATE_WINDOW`                 | 60s   | Rolling window for rate caps                                        |
| `PROVIDER_DAILY_MAX`          | 4000  | Per-provider per-UTC-day ceiling                                    |

All caps are **fixed compile-time constants**. A settings UI to configure them is a known follow-up (limits/mod.rs line 29).

## Related

- **Performance mode** ([`PATTERNS.md` § 11](../PATTERNS.md#11-performance-mode-pattern)): how to adjust limits at runtime.
- **ARCHITECTURE_STATUS.md**: Anti-abuse limits status.
- **PATTERNS.md § 13**: `limits` as a module owner (anti-abuse rate + concurrency).
