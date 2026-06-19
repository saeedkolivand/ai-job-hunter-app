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

## External standards & best-practices (verified 2026-06-19)

> Only genuinely 2026-_changed_ item: **React Compiler 1.0** (2025-10-07) flips the default from hand-memoize to compiler-memoize. CWV thresholds + Tokio/SQLite discipline are stable.

- **Core Web Vitals** (p75): LCP ≤ **2.5s** · INP ≤ **200ms** · CLS ≤ **0.1** (INP replaced FID 2024; ignore SEO claims of a tightened 2.0s LCP). Field via web-vitals lib; lab via Lighthouse. https://web.dev/articles/vitals
- **React** — with Compiler on, **don't add (or rip out) manual `useMemo`/`useCallback`/`memo`**; without it, memoize only after a _measured_ >16ms render. Virtualize long/unbounded lists (TanStack Virtual → ~viewport-only DOM); route-level code-split. https://react.dev/learn/react-compiler/introduction
- **Rust/Tokio** — never block the executor; offload std-blocking / >~1ms-CPU to `spawn_blocking` (short) or a dedicated thread (long); **bound CPU concurrency with a `Semaphore`**; `spawn_blocking` tasks can't be aborted and block shutdown (`shutdown_timeout`). Profile: `cargo flamegraph`/`samply` (CPU), DHAT/heaptrack (heap), `tokio-console` (async stalls). https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
- **SQLite** — `journal_mode=WAL` + `synchronous=NORMAL` (loses only durability on power-loss); `cache_size` (negative = KiB), `mmap_size` 30–256MB read-heavy, `temp_store=MEMORY`; `busy_timeout` (e.g. 5000) to avoid `SQLITE_BUSY`; index WHERE/JOIN/ORDER-BY cols; prepared/bound statements; **SQLite calls block → run on a pool inside `spawn_blocking`.** https://www.sqlite.org/pragma.html

**Common mistakes:** hand-memoizing everything (or removing existing memo) with Compiler on; rendering full lists instead of virtualizing (INP/jank); sync SQLite/`reqwest::blocking`/`std::fs` on a tokio worker; unbounded `spawn_blocking` / parallel CPU (pool exhaustion); leaving `synchronous=FULL` under WAL; trusting SEO "2.0s LCP" over web.dev; micro-opt without a flamegraph.
