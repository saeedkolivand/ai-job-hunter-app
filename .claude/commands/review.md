---
description: Strict pre-PR review of the diff — risk-tiered ensemble (1-4 pr-reviewer passes by size/risk), security gate first on risky diffs, per-finding verifier fan-out, deterministic verdict from parsed schema-1 findings
argument-hint: [base-ref or PR# — defaults to diff vs origin/main]
---

Run the **strict internal pre-PR review** — the gate that runs BEFORE a PR is opened so CodeRabbit finds less. Generation and judging stay separate; the verdict is computed from parsed findings, never from reviewer prose.

## 1. Context

- Read `.claude/review-config.md` (path rules + learnings — do not re-raise the listed false positives).
- Query prior lessons for the touched domains: `node .claude/hooks/lessons.mjs query --domain <d> --limit 4` for each domain whose `lessons_domains` globs (`.claude/review-routes.json`) match the diff (≤3 domains).

## 2. Scope + risk tier (mechanical — compute, don't vibe)

Scope = the diff of `$ARGUMENTS` (a base ref like `main`, or a PR#) — default `git diff origin/main...HEAD`. Stay inside the change's blast radius; pre-existing issues are out of scope unless the change endangers them.

Compute the tier:

- `lines` = added+removed from `git diff --numstat <scope>`, excluding lockfiles/snapshots/generated/docs (the gate's skip-globs).
- `security` = any changed file matches the `tauri-security-reviewer` secondary globs in `.claude/review-routes.json`.

| Tier         | Condition                        | Ensemble                           | Security gate | Verifiers              |
| ------------ | -------------------------------- | ---------------------------------- | ------------- | ---------------------- |
| **trivial**  | ≤10 lines AND no security match  | 1× pr-reviewer (sonnet)            | no            | no                     |
| **standard** | ≤100 lines AND no security match | 2× pr-reviewer (opus + sonnet)     | no            | single-source findings |
| **full**     | security match OR >100 lines     | 3× pr-reviewer (1 opus + 2 sonnet) | yes, FIRST    | single-source findings |

## 3. Security gate (full tier only — always FIRST)

Spawn `tauri-security-reviewer` over the same scope (or invoke `/review-security`) — desktop/app/backend/AI/data/abuse/supply-chain lens. **HIGH/CRITICAL block** and must be resolved (route fixes to the owning domain author) before the ensemble runs; LOW/MEDIUM are carried into the final report as advisories.

## 4. Run the tools ONCE, then the ensemble

Run the repo's real tools yourself, ONCE, scoped to what the diff touches: `rtk pnpm typecheck`, `rtk pnpm lint:strict`, cargo suite if `src-tauri/**` changed, `rtk pnpm gen:ipc:check` if `packages/shared/**` changed, targeted tests. Paste the outputs (or "clean") into every ensemble prompt — the pr-reviewer agents are instructed to use provided tool outputs and not re-run them.

Spawn the ensemble **in parallel** (one Agent call per pass, per the tier). Research: ~93% of real defects are found by exactly one reviewer of several, and a 3-pass union beats one frontier pass — diversity is the point. Give each pass a different file-order note (e.g. "review files in reverse path order" / "start from the test files") so the passes don't anchor identically. Each pass runs the full pr-reviewer agent and ends with its schema-1 findings JSON.

## 5. Synthesis — dedup, verify, decide (the precision step)

1. **Parse** each pass's fenced ```json findings block (plus carried security advisories). A pass whose JSON is missing/invalid: ask it once to re-emit the block; if still invalid, treat its prose 🔴/🟠 items as findings with `confidence: 0.5`.
2. **Dedup** by `file` + `line ±3` + `category`. Findings raised by **≥2 passes are auto-confirmed** (consensus — keep the max severity and the **max** confidence: independent agreement is stronger evidence than either pass alone, never weaker).
3. **Verify single-source findings**: for each finding only one pass raised, spawn ONE `finding-verifier` subagent (all in parallel), passing the finding JSON + its diff hunk. **Drop anything scoring < 80.** For survivors, the verifier's score **replaces** the finding's confidence (`score / 100`) — fresh-context verification is the strongest signal we have, so it, not the originating pass's self-rating, feeds the verdict. Do NOT re-verify findings yourself in-session (same-session self-review is the weakest form).
4. **Verdict — mechanical**: BLOCK iff any surviving finding has severity CRITICAL/HIGH (🔴/🟠) with `confidence ≥ 0.8` and `introduced_by_diff !== false`. 🟡/⚪ and low-confidence survivors are advisory. **🔴 + 🟠 must be resolved before the PR goes up.**
5. Report: verdict line first, then surviving findings grouped by severity (`severity | file:line | defect | substantiation | fix`), then advisories. Append any `UNVERIFIED (cross-OS: …)` coverage caveats from the passes verbatim.

## 6. Learnings

When a finding turns out to be a false positive, append it to `.claude/review-config.md` learnings so it isn't re-raised — and tell the user which verifier/pass produced it (calibration data for `/review-stats`).
