# ADR-033: No model-written agent memory — core-rule violation and injection carrier

Last updated: 2026-08-12

**Status:** Accepted

## Context

The adapted spec for the agentic assistant (Phases 4–7) included a tentative `improve_resume` agent that would review a generation against its quality report + evidence and propose targeted fixes. An early design sketch proposed that the agent could "learn" from each run by persisting synthesis notes, patterns observed, or heuristics to a per-user agent memory.

This pattern carries three overlapping risks:

1. **Core-rule violation** — agent memory written by an LLM describing candidate data becomes a second-hand, unsourced version of facts. Future runs using that memory as input reground themselves to the agent's summary, not the source résumé or job ad. The core rule (LLM decides HOW, never WHAT) is already violated at the point the agent writes the memory.
2. **Cross-run injection carrier** — LLM-written memory persists across separate jobs and runs. A hallucination or mistake in run-N's memory becomes a premise for run-N+1. Error compounding is invisible because the memory is opaque to the user.
3. **Multi-source confusion** — when the agent consumes its own prior memory alongside source documents, the model cannot easily distinguish which facts come from the source and which are the agent's own prior-synthesis. Grounding becomes ambiguous.

The practical alternative — persisting only structured, user-verifiable intermediates (runs, verdicts, quality reports) — requires no model writes and preserves auditability.

## Decision

Agent memory written by an LLM is not implemented. The agent system persists only the following, all immutable and user-verifiable:

1. **Quality reports** (per-run, immutable) — deterministic content validators run after every generation (fast, quality, max depths alike). The report lives in the `quality_report` column on `ai_generations` and includes issue codes, severities, evidence spans, and per-bullet verdicts (keep/remove/undecided).
2. **Run history** (per job) — newest 3 runs retained per `(job_url, kind)` pair; each carries status, metrics, stage timeline, artifacts (analysis, strategy, draft stage output). Immutable; new runs do not rewrite old ones.
3. **Run identities + tools** — the `improve_resume` flow (Phase 7+) is a stateless agent that works from six tools — five Read-only (`validate_resume`, `search_candidate_evidence`, `get_trim_suggestions`, `get_quality_report`, `run_quality_pipeline`) plus the gated write `save_resume` in a single pass and proposes fixes. **Tools take zero arguments** — the backend supplies context from the active session (all arguments are server-side resolved, making prompt injection across tool calls structurally impossible). No memory written after the flow concludes.

## Consequences

- **Agent complexity** — the `improve_resume` flow cannot accumulate learnings across runs. Each invocation is independent. Heuristics that would live in memory (e.g., "this type of issue is unrepairable") must be expressed in the tool outputs or the flow's system prompt instead.
- **User agency preserved** — all persistent state (reports, verdicts, run history) is human-readable and user-settable (a user can decide to keep or remove a finding, edit a run's status annotation). The agent proposes; the user verifies.
- **Simplification** — the agent subsystem (agent/controller, tools, flows, gates, budgets) has one responsibility: propose and revise documents within a single session. It does not learn or adapt across sessions.
- **Future extensibility** — if per-user learning later becomes desired (e.g., "resume patterns this user likes"), it must be:
  - Explicitly user-controlled (opt-in, with an audit trail of what was learned).
  - Sourced from user edits and verdicts, not from model outputs (the model learns from what the user chose to keep, not from the agent's own synthesis).
  - Separate from the core pipeline (a Phase-8+ optional module, not baked into the improve_resume flow).

## Related

- ADR-032 (generation-pipeline ownership) — core rule: LLM decides HOW, never WHAT.
- `apps/desktop/src-tauri/src/agent/` — agent controller, tools, flows, gates.
- `apps/desktop/src-tauri/src/validate/content/mod.rs` — deterministic quality validators.
- `apps/desktop/src-tauri/src/pipeline/runs/mod.rs` — immutable run history and retention.
- `packages/shared/src/ipc/contracts/resumePipeline.ts` — run history query contract (`listForJob`).
