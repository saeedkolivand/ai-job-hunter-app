---
name: testing-rules
description: Testing standards — frameworks, the testable-logic trigger, golden/snapshot rules, mocking rules, and the test-author → testing-reviewer pipeline. Load for /add-tests and any change that touches testable logic.
---

# Testing rules

## Frameworks & utilities

- **Frontend** — Vitest + @testing-library/react, colocated `*.test.ts(x)`. Reuse `renderer/test-support.tsx`: `createMockClient`, `renderHookWithClient`, `exerciseServiceHooks`.
- **Rust** — `cargo test`; integration in `src-tauri/tests/` (incl. `cargo test --test architecture` boundary guard).
- **E2E / golden** — golden snapshots for PDF/DOCX/template rendering.

## When tests are required (testable-logic predicate)

A change requires authoring tests iff a changed `.rs`/`.ts`/`.tsx` file (not test/generated/declaration/config) has a **behavioral** line change (not pure comment/blank/import/type-decl). Pure deletions don't trigger authoring — instead verify orphaned tests were removed.

## Strategy

- Order: **integration → unit → e2e**. Test behavior, not implementation.
- **Coverage** of changed code: success + failure + **error & security paths** (untested error/security path on changed code = HIGH/blocking) + edge cases + validation.

## Mocking

- Allowed: external APIs, AI providers, third-party, expensive ops.
- **Never mock** internal business logic, ATS scoring, resume generation, or export pipelines — use realistic fixtures.

## Golden/snapshot

Deterministic, reviewed when updated, prevents visual regressions. A non-deterministic snapshot is a finding.

## Pipeline

**Feature Owner → `test-author` (writes) → `testing-reviewer` (audits, never writes).** Separate from the ≤3-reviewer cap.
