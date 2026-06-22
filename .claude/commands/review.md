---
description: Strict pre-PR review of the diff with the pr-reviewer subagent (runs real tools + blast-radius)
argument-hint: [base-ref or PR# — defaults to diff vs origin/main]
---

Run the **strict internal pre-PR review** — the gate that runs BEFORE a PR is opened so CodeRabbit finds less.

1. Read `.claude/review-config.md` (path rules + learnings — do not re-raise the listed false positives).
2. Scope = the diff of `$ARGUMENTS` (a base ref like `main`/`develop`, or a PR#) — default `git diff origin/main...HEAD`. Stay inside the change's blast radius; pre-existing issues are out of scope unless the change endangers them.
3. Spawn **only** the `pr-reviewer` subagent (Task). It will: run the repo's real tools (typecheck, lint:strict, cargo clippy/fmt/test, gen:ipc:check, secret scan, targeted tests), do the cross-file blast-radius pass (codegraph callers/impact), apply the verification gate (substantiate or drop/⚠️), and run the React 19 / TS / Tauri 2 + Rust invariant checks.
4. Report severity-tagged findings 🔴/🟠/🟡/⚪ with a verdict. **🔴 + 🟠 must be resolved before the PR goes up**; 🟡/⚪ advisory.
5. When a finding turns out to be a false positive, append it to `.claude/review-config.md` learnings so it isn't re-raised.
