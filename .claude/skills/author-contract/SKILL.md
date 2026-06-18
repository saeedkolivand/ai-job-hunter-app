---
name: author-contract
description: Shared write-side contract every domain AUTHOR imports — how to implement reliably, the smallest-diff rule, validation gates, and the never-approve-your-own-work rule. The write-side mirror of token-efficiency. Load at the start of any implementation task.
---

# Author contract (all write-capable agents)

The write-side mirror of `token-efficiency`. Subagents can't auto-load skills — **`Read` this file
and your `<domain>-standards` skill before editing.**

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

- Read the **handoff file** (`.claude/scratch/<task>.md`) the orchestrator pre-harvested — do **not**
  cold re-explore what it already contains.
- Context priority: **graphify** (semantic) / **codegraph** (structural) → source → docs/knowledge →
  lessons. Run `codegraph callers/callees/impact <symbol>` before touching a shared symbol. No
  repo-wide scans; stop at ~90% confidence.

## You never approve your own work (the independence rule)

An agent judging its own output doesn't reliably improve (it shares its own blind spot). So:

- Implement, then **hand the diff to your independent sibling critic** (and the test pair) — never
  self-approve. Resolve every HIGH/CRITICAL before "done"; LOW/MEDIUM are advisory.
- Append what you changed (files, decisions, open questions, `Lessons-to-propose`) to the handoff file
  so the critic and `project-steward` don't re-derive it.

## Leave a check behind

Non-trivial logic (a branch, loop, parser, money/security/error path) ships **one runnable check** —
a unit/integration test (via `test-author`) or a minimal self-check. Trivial one-liners don't (YAGNI).

## Validate before "done" (hard gate)

Run the relevant: per-package `tsc --noEmit` / `pnpm -F <pkg> typecheck`, `pnpm test`,
`cargo check`/`cargo test`/`cargo clippy` for `apps/tauri/src-tauri`. Anything red → revert that change
and report what + why. End with a short summary: files touched, issues resolved, anything left for the
critic. Propose durable lessons as `LESSON · <category> · Context/Decision/Outcome` (only
`project-steward` persists them).
