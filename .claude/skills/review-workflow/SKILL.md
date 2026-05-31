---
name: review-workflow
description: The implement→review flow under the ≤3-reviewer budget with the conditional test stage. Use for /implement-feature, /fix-bug, /refactor-module and any change that needs structured review.
---

# Review workflow

1. **Analyze** the request; identify affected files (graphify first, not repo scans).
2. **Select reviewers** by Ownership precedence: the **Primary Owner** for the area, plus a **Secondary** only if the change is risk-bearing in that column (e.g. `tauri-security-reviewer`, `performance-profiler`). **≤3 reviewers** total.
3. **Plan** the minimal change.
4. **Implement** minimal changes — **Rust-first** for business logic / pipelines / ATS / document generation; the renderer stays presentation-focused.
5. **Test stage (conditional)** — if `touchesTestableLogic(diff)` (Part D predicate): `test-author` writes/updates tests → `testing-reviewer` audits **coverage of the changed code**. This stage is **separate from the ≤3-reviewer cap**.
6. **Review** — selected reviewer(s) emit severity-tagged findings; **HIGH/CRITICAL block**, LOW/MEDIUM advisory.
7. **Verify correctness** — run the relevant tests/build (`rtk pnpm test`, `cargo test`).
8. **Verify performance** — if a hot path was touched → `performance-profiler`.
9. **Verify security** — if risk-bearing → `tauri-security-reviewer` (HIGH/CRITICAL blocks).
10. **Docs + lessons** — `project-steward` syncs affected docs/knowledge, runs `graphify update .`, and persists any durable lesson.

No feature is "done" without tests (when the predicate is positive) or without docs sync.
