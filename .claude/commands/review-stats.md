---
description: Review-system health report from .claude/.review-metrics.jsonl + .review-ledger.jsonl — findings/run, block/parse-fail/fallback rates, suppression, resolution, and best-effort CodeRabbit miss rate
argument-hint: [days back, default 30]
---

Report the review system's own performance — the eval loop that tells us whether the hardening program is working. All data is local JSONL; compute with `node -e` one-liners (both files may be absent on a fresh clone — degrade gracefully, report "no data yet").

## 1. Gate + pre-push metrics (`.claude/.review-metrics.jsonl`)

Per `kind` (`stop-gate`, `pre-push`, …) over the window (`$ARGUMENTS` days, default 30):

- runs, and outcome distribution (`clean` / `advisory` / `blocked` / `tier0-block` / `reemit-block` / `reemit-advisory` / `cache-skip` / `degraded` / `llm-unavailable` / `parse-failed` / `error`)
- findings per run by severity (mean), block rate
- **parse-failure rate** (`parse_failed`) — >5% means the schema-1 contract is drifting; consider `--output-format json` wrapper parsing
- **sg_fallback rate** — nonzero means ast-grep is missing/broken locally
- cache-skip + re-emit counts (`reemits`) — how much work the ledger/caches are saving
- suppressed counts (`suppressed`) — convergence + category suppression volume
- audited skips (`skipped: true`, pre-push `REVIEW_SKIP`) — a rising trend means the gate is becoming theater; investigate why
- mean `duration_ms` per outcome

## 2. Ledger analysis (`.claude/.review-ledger.jsonl`)

- open vs resolved-changed vs suppressed, by category
- **ignore streaks**: open entries with `reemits ≥ 2` — candidates approaching category auto-suppression (only style/perf/i18n can suppress; anything else with a streak is a real unresolved problem — surface it)
- resolution rate: resolved-changed ÷ (open + resolved-changed) per category — low resolution in a category = findings devs don't act on = calibrate that category's rules/prompts

## 3. CodeRabbit miss rate (best-effort, needs `gh` auth + network)

For merged PRs in the window (`gh pr list --state merged`), fetch review comments authored by `coderabbitai` (`gh api repos/{owner}/{repo}/pulls/<n>/comments`). A CodeRabbit finding on a file for which our metrics/ledger recorded NO finding in that PR's branch = a **miss** — the number this whole program exists to drive down. Report: misses, total CodeRabbit findings, miss rate, and the missed findings themselves (file + first line of the comment) so they can become checklist rules or ast-grep rules. Note the coverage caveat: CodeRabbit is on-demand, so only PRs where it was triggered contribute.

## 4. Verdict line

End with one line: `review-stats: <runs> runs · <block-rate>% blocked · <parse-fail>% parse-fail · <skips> audited skips · miss-rate <n/m or n/a>` plus, if any threshold is breached (parse-fail >5%, ignore-rate >30% in a category, rising REVIEW_SKIP trend), a short "calibration needed" note naming the fix.
