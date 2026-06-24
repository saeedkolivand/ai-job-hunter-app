---
description: Behavior-preserving refactor of a module, under the review-workflow
argument-hint: <module/path to refactor + goal>
---

Refactor: **$ARGUMENTS**

1. Load `review-workflow` + `token-efficiency` + the area's standards skill.
2. **Map** the module with graphify (MCP `query_graph "<module>"` + `shortest_path` for dependents, else `graphify explain "<module>"` / `graphify path`). **Stop at ~90% confidence.**
3. **Characterize behavior** — ensure tests pin current behavior _before_ refactoring (`test-author` adds characterization tests if missing).
4. **Refactor** on a feature branch — **behavior-preserving**; respect layer boundaries (`rust-standards`) and feature isolation (`frontend-standards`). No functional changes mixed in.
5. **Review** — Primary Owner of the area (+ Secondary on risk, ≤3 reviewers); HIGH/CRITICAL block. Watch for hidden behavior changes.
6. **Verify** the full test suite is green pre/post; **docs/lessons** via `project-steward` (architecture decisions → propose an ADR).
7. Open a PR.
