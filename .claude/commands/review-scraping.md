---
description: Scraping/applier review with scraping-applier-expert (Primary Owner)
argument-hint: [files or PR# — defaults to current git diff]
---

Run a **scraping / applier** review (selector resilience, registries, browser automation, reliability, rate-limiting, cancellation).

1. Load the `token-efficiency` + `automation-standards` skills; read `docs/knowledge/automation-domain.md`.
2. Scope with graphify; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff` under `scraping/`, `applying/`.
4. Spawn **only** the `scraping-applier-expert` subagent (Task) as Primary Owner. Add `tauri-security-reviewer` (cookies/sessions/egress) or `performance-profiler` (concurrency) as Secondary only on risk — **≤3 reviewers**.
5. Report severity-tagged findings; **HIGH/CRITICAL block**.
