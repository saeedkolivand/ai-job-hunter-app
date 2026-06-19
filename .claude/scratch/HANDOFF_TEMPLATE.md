<!--
Per-task handoff file — the shared working context for ONE task.

The orchestrator (main session) pre-harvests context here (graphify/codegraph paths +
signatures) so no stage cold-re-explores. Each stage READS this before exploring and
APPENDS its output. Copy this template to `.claude/scratch/<task-slug>.md` (gitignored;
this template is the only committed file in scratch/). See `review-workflow` + `author-contract`.
-->

# Handoff · <task-slug>

## Context (orchestrator pre-harvest)

- **Goal:** <one line>
- **Owner pair:** author = `<x-author>` · critic(s) = `<x-reviewer>` (+ secondary on risk)
- **Touched area / paths:** <graphify/codegraph-resolved files + key signatures>
- **Constraints / prior art:** <existing helpers/hooks/registries to reuse; relevant lessons>

## Plan

- <minimal-change plan; Rust-first for business logic; new IPC → the 5-step flow>

## Changes (author appends)

- files: <…> · decisions: <…> · open questions for the critic: <…>

## Findings (critic appends — never the author)

- `SEVERITY · file:line · finding · one-line fix` (HIGH/CRITICAL block; author resolves → re-audit)

## Lessons to propose (→ project-steward)

- `LESSON · <category> · Context: … · Decision: … · Outcome: …` (tag memory type: episodic|semantic|procedural)
