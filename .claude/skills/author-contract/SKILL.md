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
env-mutating test, and no real network. These only run on that OS's CI runner — #486 lost four CI
round-trips to exactly this.

## Validate before "done" (hard gate — MANDATORY, no exceptions)

This is not optional and you do **not** declare done on assumption — **run** the relevant gate and
**verify** it green with your own eyes: per-package `tsc --noEmit` / `pnpm -F <pkg> typecheck`,
`pnpm test`, `cargo check`/`cargo test`/`cargo clippy` for `apps/tauri/src-tauri`. Run the **exact**
gate command (per-package, `--force` where caching can mask failures) — a wrapped/cached "no errors"
is not proof. Anything red → revert that change and report what + why. **Cross-OS caveat:** a same-host
`cargo`/`pnpm` build **silently excludes** `#[cfg(target_os=…)]` code for other targets — a green local
run does NOT verify it; cross-target-check (`cargo check --target <triple>`) any OS-gated code you touch,
or flag it unverified-on-this-host for CI. Never hand a red or unverified diff to the critic. End with a short summary: files touched, issues resolved, anything left for the
critic. Propose durable lessons as `LESSON · <category> · Context/Decision/Outcome` (only
`project-steward` persists them).
