---
name: performance-checklist
description: Performance review checklist — hot paths, async-runtime discipline, streaming, query-client tuning, layout pre-measurement, token/cost. Load when reviewing perf-sensitive changes.
---

# Performance checklist

Authoritative: `docs/knowledge/performance-rules.md`. Bias perf findings toward MEDIUM unless there's a concrete on-a-hot-path regression.

## Async runtime (HIGH if blocking)

- No blocking I/O or CPU-bound work on the tokio runtime — use `spawn_blocking` / the data layer. SQLite never blocks the async runtime.

## Hot paths

- **Scraping** — bounded concurrency, chromiumoxide page lifecycle reused, per-board rate limits.
- **Embeddings / AI** — batch sizing, streaming back-pressure, token/context budget minimized, cheapest viable model.
- **Layout / export** — pre-measure before render; don't re-shape fonts per glyph; pagination computed once.
- **Data** — no N+1 queries, no full-table scans on warm paths.

## Renderer

- React Query `staleTime`/`gcTime` tuned (desktop: no refetch-on-focus); large lists virtualized; avoid needless re-renders (stable deps, memo where measured).

## Token efficiency

- Minimize prompt/context size in AI calls; reuse cached/embedded context; don't resend unchanged context.
