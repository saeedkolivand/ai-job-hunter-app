---
description: Implement a feature end-to-end under the review-workflow (Primary Owner + conditional test stage + docs sync)
argument-hint: <feature description>
---

Implement: **$ARGUMENTS**

Follow the `review-workflow` skill exactly:

1. Load `review-workflow` + `token-efficiency` + `coding-standards`.
2. **Analyze** — graphify/codegraph-scope affected files; identify the area's **author + critic pair** (Ownership precedence). **Pre-harvest** into `.claude/scratch/<task>.md`. **Stop at ~90% confidence.**
3. **Plan** the minimal change (Rust-first for business logic; new IPC capability → the 5-file flow in `tauri-standards`).
4. **Implement** — the domain **author** (loads `author-contract`) makes minimal changes on a feature branch (PRs only — never push to `main`); appends what changed to the handoff.
5. **Test stage** (if `touchesTestableLogic`): `test-author` writes tests → `testing-reviewer` audits coverage of the changed code.
6. **Review** — the independent **critic** (never the author) audits (+ Secondary only on risk, ≤3 critics); HIGH/CRITICAL block → author resolves → re-audit.
7. **Verify** correctness (`rtk pnpm test` / `cargo test`), performance & security where applicable.
8. **Docs + lessons** — `project-steward` syncs affected docs/knowledge, runs `graphify update .`, persists any durable lesson.
9. Open a PR (`gh pr create`) and wait for approval.
