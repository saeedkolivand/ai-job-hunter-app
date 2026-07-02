---
name: author-contract
description: Shared write-side contract every domain AUTHOR imports — how to implement reliably, the smallest-diff rule, validation gates, and the never-approve-your-own-work rule. The write-side mirror of token-efficiency. Load at the start of any implementation task.
---

# Author contract (all write-capable agents)

The write-side mirror of `token-efficiency`. Subagents can't auto-load skills — **`Read` this file
and your `<domain>-standards` skill before editing.**

> **Model tier:** authors default to **Sonnet**. If a change is genuinely beyond a Sonnet pass — deep Rust concurrency/`unsafe`, a new provider's streaming protocol, a schema/data migration — flag it up front so the orchestrator re-spawns you on Opus instead of thrashing.

## Implement like a lazy senior dev

- **Smallest diff per issue.** Preserve behavior and public/package APIs. One concern per edit.
- **Rust-first** for business logic / pipelines / ATS / document generation; the renderer stays
  presentation-focused.
- **Reuse before adding** — an existing service hook, `@ajh/ui` primitive, registry, or helper beats
  new code. Never invent an abstraction the standards' "do not over-apply" section would reject;
  under-abstraction beats the wrong abstraction.
- Never reformat untouched lines; never rename across package boundaries unprompted.
- State a one-line plan before a large or multi-file refactor and pause for confirmation; small in-file
  fixes proceed.

## Ground first (token-efficiency)

- **Codegraph FIRST — hard rule.** Before ANY raw `Grep`/`Read` exploration, query codegraph for the
  symbols/files in scope (MCP `codegraph_explore` when your allowlist has `mcp__codegraph`, else the
  CLI `codegraph callers/callees/impact/query`) — one call returns the verbatim source plus callers
  and blast radius. Raw file reads are reserved for what codegraph can't answer (un-indexed edits
  from this turn, non-code assets, config prose).
- Read the **handoff file's `## Current state` section** (`.claude/scratch/<task>.md`) the
  orchestrator pre-harvested — do **not** cold re-explore what it already contains (the `## Log`
  below it is steward-only).
- Context priority: **graphify** (semantic) / **codegraph** (structural) → source → docs/knowledge →
  lessons. No repo-wide scans; stop at ~90% confidence.

## You never approve your own work (the independence rule)

An agent judging its own output doesn't reliably improve (it shares its own blind spot). So:

- Implement, then **hand the diff to your independent sibling critic** (and the test pair) — never
  self-approve. Resolve every HIGH/CRITICAL before "done"; LOW/MEDIUM are advisory.
- Append what you changed (files, decisions, open questions, `Lessons-to-propose`) to the handoff's
  `## Log`, then rewrite the stale parts of `## Current state` (≤2K chars) so the critic and
  `project-steward` don't re-derive it.

## Leave a check behind (STRICT — missing tests now BLOCK)

Non-trivial logic (a branch, loop, parser, money/security/error path) ships **one runnable check** —
a unit/integration test (via `test-author`) or a minimal self-check. Trivial one-liners don't (YAGNI).
**The bar was raised:** changed non-trivial logic shipped **without** a test — or with a test whose
assertion is weak / tautological / asserts the mock / doesn't exercise the change — is now a **HIGH
(blocking)** finding for your critic. Cover the error/edge path, not just the happy path. New/changed
user-facing text must have its i18n key added to **both** `en` and `de` (also HIGH).
**Hermetic tests (cross-OS) — obey `testing-rules` whichever `<domain>-standards` you loaded:** a
`#[cfg(target_os=…)]` test must be hermetic (inject dirs / a temp `HOME`, never assume a system
binary/lib like `/usr/bin/google-chrome` or `libwayland` is _absent_, never reach an
`exec()`/process-replacing path in-process), `#[serial_test::serial]` (fully-qualified) every
env-mutating test, and no real network. These only run on that OS's CI runner.

## Validate before "done" (hard gate — MANDATORY, no exceptions)

This is not optional and you do **not** declare done on assumption — **run** the relevant gate and
**verify** it green with your own eyes: `tsc --noEmit` / `pnpm typecheck`, `pnpm test`,
`cargo check`/`cargo test`/`cargo clippy` for `apps/desktop/src-tauri`. Anything red → revert that change
and report what + why.

**Your "green" must match the bar that GATES — the pre-push/CI scope, not a narrower one** (this class
cost real failed-push cycles):

- **Scope ≥ the gate.** A `pnpm -F <pkg> test`/`typecheck` is for fast iteration only — it does **not**
  run sibling packages' tests. Any change touching `packages/shared`/the IPC contracts, **or any
  cross-package public API**, must pass the **whole-graph** `pnpm test` (a shared "every expected
  namespace" enumeration test lives in `@ajh/shared`, not in `@ajh/desktop` — a scoped run never sees it).
- **Force past the cache.** Report green from `TURBO_FORCE=1 pnpm typecheck` (or `--force`) — the exact
  command the pre-push runs. A cached/stale `^build` can mask a real type error; a wrapped "no errors" is
  not proof.
- **`tsc` after every test edit — `vitest` is NOT a typecheck.** vitest runs on esbuild and does **zero**
  type-checking, so `getByTestId(...).value`/`.checked` casts and `noUncheckedIndexedAccess` violations
  pass `pnpm test` yet fail CI's `tsc`. After writing/editing ANY test file, run `pnpm typecheck` — a
  green test run is not a green typecheck. (Cast `get*By*` results to `HTMLInputElement` before
  `.value`/`.checked`; guard indexed access — no `!`.) **Cross-OS caveat:** a same-host
  `cargo`/`pnpm` build **silently excludes** `#[cfg(target_os=…)]` code for other targets — a green local
  run does NOT verify it; cross-target-check (`cargo check --target <triple>`) any OS-gated code you touch.
  If your host genuinely can't build that target, say so explicitly in the handoff (`cross-OS-unverified — CI
runs it`) — that labeled exception is the ONLY unverified hand-off allowed; otherwise never hand a red or
  unverified diff to the critic. End with a short summary: files touched, issues resolved, anything left for the
  critic. Propose durable lessons as `LESSON · <category> · Context/Decision/Outcome` (only
  `project-steward` persists them).
