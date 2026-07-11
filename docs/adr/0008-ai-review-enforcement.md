---
status: accepted
---

# Mandatory AI review enforcement via deterministic schema-1 verdicts across four surfaces (Stop gate, /review, pre-push, CI)

## Context

Code review in the repo relied on two **disconnected** advisory paths:

1. **Weak auto gate** — the Stop review-gate (`.claude/hooks/review-gate.mjs`, injected by pre-commit) runs `claude -p` review locally, but `--no-verify` bypasses it habitually (especially after the Windows cargo-test entrypoint fault forced `git commit --no-verify` workarounds), leaving review unenforced. The gate also lacked deterministic verdict logic (model prose decided pass/fail).

2. **Strong opt-in** — `@claude review` on-demand review (tag-mode `claude-review.yml`) posts inline comments and is agency-aware, but it is never automatic and crucially **never blocks merge** — the owner can still commit/push without reviewing findings.

Meanwhile, **CodeRabbit** (ADR-0002) is deliberately on-demand per design, and the pre-push gate's conviction was limited (no cache of prior findings, no cross-run convergence tracking). Nothing AI was **mandatory** anywhere — the entire review system was advisory. Compounded by merge-blocking CI checks for linting + tests but optional AI findings review created an asymmetry: functional defects block merge, semantic issues don't.

This was a 6-PR program to harden the review pipeline:

- **PR 1 (review-gate)**: Move Stop gate to a deterministic schema-1 verdict script; separate parse/verdict contracts; add findings ledger + convergence tracking; document false-positive learning in `review-config.md`.
- **PR 2 (pre-push gate)**: Pre-push review gate with ratcheted enforcement (REVIEW_MODE env: `off`/`warn`/`fail`); skip patterns (REVIEW_SKIP); documented Windows entrypoint fault workaround.
- **PR 3 (gitleaks + cargo-test fix)**: Add gitleaks to CI; patch Windows cargo-test DLL fault to unblock the pre-push gate retest.
- **PR 4 (/review reformatted findings)**: Update `/review` command to emit schema-1 JSON; route findings via a new dedicated `/review-stats` (top findings per category, per file, per severity).
- **PR 5 (review-finding-verifier)**: New ast-grep Tier-0 pass before model review (scans for literal-string patterns known to produce FP); ratcheted confidence downgrade (0.8 → 0.5).
- **PR 6** (this): Server-side enforcement via the "🤖 AI Review OK" **required** check (`claude-review.yml` ai-review-gate job) — diff-based review with schema-1 verdict; fail-open on infra, fail-closed on findings at confidence ≥ 0.8; gitleaks added to `ci-ok` umbrella.

## Decision

Implement the final 6-PR surface: a **required CI status check** (`🤖 AI Review OK` in `claude-review.yml`) that runs on every PR (unless draft) and produces a deterministic verdict from schema-1 findings:

- **Trigger:** automatic on every PR open/sync/ready_for_review; no manual triggering needed (unlike the advisory `/review`).
- **Logic:** diff-based review (git diff origin/base...HEAD); path-scoped checklist matching (`.claude/review-routes.json` globs map to `.claude/review-checklists/<domain>.md` or fallback `.claude/agents/<owner>.md`); report defects only in changed code (`introduced_by_diff: true` by default).
- **Verdict:** deterministic in JS — parse schema-1 findings from the action step, extract blocking findings (HIGH or CRITICAL, confidence ≥ 0.8, introduced_by_diff ≠ false), post step-summary table, exit 1 if any blocking finding exists.
- **Fail semantics:**
  - **Fail-open on infra** — file missing/unparseable (e.g., action outage, model unavailable, no CLAUDE_CODE_OAUTH_TOKEN) → `::warning::` + exit 0 (do not freeze merges on infra failure; ci-ok + pre-push + CodeRabbit still gate).
  - **Fail-closed on findings** — HIGH/CRITICAL at confidence ≥ 0.8 → exit 1 (block merge).
  - **Advisory findings** — LOW/MEDIUM/pre-existing issues / confidence < 0.8 → posted to summary, never block.
- **Concurrency & draft skip** — concurrency-cancel per PR (only one review per PR running at a time); draft PRs skip the gate entirely (no quota spend on WIP).
- **Verdict script:** new `scripts/ci-review-verdict.mjs` (deterministic gate, shared with Stop gate + pre-push, reusing `review-lib.mjs` from the 6-PR program).
- **Gitleaks:** add `secret-scan` job to `ci-pipeline.yml`, running gitleaks on all PRs; add to `ci-ok` umbrella's always-on jobs list (never path-filtered, never skippable).

## Consequences

- **Amends ADR-0002's "✅ CI OK is the sole required check" clause** — now **TWO required checks exist**: `✅ CI OK` (linting, testing, building, secrets + gitleaks) and `🤖 AI Review OK` (semantic findings). Both must pass for merge (unless the admin overrides the ruleset). ADR-0002's on-demand CodeRabbit stance is preserved; CodeRabbit remains advisory (no merge block).
- **Amends ADR-0003's gate-isolation principle** — gitleaks now joins the `ci-ok` umbrella (part of the always-on functional gate, not a separate surface); the AI check stays separate. Both are required.
- **Four review surfaces now exist:**
  1. **Stop gate** (local pre-commit) — schema-1 verdict, ratcheted enforcement (REVIEW_MODE env).
  2. **Pre-push gate** (local) — schema-1 verdict, skip patterns (REVIEW_SKIP).
  3. **CI gate** (this PR) — schema-1 verdict, auto-run, **required check**, fail-open on infra.
  4. **/review command** (on-demand) — advisory deep dive, agent-routed, inert until invoked.
  - **CodeRabbit** (external, advisory per ADR-0002) — semantic review, labels, no merge block.
- **Quota spend per synchronize** is capped by concurrency-cancel (only one review per PR at a time) + draft skip (no review on WIP). Infra failures (action outage, no token) fail-open with no quotas spent. Model parse failures (invalid JSON) are retried once then fall back to conservative "no findings" pass.
- **Operational dependency** — after merge, a **manual step** is required: in the GitHub repository's branch protection ruleset, add `🤖 AI Review OK` to the required status checks list (same place where `✅ CI OK` is required). Without this, the check runs but does not block merge. Admin can later un-require it to unblock merges on production emergencies (no code changes needed — the rule is in the ruleset, not the code).
- **False-positive learning** (from ADR-0002 feedback) is baked into the prompt: "Honor the Hard exclusions / Signal-quality criteria / Precedents in `.claude/review-config.md` — never re-raise a listed false positive." Prior runs' findings ledger allows convergence tracking (PR 1) and re-filing avoided (same category+summary on the same file is suppressed if already resolved-changed or suppressed).
- **Audit trail** — every finding is logged to `.claude/.review-ledger.jsonl` (branch-scoped, per-session truncated to 5000 lines). Metrics (model, costs, time, parse retries, confidence distribution) logged to `.claude/.review-metrics.jsonl`. Both read-only to the local developer (to prevent manual cheating of the ledger).
- **Model tier:** Claude Sonnet 5 for the CI gate (lower latency/cost than Opus for a deterministic diff review that doesn't need agency chains). Opus still used for the on-demand `/review` deep dive (agent routing, inline comments).
- **Breaking change for pre-commit hook users:** the Stop gate is now stricter (unless REVIEW_MODE=off or bypassed with --no-verify). Developers must either pass local review, suppress known FP, or invoke --no-verify (which is now auditable in the commit history via the pre-push gate).

## Rationale

Connecting review enforcement at the merge boundary (CI) closes the gap between functional gatekeeping (linting, tests, build) and semantic gatekeeping (AI findings). The **fail-open on infra** design means an action outage will not freeze the main branch — the pre-push gate and CodeRabbit still provide redundant review. The **deterministic verdict in JS** (never "APPROVED" prose) means there is no ambiguity — HIGH/CRITICAL findings at confidence ≥ 0.8 block merge, period. The concurrency-cancel + draft skip ensure quota spend is bounded and sensible. The ledger tracks convergence (same finding reappears across reruns, signaling a real issue) and false positives (marking them as suppressed prevents re-filing).
