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

## Spawning implementation agents efficiently

Implementation agents use `general-purpose` (read-write) type and lack the domain reviewers' graphify-first grounding. Cold repo re-exploration is the dominant token cost (~70–122 k per agent). The only lever in this harness is cold-start minimization — `SendMessage` agent-reuse is not available.

**Pattern:**

1. **Pre-harvest** — before spawning, run `graphify query` / `graphify explain` / `graphify path` yourself; collect exact file paths and the relevant function/type signatures. Hand them in the prompt. A pre-harvested spawn costs ~44 tool-uses vs ~120 for cold exploration.
2. **Graphify-first directive in the prompt** — explicitly tell the spawned agent to run `graphify query "<question>"` before reading any source file. Domain reviewer agents get this from their system prompt; implementation agents do not.
3. **Fewest + largest vertical-slice spawns** — one agent per full feature slice; avoid spawning separate agents for Rust, TS, and tests when they are the same slice.
4. **Right-size the model** — reserve large-context / high-reasoning models for ambiguous design decisions; use smaller models for mechanical CRUD, test scaffolding, or renaming tasks.
5. **Batch domain reviews** — collect all reviewable diffs and send them to domain agents in a single pass, not one per file.
6. **Thin orchestration** — the orchestrator prepares context and sequences agents; agents execute. Orchestrators must not re-explore what agents will re-explore; agents must not re-explore what the orchestrator already harvested.

**Reference files:** `.claude/skills/graphify/SKILL.md` (query/explain/path commands) · `.claude/agents/` (domain reviewer system prompts as grounding examples).

**Future / not-yet-built — graphify MCP:** A planned enhancement is to expose `graphify query`, `graphify explain`, and `graphify path` as MCP tools so agents call structured graph retrieval instead of shelling out. `graphify --mcp` already starts a stdio MCP server (see the `--mcp` section of `.claude/skills/graphify/SKILL.md`) — the remaining work is wiring it into the Claude Code MCP config so it is always-on for this project. Until then, agents must shell out via `rtk graphify query …`.

## Lessons

Propose durable lessons as `LESSON · <category> · Context: … · Decision: … · Outcome: …` (≤5 lines). Only `project-steward` persists them.
