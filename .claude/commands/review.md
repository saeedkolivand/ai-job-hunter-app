---
description: Strict pre-PR review of the diff — security gate (tauri-security-reviewer) first, then dual-model pr-reviewer (opus + sonnet) + verify/dedup pass (real tools + blast-radius)
argument-hint: [base-ref or PR# — defaults to diff vs origin/main]
---

Run the **strict internal pre-PR review** — the gate that runs BEFORE a PR is opened so CodeRabbit finds less.

1. Read `.claude/review-config.md` (path rules + learnings — do not re-raise the listed false positives).
2. Scope = the diff of `$ARGUMENTS` (a base ref like `main`/`develop`, or a PR#) — default `git diff origin/main...HEAD`. Stay inside the change's blast radius; pre-existing issues are out of scope unless the change endangers them.
3. **Security gate FIRST.** Before spawning pr-reviewer, run the security pass over the same scope: spawn the `tauri-security-reviewer` subagent (or invoke `/review-security`) — desktop/app/backend/AI/data/abuse/supply-chain lens. **HIGH/CRITICAL block** and must be resolved (route fixes to the owning domain author) before pr-reviewer runs; LOW/MEDIUM are advisory and carried into the final report. This runs on every PR, not just risk-flagged ones.
4. Spawn the `pr-reviewer` subagent **twice in parallel, on two different models** — one default (opus), one with `model: sonnet`. Different models catch different real bugs (research: ~93% of real defects are found by exactly one reviewer of several), so two diverse passes beat one. Each runs the full agent: the repo's real tools (typecheck, lint:strict, cargo clippy/fmt/test, gen:ipc:check, secret scan, targeted tests), the cross-file blast-radius pass (codegraph callers/impact), the verification gate (substantiate or drop/⚠️), and the React 19 / TS / Tauri 2 + Rust invariant checks.
5. **Synthesize both reports into one** (this is the precision step — generation and judging stay separate): union the findings (including the carried-over security advisories); dedup by `file:line` + defect; for any finding only **one** reviewer raised, or that either marked **⚠️ Suspected**, re-verify it yourself (construct the triggering input or trace the exact path) and **drop it if you can't substantiate it**. Severity-rank the survivors 🔴/🟠/🟡/⚪ and emit a single verdict. **🔴 + 🟠 must be resolved before the PR goes up**; 🟡/⚪ advisory.
6. When a finding turns out to be a false positive, append it to `.claude/review-config.md` learnings so it isn't re-raised.
