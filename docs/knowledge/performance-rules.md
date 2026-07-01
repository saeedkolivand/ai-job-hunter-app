# Performance rules (hot paths + discipline)

Last updated: 2026-06-01

For `performance-profiler` (Secondary). Bias findings toward MEDIUM unless there's a concrete on-a-hot-path regression. Use `graphify` to locate hot code; measure, don't guess.

## Async-runtime discipline (HIGH if blocking)

- No blocking I/O or CPU-bound work on the [Tokio][tokio] runtime. [SQLite][sqlite] (`db.rs`, `data_store.rs`) and other blocking work go through `spawn_blocking` / the data layer — never block the async runtime.

## Hot paths

- **Scraping** (`scraping/`, `browser/`) — bounded concurrency, reuse [chromiumoxide][chromiumoxide] pages where possible, per-board rate limits, don't spawn unbounded tasks.
- **AI / embeddings** (`commands/ai_provider/`, `documents/`) — batch sizing, streaming back-pressure, minimal token/context, cheapest viable model; avoid re-embedding unchanged content.
- **Export / layout** (`export/`, `layout/`, `measure/`) — pre-measure before render; don't re-shape fonts per glyph; compute pagination once. PDF fonts are glyph-subset at export time (`export/pdf_renderer/fonts.rs: parse_font`); size guardrail test catches regressions. See [ADR-008](decision-records/adr-008-pdf-glyph-subsetting.md).
- **Pipeline** (`pipeline/`, `autopilot/`) — bounded queues; backpressure; cancellation honored.
- **Data** — no N+1 queries, no full-table scans on warm paths.

## Renderer (`apps/desktop/src/renderer/`)

- [TanStack Query][tanstack-query] tuned for desktop (longer `staleTime`/`gcTime`, no refetch-on-focus — see `services/query-client/`).
- Large lists virtualized; avoid needless re-renders (stable deps, memo where measured).

## Token efficiency (applies to AI calls and to agents)

- Minimize prompt/context size; reuse cached/embedded context; never resend unchanged context. (The review-gate itself follows this: tiered, batched, prompt-cached.)

[tokio]: https://tokio.rs
[sqlite]: https://www.sqlite.org
[chromiumoxide]: https://github.com/mattsse/chromiumoxide
[tanstack-query]: https://tanstack.com/query
