---
name: token-efficiency
description: Shared context-discipline contract every agent imports — context-source priority, read budget, confidence-stop, the severity rubric, and terse output. Load at the start of any review or implementation task.
---

# Token-efficiency contract (all agents)

## Context-source priority (in order)

1. **graphify** — `graphify query "<question>"`, `graphify explain "<concept>"`, `graphify path "<A>" "<B>"`. Returns a scoped subgraph, far smaller than grep / GRAPH_REPORT.md.
2. **source code** — authoritative for any region edited this turn (graphify can lag un-indexed edits until `graphify update .`).
3. **docs/knowledge/** — shape, contracts, standards.
4. **lessons** — historical experience, queried on-demand (never bulk-loaded).

## Read discipline

- Read the **minimum** files needed. **No repo-wide scans**; prefer `graphify` over `rg`/`grep` for "where is X".
- **Stop at ~90% confidence.** Never read another file solely to go 90→100%.
- Knowledge files are capped (~150 lines) — read the relevant section, not the whole file.

## Severity rubric (anchors blocking — reproducible, not free judgment)

- **CRITICAL** — exploitable security on a secret/credential/IPC/updater/network-egress path; data loss/corruption; breaks a release or CI gate.
- **HIGH** — architecture-rule violation (`std::env::var` outside `platform/`, `reqwest::Client` outside `net/`, untyped `Result<_,String>` outside `error/`); an untested error/security path on changed code; provider-specific coupling in business logic; a PII / temp-file-cleanup / data-retention regression.
- **MEDIUM** — missing edge-case test, weak assertion, unguarded perf regression on a hot path, non-blocking correctness smell.
- **LOW** — style, naming, comments, formatting, doc nits.
- **Only HIGH/CRITICAL block.** Tie-break to the **lower** level (bias against false blocks) — **except** security/data findings, which round **up**.

## Output format

Terse findings only: `SEVERITY · file:line · finding · one-line fix`. No prose essays.

## Lessons

Propose durable lessons as `LESSON · <category> · Context: … · Decision: … · Outcome: …` (≤5 lines). Only `project-steward` persists them.
