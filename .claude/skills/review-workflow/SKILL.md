---
name: review-workflow
description: The implement→review flow under the ≤3-reviewer budget with the conditional test stage. Use for /implement-feature, /fix-bug, /refactor-module and any change that needs structured review.
---

# Review workflow

1. **Analyze** the request; identify affected files (graphify/codegraph first, not repo scans). **Pre-harvest** the paths + signatures into a handoff file `.claude/scratch/<task>.md` (copy `HANDOFF_TEMPLATE.md`) so no stage cold-re-explores.
2. **Select the pair** by Ownership precedence: the area's **author** (implements) + its independent **critic**, plus a **Secondary critic** only if the change is risk-bearing in that column (`tauri-security-reviewer`, `performance-profiler`, `ui-ux-expert`). **≤3 critics** total.
3. **Plan** the minimal change (in the handoff).
4. **Implement** — the **domain author** makes minimal changes (reads the handoff first; loads `author-contract`). **Rust-first** for business logic / pipelines / ATS / document generation; the renderer stays presentation-focused. The author appends what changed to the handoff.
5. **Test stage (conditional)** — if `touchesTestableLogic(diff)` (Part D predicate): `test-author` writes/updates tests → `testing-reviewer` audits **coverage of the changed code**. This stage is **separate from the ≤3-critic cap**. **STRICT:** changed non-trivial logic without a test — or a weak/tautological/mock-asserting test that doesn't exercise the change — is **HIGH (blocking)**, not advisory; the error/edge path must be covered, not just the happy path.
6. **Review** — the independent **critic** (never the author) audits the diff against its checklist and appends severity-tagged findings to the handoff; **HIGH/CRITICAL block** → author resolves → re-audit; LOW/MEDIUM advisory. For genuinely parallel, file-disjoint, multi-domain work, run this as native **Agent Teams** (the lead spawns authors as teammates owning disjoint files; critics challenge via the mailbox); otherwise sequential subagents (cheaper).
7. **Verify correctness** — run the relevant tests/build (`rtk pnpm test`, `cargo test`).
8. **Verify performance** — if a hot path was touched → `performance-profiler`.
9. **Verify security** — if risk-bearing → `tauri-security-reviewer` (HIGH/CRITICAL blocks).
10. **Docs + lessons** — `project-steward` syncs affected docs/knowledge, runs `graphify update .`, and persists any durable lesson.

No feature is "done" without tests (when the predicate is positive) or without docs sync.
