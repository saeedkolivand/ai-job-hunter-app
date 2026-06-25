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
- **Prove the guard is real (red-green).** A test locking a bugfix must FAIL on the unfixed code and PASS on the fix — confirm it (temporarily revert the fix, or assert the exact sentinel/branch the fix introduced). A regression test that passes both ways guards nothing; CodeRabbit/CI caught several this run that did exactly that (a mocked dependency hid the real failure path).

## Mocking

- Allowed: external APIs, AI providers, third-party, expensive ops.
- **Never mock** internal business logic, ATS scoring, resume generation, or export pipelines — use realistic fixtures.
- **Mock fidelity — a stub must reproduce the REAL side-effects of what it replaces.** A `vi.fn()` standing in for a fn that commits optimistic state (e.g. `updateAnswer` calling `setAnswers` BEFORE its async save) must reproduce that effect (update the controlled prop/state in the test) — otherwise the test passes against logic production would break. A rollback-guard bug shipped green precisely because the mocked `updateAnswer` never updated state, so the guard's sentinel was never exercised. If a stub can't reproduce the real effect, render the real unit and mock only the leaf (network/provider/IPC).

## Cross-OS / cfg-gated & environment tests (each of these cost a CI round-trip on #486)

- **`#[cfg(target_os = …)]` tests run only on that OS's CI runner** — never on a Windows/macOS dev host. A green local `cargo test` does **not** cover them; cross-target-check the gated module (`cargo check --target <triple> --tests`, or a dep-light standalone-crate check) before claiming done.
- **Never assume the runner lacks a system binary/lib** — CI runners ship `/usr/bin/google-chrome`, `libwayland-client.so.0`, etc. A test that passes only because the host has _no_ native browser / _no_ host lib is env-fragile. Drive the code with an injected dir / temp `HOME` and assert the **decision**, not the ambient system.
- **Never reach a process-replacing path in-process** — a test that calls a fn which may `exec()`/re-exec replaces or loops the test binary and **cancels the CI job**. Extract the exec-free decision (pure fn / enum) and assert that; leave the `exec()` tail intentionally uncovered.
- **`#[serial]` every env-mutating test** — `std::env::set_var` is not thread-safe and cargo runs tests in parallel. Use fully-qualified `#[serial_test::serial]` (so it resolves inside a `mod` submodule) + restore/temp-`HOME`.

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
