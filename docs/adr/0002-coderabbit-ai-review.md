---
status: accepted
---

# CodeRabbit is the always-on AI PR reviewer; the duplicated reviewdog/Danger/labeler lanes are retired

## Context

The repo runs an 18-workflow CI program whose stated default is "Actions-native, zero
external SaaS, advisory-first" — Codecov and SonarCloud were dropped for exactly that
reason; the only deliberate external opt-in was Harden-Runner (StepSecurity telemetry).
AI/heuristic PR review was spread across three advisory lanes: `pr-review.yml` (reviewdog
posting ESLint + Clippy inline + `dangerfile.ts` deterministic rules reusing
`.claude/review-routes.json`), the reviewdog `actionlint` lane in `workflow-lint.yml`,
the path-based `labeler.yml`, and the on-demand `claude-review.yml` (`@claude review`).

We wanted richer review (semantic findings, PR summaries/walkthroughs, line-by-line
fixes) without three bots commenting on every PR. CodeRabbit is **free and unlimited on
public repositories** (this repo qualifies) and runs ESLint, Clippy, Semgrep, secret-scan,
and actionlint in-PR — directly overlapping those advisory lanes.

## Decision

Adopt **CodeRabbit** (external SaaS, free OSS plan) as the **always-on** AI reviewer,
configured by [`.coderabbit.yaml`](../../.coderabbit.yaml) whose `path_instructions`
mirror the Primary-owner map in `.claude/review-routes.json` and the `CLAUDE.md` /
`.github/copilot-instructions.md` conventions, with `labeling_instructions` mirroring the
former `.github/labeler.yml`. It is advisory only (`request_changes_workflow: false`) — it
never approves or blocks; **"✅ CI OK" remains the sole required check**.

Consequently we **retire the now-duplicated lanes**: delete `pr-review.yml`,
`dangerfile.ts`, `labeler.yml` + `.github/labeler.yml`, and the reviewdog `actionlint`
job in `workflow-lint.yml` (keeping its `zizmor` + catalog-drift jobs). The `danger`
devDependency is removed. **Keep `claude-review.yml`** as the on-demand, owner-gated,
agent-routed deep dive (inert until `@claude review`, so zero idle cost).

Everything CodeRabbit cannot replace is **unchanged**: the gating `ci-pipeline.yml`
(lint/type/test/build/Rust quality/architecture/`cargo-deny`/`cargo-machete`), the
SARIF→Security-tab jobs (`codeql.yml`, `semgrep.yml`, `scorecard.yml`, `security.yml`),
`rust-quality.yml`, the functional/perf/visual jobs (`e2e`/`benchmark`/`lighthouse`/
`visual`), `release.yml`/`pages.yml`, `format-guard.yml`, and the deterministic
`quality.yml` (knip/i18n/typos/links/a11y).

## Consequences

- **Tradeoff (named):** this breaks the "zero external SaaS" default — CodeRabbit reads
  the repo in its cloud. Accepted as a deliberate opt-in (like Harden-Runner); low risk
  for a public repo, but it is a third-party with repo read access and PR-write, so it is
  in scope for the `tauri-security-reviewer` supply-chain checklist.
- **Setup:** requires installing the CodeRabbit GitHub App on the repo (one-time, like the
  Claude GitHub App). Until installed, `.coderabbit.yaml` is inert.
- **Fork PRs** are reviewed too (GitHub App, not a `GITHUB_TOKEN` job) — an improvement
  over reviewdog/Danger, which could not comment on read-only fork tokens.
- Net workflow count drops from 18 → 16; one AI review surface instead of three bots.
- If CodeRabbit is ever dropped, restore `pr-review.yml` + `dangerfile.ts` + `labeler.yml`
  from git history and re-add the `danger` devDependency.
