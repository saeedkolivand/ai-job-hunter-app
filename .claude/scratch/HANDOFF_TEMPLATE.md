<!--
Per-task handoff file — the shared working context for ONE task.

The orchestrator (main session) pre-harvests context here (graphify/codegraph paths +
signatures) so no stage cold-re-explores. Copy this template to `.claude/scratch/<task-slug>.md`
(gitignored; this template is the only committed file in scratch/). See `review-workflow` +
`author-contract` + `token-efficiency`.

Read contract: every stage reads ONLY `## Current state` (mandatory, keep ≤2K chars — rewrite
stale parts, don't grow it). Full detail goes to the append-only `## Log`, which ONLY
`project-steward` reads at close-out.
-->

# Handoff · <task-slug>

## Current state

<!-- MANDATORY. ≤2K chars. The only section stages read — each stage rewrites the stale parts
     (esp. Status) after appending its detail to the Log below. -->

- **Goal:** <one line>
- **Owner pair:** author = `<x-author>` · critic(s) = `<x-reviewer>` (+ secondary on risk)
- **Paths / signatures:** <graphify/codegraph-resolved files + key signatures>
- **Constraints / prior art:** <existing helpers/hooks/registries to reuse; relevant lessons>
- **Plan:** <minimal-change plan; Rust-first for business logic; new IPC → the 5-step flow>
- **Status:** <what's done · unresolved findings the next stage must act on (`SEVERITY · file:line · finding · fix`) · open questions>

## Log

<!-- Append-only. Read by project-steward ONLY (close-out: docs/lessons sync). Stages append
     their full output here — changes, decisions, findings, lessons — newest last. -->

- `<stage> · <agent>` — changes: <files + decisions> · findings: `SEVERITY · file:line · finding · one-line fix` · lessons-to-propose: `LESSON · <category> · Context: … · Decision: … · Outcome: …` (tag memory type: episodic|semantic|procedural)
