---
description: Fix a bug with a minimal change + regression test, under the review-workflow
argument-hint: <bug description or issue#>
---

Fix: **$ARGUMENTS**

1. Load `review-workflow` + `token-efficiency`.
2. **Reproduce / locate** — graphify-scope to the failing area; read the authoritative source. **Stop at ~90% confidence.**
3. **Root cause** — state it in one line before editing.
4. **Minimal fix** on a feature branch — change the cause, not the symptom; no scope creep.
5. **Regression test** (if `touchesTestableLogic`): `test-author` adds a failing-then-passing test → `testing-reviewer` audits.
6. **Review** — Primary Owner of the area (+ Secondary on risk, ≤3 reviewers); HIGH/CRITICAL block.
7. **Verify** the fix (run the test/build); **docs/lessons** via `project-steward` if the cause is a durable lesson.
8. Open a PR.
