---
name: author-contract
description: Shared write-side contract every domain AUTHOR imports — smallest-diff implementation, ground-first, validation gates, tests-are-blocking, and never-approve-your-own-work. The write-side mirror of token-efficiency. Load at the start of any implementation task.
---

# Author contract (all write-capable agents)

Subagents can't auto-load skills — **`Read` this file and your `<domain>-standards` skill before editing.**

> **Model tier:** authors default to **Sonnet**. Flag genuinely-beyond-Sonnet work (deep Rust
> concurrency/`unsafe`, a new provider's streaming protocol, a schema/data migration) up front so the
> orchestrator re-spawns you on Opus instead of thrashing.

## Implement like a lazy senior dev

- **Smallest diff per issue.** Preserve behavior and public/package APIs. One concern per edit.
- **Rust-first** for business logic/pipelines/ATS/documents; the renderer stays presentational.
- **Reuse before adding** — an existing service hook, `@ajh/ui` primitive, registry, or helper beats
  new code; under-abstraction beats the wrong abstraction.
- Never reformat untouched lines; never rename across package boundaries unprompted.
- One-line plan before a large multi-file refactor, then pause for confirmation; small in-file fixes proceed.

## Ground first

- **Codegraph FIRST — hard rule.** Query codegraph (MCP `codegraph_explore` when allowed, else the
  CLI) before ANY raw Grep/Read; raw reads are for what codegraph can't answer (this turn's
  un-indexed edits, non-code assets, config prose).
- Read the handoff's `## Current state` (`.claude/scratch/<task>.md`) — never cold re-explore what
  it already contains (the `## Log` below it is steward-only).
- Priority: codegraph/graphify → source → docs/knowledge → lessons. No repo-wide scans; stop at ~90% confidence.

## You never approve your own work

Implement, then hand the diff to your **independent sibling critic** (and the test pair). Resolve
every HIGH/CRITICAL before "done"; LOW/MEDIUM are advisory. Append what you changed (files,
decisions, open questions, `Lessons-to-propose`) to the handoff `## Log`, then rewrite the stale
parts of `## Current state` (≤2K chars).

## Leave a check behind (missing tests BLOCK)

Non-trivial logic (a branch, loop, parser, money/security/error path) ships **one runnable check**
(via `test-author`); trivial one-liners don't (YAGNI). A missing test — or a weak / tautological /
mock-asserting one — is a **HIGH (blocking)** finding for your critic. Cover the error/edge path,
not just the happy path. New/changed user-facing text needs its i18n key in **both** `en` and `de`
(also HIGH). Hermetic cross-OS tests per `testing-rules`: inject dirs / a temp `HOME`,
`#[serial_test::serial]` on env-mutating tests, no real network, never assume a system binary is
absent, never reach an `exec()`-replacing path in-process.

## Validate before "done" (hard gate — MANDATORY)

Run the relevant gates and SEE them green: `pnpm typecheck`, `pnpm test`,
`cargo check`/`cargo test`/`cargo clippy` for `apps/desktop/src-tauri`. Anything red → revert that
change and report what + why. Your "green" must match the bar that GATES:

- **Scope ≥ the gate** — cross-package / IPC-contract / shared-API changes need the whole-graph
  `pnpm test`, not `-F <pkg>` (shared enumeration tests live outside the package you touched).
- **Force past the cache** — `TURBO_FORCE=1 pnpm typecheck`, the exact pre-push command.
- **`tsc` after every test edit** — vitest runs esbuild and type-checks NOTHING
  (`noUncheckedIndexedAccess`, `get*By*` casts fail only in `tsc`).
- **Cross-OS** — `cargo check --target <triple>` for any `#[cfg(target_os=…)]` code you touch; if
  the host can't build it, label `cross-OS-unverified — CI runs it` (the only allowed unverified
  handoff). Otherwise never hand a red or unverified diff to the critic.

End with a short summary: files touched, issues resolved, anything left for the critic. Propose
lessons as `LESSON · <category> · Context/Decision/Outcome` (only `project-steward` persists them).
