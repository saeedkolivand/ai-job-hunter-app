---
name: token-efficiency
description: Shared context-discipline contract every agent imports — context-source priority, read budget, confidence-stop, the severity rubric, and terse output. Load at the start of any review or implementation task.
---

# Token-efficiency contract (all agents)

## Context-source priority (in order)

1. **codegraph (structural)** — symbols/calls/impact: MCP `codegraph_explore` when your allowlist has it, else CLI `codegraph callers/callees/impact/query`.
2. **graphify (semantic)** — meaning/rationale/cross-doc: MCP `query_graph`/`shortest_path`/`get_*` when connected, else CLI `graphify query|explain|path`.
3. **source** — authoritative for any region edited this turn.
4. **docs/knowledge/** — contracts, standards.
5. **lessons** — on-demand only, never bulk-loaded.

## Read discipline

- Minimum files; no repo-wide scans — codegraph for "where/what calls X", graphify for "what connects to X", `rg` only when neither answers.
- **Stop at ~90% confidence.** Read the relevant section, not the whole file.

## Severity rubric (anchors blocking — STRICT MODE, verify-don't-assume)

- **CRITICAL** — exploitable security on a secret/credential/IPC/updater/network-egress path; data loss/corruption; breaks a release or CI gate.
- **HIGH** — architecture-rule violation (`std::env::var` outside `platform/`, `reqwest::Client` outside `net/`, untyped `Result<_,String>` outside `error/`); changed non-trivial logic WITHOUT a test, or a weak/tautological/mock-asserting test; untested error/edge/security path on changed code; provider-specific coupling in business logic; PII/temp-file-cleanup/retention regression; user-facing text whose i18n key is missing from `en` or `de`.
- **MEDIUM** — unguarded hot-path perf regression, non-blocking correctness smell, missing non-critical edge test.
- **LOW** — style/naming/comments/formatting/docs.
- Only HIGH/CRITICAL block. **Tie-break: round UP** for test-coverage, error/edge-path, i18n, security, and data findings; round down only for pure style/docs. Confirm every claim against the real code/files; never pass a hunk you did not actually read.

## Output format

Terse findings only: `SEVERITY · file:line · finding · one-line fix`. No prose essays.

## Strict enforcement (canonical — agent files point here, never restate)

The single source for every agent's "Strict enforcement" block: STRICT MODE + the rubric above + verify-don't-assume + round-UP tie-break + the terse format. Write-capable **authors** additionally follow `author-contract`; read-only **critics** follow `critic-contract`. Agent files add ONLY their domain-specific HIGH examples below a one-line pointer here.

## Lessons

Propose durable lessons as `LESSON · <category> · Context: … · Decision: … · Outcome: …` (≤5 lines). Only `project-steward` persists them.
