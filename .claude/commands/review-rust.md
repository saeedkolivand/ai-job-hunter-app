---
description: Focused Rust/Tauri backend review with rust-backend-architect (Primary Owner)
argument-hint: [files or PR# — defaults to current git diff]
---

Run a focused **Rust backend** review.

1. Load the `token-efficiency` + `rust-standards` skills.
2. Scope with graphify (MCP `query_graph "rust backend architecture"`, else `graphify explain "rust backend architecture"`) — no repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff`.
4. Read `docs/knowledge/architecture.md` + `docs/architecture-rules.md` before source; **stop at ~90% confidence**.
5. Spawn **only** the `rust-backend-architect` subagent (Task) as Primary Owner over the target. Add `tauri-security-reviewer` and/or `performance-profiler` as Secondary **only** if the change is risk-bearing — **≤3 reviewers** total.
6. If the change touches testable logic, route to `test-author` → `testing-reviewer` (separate stage).
7. Report severity-tagged findings (`LOW/MEDIUM/HIGH/CRITICAL · file:line · fix`); **HIGH/CRITICAL block**.
