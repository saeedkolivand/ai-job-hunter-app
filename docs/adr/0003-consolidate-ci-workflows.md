---
status: accepted
---

# Consolidate the CI workflows (16 → 9) and DRY the job bootstrap

## Context

The CI-tooling program deliberately landed one tool per workflow file (advisory-first,
role-tagged, with a generated catalog). That gave clean single-responsibility but, at 16
files, scattered truly-related concerns (four separate security workflows all feeding the
Security tab), a 16-badge wall, and the same `harden-runner` + `checkout` + setup
boilerplate repeated in every job. This is a public repo (free unlimited Actions minutes),
so the only payoff sought is **readability / maintenance**, not cost.

## Decision

Consolidate by _concern_, keeping the required gate isolated:

- **Security 4 → 1** — `security.yml` now holds CodeQL + Semgrep + OpenSSF Scorecard +
  npm/cargo audit as separate jobs, each **event-gated** (`if:`) and **least-privilege**
  (`permissions:` per job), under one weekly cron. `codeql.yml`/`semgrep.yml`/`scorecard.yml`
  deleted.
- **UI 3 → 1** — `ui-checks.yml` holds Playwright e2e + Lighthouse + Lost Pixel.
  `e2e.yml`/`lighthouse.yml`/`visual.yml` deleted.
- **Quality 3 → 1** — `quality.yml` holds the JS/docs checks + Rust cargo-hack/cargo-mutants
  - the export-render **benchmark**. `rust-quality.yml`/`benchmark.yml` deleted. The benchmark
    job is the only one with elevated scope (`contents: write` + deploy key + Pages PAT); it is
    isolated by per-job `permissions:`/`if:` and a `changes` path-gate, so the rest of the file
    stays `contents: read`, PR-only.
- **DRY (descoped)** — a "bootstrap" composite that _also_ did checkout was attempted but is
  infeasible: a **local composite action cannot run before `actions/checkout`** (GitHub resolves
  `./` actions from the checked-out working tree, so a job whose first step is the local action
  fails with "Did you forget to run actions/checkout?"). So `harden-runner` + `checkout` stay
  explicit per job; the existing `setup-node-pnpm` / `setup-rust` composites remain the setup DRY.
- **Organization** — the catalog generator now renders **role-grouped sections** (Required /
  Security / Advisory / Deploy) in `.github/workflows/README.md` + the root README badges.

Result: 16 → 9 workflows.

## Consequences

- **Reverses the one-tool-per-file decision** for the security/UI/quality groups — the reason
  a future reader sees fewer, multi-job files. Single-responsibility is preserved at the _job_
  level; the merged files are grouped strictly by concern.
- **Gate stays isolated.** `ci-pipeline.yml` (the only required check, `✅ CI OK`) is **not**
  touched — no advisory/security job folds into it, protecting the required-status contract.
  `release.yml` is also untouched (cross-platform; Harden-Runner is Linux-only).
- **Heterogeneity moved into `if:`/`permissions:`** — `security.yml`'s union `on:` (PR + push +
  weekly + `branch_protection_rule`) means each job must gate its own event/permissions; this is
  the densest file. Acceptable for the "all security in one place" win; zizmor still audits it.
- **Benchmark in an advisory file** carries `contents: write` + secrets in one job. Per-job
  token scoping keeps the other jobs read-only, but the file now _contains_ an elevated job —
  an accepted trade for the consolidation.
- Revertible from git history if a merged file proves unwieldy.
