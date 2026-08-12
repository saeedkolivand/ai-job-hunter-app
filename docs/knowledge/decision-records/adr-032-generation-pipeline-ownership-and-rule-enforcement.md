# ADR-032: Generation-pipeline ownership — mechanical core-rule enforcement

Last updated: 2026-08-12

**Status:** Accepted

## Context

Resume generation prior to Phase 3 was one-shot: a single LLM call produced the resume without fact-grounding or validation. The system's core rule — the LLM decides HOW to present verified candidate evidence, never WHAT the candidate has done — was aspirational and only enforced via prompt text. No structural barriers existed to prevent a model from inventing facts, dropping roles, altering dates, or fabricating technologies.

LLM-driven multi-stage decomposition without re-grounding to source documents risks progressive hallucination. Every stage that accepts a prior stage's output as fact (rather than suspect intermediate text) compounds the risk (documented in the plan's research-deltas § "Decomposition").

## Decision

The quality-depth pipeline (Phases 2–4) enforces the core rule structurally and mechanically in Rust, not via prompt discipline:

1. **Orchestration is Rust-owned.** The pipeline (`apps/desktop/src-tauri/src/pipeline/resume/`) is a deterministic state machine (`Pipeline::run_hooked`), not an LLM-driven multi-agent orchestrator. Each stage produces a structured artifact; the next stage consumes it through a hard schema.

2. **Three things are withdrawn from the model structurally:**
   - **Evidence grounding** — the LLM ranks candidate evidence; Rust drops non-verbatim quotes (never repaired) and overwrites the `status` field from the source keywords kernel, guaranteeing the pipeline's coverage claim cannot disagree with the Jobs-page match score.
   - **Company roster lock** — the LLM seeded with a roster extracted from the source résumé; it may plan tailored emphasis per company but never drop, invent, or re-date a role. The strategy artifact is rebuilt on the seeded roster.
   - **Validation** — zero provider calls. Every Critical issue is deterministic: a factual comparison against the source document (dropped roles, unsourced metrics, altered links, language misalignment). A model may never emit a Critical.

3. **Prior-stage output is untrusted.** Every stage fences the prior stage's artifact in a hostile-input boundary (ADR-010); the new stage treats it as suspect text from a scraped posting, never as verified truth.

4. **Deterministic depth tiers:**
   - **Fast** — today's TS path + validators over its output (auto-report, no auto-repair).
   - **Quality** — analyze/evidence/strategy/draft/validate/repair pipeline (4 LLM calls + deterministic checks + ≤2 repair rounds).
   - **Max** — quality + section-wise generation (7 calls + ≤9 sequential section stages).

5. **Repair is bounded and transparent.** Criticals group by section; ≤2 rounds; failing sections only. Re-validation happens after each round. Strictly-more-criticals ⇒ revert + stop. Terminal fabrications (surviving after repair) go to a per-bullet review panel; the user decides to keep or remove. Nothing is silently dropped.

6. **Prompt codegen prevents drift.** `pnpm gen:prompts` (scripts/gen-prompts-rust.ts) calls the real TS functions at build time and freezes pure `(lang)→string` outputs into `prompt_blocks.rs` (`FACTUAL_GROUNDING_RULES`, `HUMANIZE_LEXICAL`, `ATS_PRECEDENCE`, anti-AI-tell lexicon). The same lexicon instructs the model AND validates its output; there is no second copy.

7. **Observability is content-free.** Every stage logs lifecycle (name, duration, cached-vs-live, attempt, ok), parsed-output outcome (native/fallback/re-ask/hard-fail), and validator summary. No résumé text, no evidence spans, no prompt fragments ever log. The run row and persisted events table carry the full audit trail.

## Consequences

- **Language/depth negotiation:** the app suggests depth (fast default, soft small-model warning); Settings + per-run overrides let the user choose. UI self-describes each depth (call count, cost, time, what runs).
- **Type-system enforcement:** `SectionKey` rejects "header" at the type level; `ExportIssue::Severity` forbids a model-emitted Critical; the schema layer prevents invalid states before Rust code runs.
- **Repair loop boundary:** the loop takes the provider call and validation as closures; production passes real callables; tests drive it against mock models. No private loop logic; coverage is end-to-end.
- **Artifact clamping:** strategy and evidence artifacts are JSON; both are clamped before persistence; truncated artifacts fail hard (never splice). Sizes are tripwired so growth is argued.
- **Retention and cleanup:** runs persist newest 3 per `(job_url, kind)` pair; validation/repair artifacts never cache (verdicts age); the run row is immutable history; the aggregate document is live state (join on source-text-hash for staleness detection).
- **Budget enforcement:** outer deadline checked between stage calls (not just at boundaries) and enforced in the repair loop. Per-call bounds hold the sequence even if one call times out. Retry-after-timeout is eliminated for completion and stream entry points (sequence budget = per-call bound); embed sites deliberately preserve it via `send_embed_with_retry` (per-attempt bound + sequence-budget multiplier for cold-start recovery).
- **Context isolation:** each stage receives only what it needs (prior artifact fenced, job ad fenced, user input fenced). The stage cannot access globals, priors beyond the one it consumes, or anything except its schema inputs.

## Related

- `apps/desktop/src-tauri/src/pipeline/resume/` — stage orchestration + prompts + validation
- `apps/desktop/src-tauri/src/validate/content/` — deterministic content validators (factual, alignment, consistency, voice, ATS, letter)
- `apps/desktop/src-tauri/src/documents/evidence/mod.rs` — evidence extraction and ranking
- `packages/prompts/scripts/gen-prompts-rust.ts` — prompt codegen (TS → Rust frozen blocks)
- ADR-010 (untrusted-input fencing) — fence tag patterns, hostile-input boundaries
- ADR-017 (persisted caches) — KvCache strategy for stages
- ADR-027 (diagnostics privacy) — content-free observability
