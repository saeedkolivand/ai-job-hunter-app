# Anti-Abuse Rate & Concurrency Limits

Canonical source: `apps/desktop/src-tauri/src/limits/mod.rs`

## Overview

The `limits` module provides **in-memory rate limiting + concurrency control** on expensive, abuse-prone operations:

- `ai_generate`: AI inference (cost, latency)
- `scrape_boards` / `scrape_url`: Web scraping (target rate-limits, IP bans)

The limiter is **process-scoped** (in-memory; resets on app restart) and operates at the **command boundary** (right after deserialization, before business logic). **Multi-board batch limit**: server-side cap enforced by `max_boards_per_batch()` in `apps/desktop/src-tauri/src/scraping/engine/mod.rs` prevents unbounded request amplification from crafted IPC payloads. The engine-level bound scales automatically as new boards are added to the registry (see `max_boards_per_batch()` source; no `scraping/engine` code edit required). Note: the shared Zod schemas in `packages/shared/src/schemas/index.ts` (ScrapeBoardsRequestSchema `.max(BOARD_IDS.length)`) independently bound request size at the IPC boundary; scaling is subject to both limits.

## Design

### Components

**`RateLimiter`** (generic, sync):

```rust
pub struct RateLimiter {
    max_concurrent: Arc<Mutex<u32>>,
    per_second_semaphore: Arc<Semaphore>,
    daily_usage: Arc<DashMap<String, DailyBucket>>,
}
```

- **Concurrent ops limit**: prevents unbounded parallel work (default 8–16 ops, configurable per operation).
- **Per-second rate**: sliding window (generous default; prevents request storms).
- **Daily ceiling**: per-provider cap (e.g., Anthropic API monthly quota proxy).

### Error

When a limit is exceeded, the command returns `AppError::RateLimited(H13)`:

```rust
pub enum AppError {
    // ...
    RateLimited(String), // "Concurrent limit exceeded" | "Daily limit reached for provider X"
}
```

The renderer can catch `H13`, display a user-facing message, and retry after a delay.

## Usage

### AI Commands

Applied to `ai_generate` in `commands/ai.rs`:

```rust
#[tauri::command]
pub async fn ai_generate(
    request: GenerateRequest,
    state: State<'_, AppState>,
) -> AppResult<()> {
    // Apply rate limit before ANY work
    state.limits.check_ai_generate(request.provider_id).await?;

    // Proceed with generation
    // ...
}
```

### Scraping Commands

Applied to `scrape_boards` / `scrape_url` in `commands/scrape.rs`:

```rust
#[tauri::command]
pub async fn scrape_boards(
    request: ScrapeBoardsRequest,
    state: State<'_, AppState>,
) -> AppResult<()> {
    // Apply rate limit per board
    for board in &request.boards {
        state.limits.check_scrape(board).await?;
    }

    // Engine enforces multi-board batch cap via max_boards_per_batch() in scraping/engine/mod.rs
    // See: apps/desktop/src-tauri/src/scraping/engine/mod.rs::max_boards_per_batch (CWE-770 defense)
    // ...
}
```

## Configuration

**Current defaults** (in `limits/mod.rs`):

| Limit                          | Default | Rationale                         |
| ------------------------------ | ------- | --------------------------------- |
| `max_concurrent_ai_generate`   | 8       | Prevent 100 parallel requests     |
| `max_concurrent_scrape`        | 4       | Respect board rate limits         |
| `ai_generate_per_second`       | 10      | Sliding window; bursty is ok      |
| `scrape_per_second`            | 5       | Per-board; respect rate-limit hdr |
| `daily_ceiling_anthropic_text` | 100k    | Proxy for Anthropic monthly quota |
| `daily_ceiling_openai`         | 50k     | Conservative estimate             |

All are **runtime-configurable** via `system_set_performance_mode` (the PerformancePreferences component sets bounds dynamically).

## Related

- **Performance mode** ([`PATTERNS.md` § 11](../PATTERNS.md#11-performance-mode-pattern)): how to adjust limits at runtime.
- **ARCHITECTURE_STATUS.md**: Anti-abuse limits status.
- **PATTERNS.md § 13**: `limits` as a module owner (anti-abuse rate + concurrency).
