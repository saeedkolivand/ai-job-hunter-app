# Performance rules (hot paths + discipline)

For `performance-profiler` (Secondary). Bias findings toward MEDIUM unless there's a concrete on-a-hot-path regression. Use `graphify` to locate hot code; measure, don't guess.

## Async-runtime discipline (HIGH if blocking)

- No blocking I/O or CPU-bound work on the tokio runtime. SQLite (`db.rs`, `data_store.rs`) and other blocking work go through `spawn_blocking` / the data layer — never block the async runtime.

## Hot paths

- **Scraping** (`scraping/`, `browser/`) — bounded concurrency, reuse chromiumoxide pages where possible, per-board rate limits, don't spawn unbounded tasks.
- **AI / embeddings** (`commands/ai_provider/`, `documents/`) — batch sizing, streaming back-pressure, minimal token/context, cheapest viable model; avoid re-embedding unchanged content.
- **Export / layout** (`export/`, `layout/`, `measure/`) — pre-measure before render; don't re-shape fonts per glyph; compute pagination once.
- **Pipeline** (`pipeline/`, `autopilot/`) — bounded queues; backpressure; cancellation honored.
- **Data** — no N+1 queries, no full-table scans on warm paths.

## Renderer (`apps/tauri/src/renderer/`)

- React Query tuned for desktop (longer `staleTime`/`gcTime`, no refetch-on-focus — see `services/query-client/`).
- Large lists virtualized; avoid needless re-renders (stable deps, memo where measured).

## Token efficiency (applies to AI calls and to agents)

- Minimize prompt/context size; reuse cached/embedded context; never resend unchanged context. (The review-gate itself follows this: tiered, batched, prompt-cached.)
