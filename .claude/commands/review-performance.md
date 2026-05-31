---
description: Performance review with performance-profiler
argument-hint: [files or PR# — defaults to current git diff]
---

Run a **performance** review.

1. Load the `token-efficiency` + `performance-checklist` skills; read `docs/knowledge/performance-rules.md`.
2. Scope with graphify; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff`.
4. Spawn the `performance-profiler` subagent (Task) over the target — hot paths only (async-runtime blocking, scraping concurrency, embeddings/streaming, layout/export, SQLite, large lists, token/cost).
5. Report severity-tagged findings; bias toward MEDIUM/advisory unless a concrete on-a-hot-path regression; **HIGH/CRITICAL block**.
