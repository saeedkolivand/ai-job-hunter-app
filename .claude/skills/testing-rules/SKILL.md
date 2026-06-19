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

## External standards & best-practices (verified 2026-06-19)

> Tooling baseline: **Vitest 4.0** (GA 2025-10-22) + Testing Library current.

- **Test like a user** — assert behavior, not internals; don't test implementation details (shallow render, internal-state probing) → false confidence + refactor breakage. https://kentcdodds.com/blog/testing-implementation-details
- **Query priority** — `getByRole`(`{name}`) → `getByLabelText` → `getByText` → … → **`getByTestId` last resort**. Can't reach by role? The UI is likely inaccessible — fix the markup. https://testing-library.com/docs/queries/about/
- **Async** — `findBy*`/`waitFor`, never manual sleeps; never assert before awaiting; no `act()` warnings; for negative async assertions, wait until the side effect _could_ have run, then assert it didn't.
- **Shape** — weight integration tests (best confidence/speed ROI), static analysis as the base, thin E2E on top. https://kentcdodds.com/blog/the-testing-trophy-and-testing-classifications
- **Flakiness** (~45% async-wait, ~20% races, ~12% order): run shuffled with a logged seed (Vitest `sequence.shuffle`/`seed`); fake the clock (`vi.useFakeTimers`/`setSystemTime`), seed + log RNG, freeze animations; each test sets up/tears down its own data; reset mocks/timers between tests; no real network. https://vitest.dev/config/
- **Golden/snapshot** — proves _something changed_, not that it's _correct_ → pair with behavioral assertions; keep small + reviewed (snapshot fatigue hides regressions); best for binary/visual (PDF/DOCX/images). Rust: `insta` + `proptest` (seeds deterministic). https://percy.io/blog/snapshot-testing
- **Coverage** — branch/path > line %; don't test trivial getters/3rd-party/generated/types/config. Vitest 4 V8 provider = Istanbul-accurate branches at V8 speed. https://vitest.dev/guide/coverage.html
- **2026 Vitest 4 flags** — Browser Mode stable (provider pkgs e.g. `@vitest/browser-playwright`); new `toMatchScreenshot`/`toBeInViewport`/`expect.schemaMatching`; reworked module-mock semantics; `basic` reporter removed. https://vitest.dev/blog/vitest-4

**Common mistakes:** asserting on implementation details/internal state; `getByTestId` first instead of role/label; missing `await` on `findBy`/`waitFor`; over-mocking → green-but-broken; weak assertions (`toBeTruthy`, snapshot-only) passing on wrong output; blind snapshot updates; order-dependent tests masked by fixed run order; chasing a coverage % by testing trivial code.
