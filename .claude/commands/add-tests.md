---
description: Author tests with test-author, then audit with testing-reviewer
argument-hint: [files/feature to cover — defaults to current git diff]
---

Add tests for: **$ARGUMENTS** (or the current `git diff`).

1. Load `testing-rules` + `token-efficiency`.
2. Spawn the `test-author` subagent (Task) — it writes tests (integration→unit→e2e; golden for PDF/DOCX; realistic fixtures over mocks; cover success + failure + **error/security paths** + edge cases). Reuse `renderer/test-support.tsx` utilities and `src-tauri/tests/` conventions.
3. Then spawn `testing-reviewer` (Task) to audit **coverage of the changed code** + test quality (weak assertions, flakiness, over-mocking, redundancy). It NEVER writes tests.
4. Run the tests (`rtk pnpm test` / `cargo test`); ensure green. Report any HIGH/CRITICAL coverage gap as blocking.

(`/add-tests` always runs `test-author` regardless of the testable-logic predicate — tests were explicitly requested.)
