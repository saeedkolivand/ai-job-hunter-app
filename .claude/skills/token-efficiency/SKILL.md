---
name: token-efficiency
description: Shared context-discipline contract every agent imports — context-source priority, read budget, confidence-stop, the severity rubric, and terse output. Load at the start of any review or implementation task.
---

# Token-efficiency contract (all agents)

## Context-source priority (in order)

1. **codegraph (structural)** — symbols, calls, imports, impact/blast-radius: prefer the `mcp__codegraph` MCP tool (`codegraph_explore`) when the agent's allowlist includes it, else the CLI (`codegraph callers/callees/impact/query`). Sub-millisecond, zero-token.
2. **graphify (semantic)** — meaning, rationale, cross-document synthesis: prefer the MCP tools when connected (`query_graph`, `shortest_path`, `get_community` / `get_node` / `god_nodes`); fall back to the CLI (`graphify query "<question>"`, `graphify explain "<concept>"`, `graphify path "<A>" "<B>"`) when MCP is unavailable. Either returns a scoped subgraph, far smaller than grep / GRAPH_REPORT.md.
3. **source code** — authoritative for any region edited this turn (graphify can lag un-indexed edits until `graphify update .`).
4. **docs/knowledge/** — shape, contracts, standards.
5. **lessons** — historical experience, queried on-demand (never bulk-loaded).

## Read discipline

- Read the **minimum** files needed. **No repo-wide scans**; prefer `codegraph` for structural lookups ("where is X" / "what calls X") and `graphify` for semantic ("what is X connected to"), over `rg`/`grep`.
- **Stop at ~90% confidence.** Never read another file solely to go 90→100%.
- Knowledge files are capped (~150 lines) — read the relevant section, not the whole file.

## Severity rubric (anchors blocking — reproducible, not free judgment)

**STRICT MODE (enforced).** The bar is deliberately high. **Verify, don't assume** — confirm every claim against the actual code/files before clearing it (a key exists, a path is covered, a guard is present); never wave something through because it "looks fine."

- **CRITICAL** — exploitable security on a secret/credential/IPC/updater/network-egress path; data loss/corruption; breaks a release or CI gate.
- **HIGH** — architecture-rule violation (`std::env::var` outside `platform/`, `reqwest::Client` outside `net/`, untyped `Result<_,String>` outside `error/`); **changed non-trivial logic shipped WITHOUT a test**, or a test whose assertion is **weak/tautological / asserts the mock / doesn't exercise the change**; an **untested error / edge / security path** on changed code; provider-specific coupling in business logic; a PII / temp-file-cleanup / data-retention regression; **user-facing text whose i18n key is missing from `en` or `de`** (or a `t()` referencing a non-existent key).
- **MEDIUM** — unguarded perf regression on a hot path, a non-blocking correctness smell, a missing NON-critical edge-case test.
- **LOW** — style, naming, comments, formatting, doc nits.
- **Only HIGH/CRITICAL block.** **STRICT tie-break: round UP** for test-coverage, error/edge-path, i18n, security, and data findings; round down only for pure style / naming / docs.

## Output format

Terse findings only: `SEVERITY · file:line · finding · one-line fix`. No prose essays.

## Spawning implementation agents efficiently

Domain **authors** (write-capable) implement; their independent **critics** audit. Cold repo re-exploration is the dominant token cost, so the primary lever is **cold-start minimization via the per-task handoff file** (`.claude/scratch/<task>.md`): the orchestrator pre-harvests paths + signatures once, and every stage reads it instead of re-exploring. Two further levers now exist in this harness: `SendMessage` to continue a warm agent (its context intact), and native **Agent Teams** (shared task list + mailbox) — but teams cost more tokens, so use them **only when parallelism genuinely pays** (file-disjoint, multi-domain work).

**Pattern:**

1. **Pre-harvest** — before spawning, query graphify yourself (MCP `query_graph` / `shortest_path` when connected, else the `graphify query` / `explain` / `path` CLI); collect exact file paths and the relevant function/type signatures. Hand them in the prompt.
2. **Graphify-first directive in the prompt** — explicitly tell the spawned agent to query graphify (MCP `query_graph` if its `tools:` allowlist includes `mcp__graphify`, else `graphify query "<question>"`) before reading any source file. Domain reviewer agents get this from their system prompt; implementation agents do not.
3. **Fewest + largest vertical-slice spawns** — one agent per full feature slice; avoid spawning separate agents for Rust, TS, and tests when they are the same slice.
4. **Right-size the model** — reserve large-context / high-reasoning models for ambiguous design decisions; use smaller models for mechanical CRUD, test scaffolding, or renaming tasks.
5. **Batch domain reviews** — collect all reviewable diffs and send them to domain agents in a single pass, not one per file.
6. **Thin orchestration** — the orchestrator prepares context and sequences agents; agents execute. Orchestrators must not re-explore what agents will re-explore; agents must not re-explore what the orchestrator already harvested.

**Reference files:** `.claude/skills/graphify/SKILL.md` (query/explain/path commands) · `.claude/agents/` (domain reviewer system prompts as grounding examples).

## Lessons

Propose durable lessons as `LESSON · <category> · Context: … · Decision: … · Outcome: …` (≤5 lines). Only `project-steward` persists them.
