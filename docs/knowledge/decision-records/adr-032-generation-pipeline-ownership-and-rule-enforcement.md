# ADR-032: Generation-pipeline ownership — mechanical core-rule enforcement

Last updated: 2026-08-17

**Status:** Accepted

## Context

Resume generation prior to Phase 3 was one-shot: a single LLM call produced the resume without fact-grounding or validation. The system's core rule — the LLM decides HOW to present verified candidate evidence, never WHAT the candidate has done — was aspirational and only enforced via prompt text. No structural barriers existed to prevent a model from inventing facts, dropping roles, altering dates, or fabricating technologies.

LLM-driven multi-stage decomposition without re-grounding to source documents risks progressive hallucination. Every stage that accepts a prior stage's output as fact (rather than suspect intermediate text) compounds the risk (documented in the plan's research-deltas § "Decomposition").

## Decision

The staged résumé pipeline enforces the core rule structurally and mechanically in Rust, not via prompt discipline:

1. **Orchestration is Rust-owned.** The pipeline (`apps/desktop/src-tauri/src/pipeline/resume/`) is a deterministic state machine (`Pipeline::run_hooked`), not an LLM-driven multi-agent orchestrator. Each stage produces a structured artifact; the next stage consumes it through a hard schema.

2. **Three things are withdrawn from the model structurally:**
   - **Evidence grounding** — the LLM ranks candidate evidence; Rust drops non-verbatim quotes (never repaired) and overwrites the `status` field from the source keywords kernel, guaranteeing the pipeline's coverage claim cannot disagree with the Jobs-page match score.
   - **Company roster lock** — the LLM seeded with a roster extracted from the source résumé; it may plan tailored emphasis per company but never drop, invent, or re-date a role. The strategy artifact is rebuilt on the seeded roster.
   - **Validation** — zero provider calls. Every Critical issue is deterministic: a factual comparison against the source document (dropped roles, unsourced metrics, altered links, language misalignment). A model may never emit a Critical.

3. **Prior-stage output is untrusted.** Every stage fences the prior stage's artifact in a hostile-input boundary (ADR-010); the new stage treats it as suspect text from a scraped posting, never as verified truth.

4. **The single staged pipeline** (`analyze_job` → `match_evidence` → `strategy` → `draft` → `cover_letter` → `validate` → `repair` → `humanize`):
   - **Quality pipeline** — the ONLY production pipeline for the apply flow. Built across: #990 (deterministic projects normalization), #991 (cover-letter + humanize stages), #992 (apply-flow cutover to staged run). Base call count: 4 (analyze, evidence, strategy, draft). Optional calls: cover_letter (1 call, gated on `ResumePipelineRunSchema.includeCoverLetter`, which owns the `false` default, in `packages/shared/src/schemas/index.ts`), repair (variable; only if Criticals exist; ≤2 rounds × `stages::repair::MAX_SECTIONS_PER_ROUND`, which owns that bound), humanize (0–2 calls; deterministic-first — the zero-flag early return in `stages::humanize::Humanize::run` owns the zero-cost path, and that same function owns the ≤1-call-per-document cap). Deterministic validation always runs; repair and humanize gate on report findings, never on assumptions. All stages are listed in `ipc_contracts::events::PIPELINE_STAGES` (generated from `packages/shared/src/events/pipeline.ts`).
   - **Fast path** — a separate one-shot generation entry point (renderer-owned TS implementation via `generateResume`) for the Resume Builder and AI Generate features. Not a settings choice on the apply flow; a different job (generating without a job posting). Backed by `lib/generate/generation` in the renderer.
   - **Historical**: max-depth section-wise generation with LLM judge was deleted in a follow-on deletion PR. The judge emitted Warning-only opinions nobody acted on, consuming 12+ calls per run for pure token spend. Historic runs still carry `depth: 'max'` in persisted rows, parsed by the frozen `GENERATION_DEPTHS` vocabulary read-side constant — the vocabulary survives the feature deletion so existing run history does not silent-relabel.

5. **Repair is bounded and transparent.** Criticals group by section; ≤2 rounds; failing sections only. Re-validation happens after each round. Strictly-more-criticals ⇒ revert + stop. Terminal fabrications (surviving after repair) go to a per-bullet review panel; the user decides to keep or remove. Nothing is silently dropped.

6. **Prompt codegen prevents drift.** `pnpm gen:prompts` (scripts/gen-prompts-rust.ts) calls the real TS functions at build time and freezes pure `(lang)→string` outputs into `prompt_blocks.rs` (`FACTUAL_GROUNDING_RULES`, `HUMANIZE_LEXICAL`, `ATS_PRECEDENCE`, anti-AI-tell lexicon). The same lexicon instructs the model AND validates its output; there is no second copy.

7. **Observability is content-free.** Every stage logs lifecycle (name, duration, cached-vs-live, attempt, ok), parsed-output outcome (native/fallback/re-ask/hard-fail), and validator summary. No résumé text, no evidence spans, no prompt fragments ever log. The run row and persisted events table carry the full audit trail.

## Amendment (2026-08-17): Language-mismatch enforcement

The decision that language misalignment is a deterministic Critical (outlined in Decision §2) was documented but not delivered for the cross-language case (English source, non-English target). Three causes compounded: the control asked the wrong question, the identity check routed through a stemmer predicate (wrong for most languages), and the prompt never instructed translation. Amended enforcement (now ships):

- **Language identity** (`documents::keywords::detected_language`) is a separate function from `languages_align` (stemming compatibility). It returns `Option<&'static str>` ISO-639-1 tag when `whatlang` reads the text at ≥0.9 confidence; `None` otherwise (covers 19 curated languages; every other language is a no-op, not a mis-detection). **Two detectors decide this question:** the renderer uses **franc**, Rust uses **whatlang**; when they disagree the guard goes quiet (as this module's documented posture demands).
- **Corroboration** (`target_is_corroborated`) replaces the broken `source_is_a_reliable_control`. The target language is credible when either the job ad OR the source résumé confidently reads as it — a document-agnostic question, so a translation run is no longer disqualified from being graded on its own translation output.
- **Whole-document language failure** is automatically retried ONCE inside the Draft stage. The retry is a loop-local counter (never persists), bounded to one call. A second failure falls through to Validate, raises the Critical, and parks the run at `needsReview`. Per-section language Criticals still route to Repair (unchanged).
- **Critical contract:** `validate::content` is never called from `export/` (ADR-034). A Critical does **not** block export — it parks the run at `needsReview` for user review. This is unchanged; documented here so the fix is not read as an export gate.

## Consequences

- **Quality pipeline only for apply:** the staged quality pipeline is the sole production route in the apply flow. No depth selection offered to users; no depth negotiation. The fast path is available as a separate entry point in the Resume Builder and AI Generate features, not as a cheaper setting on the apply flow.
- **Type-system enforcement:** `SectionKey` rejects "header" at the type level; `ExportIssue::Severity` forbids a model-emitted Critical; the schema layer prevents invalid states before Rust code runs. Historic `GenerationDepth` values ('fast', 'quality', 'max') persist in the read-side vocabulary so old run rows continue to parse and render.
- **Repair loop boundary:** the loop takes the provider call and validation as closures; production passes real callables; tests drive it against mock models. No private loop logic; coverage is end-to-end. Section-scoped rewrites via `regenerateSection` now always rewrite the whole section (no longer per-entry on max depth).
- **Artifact clamping:** strategy and evidence artifacts are JSON; both are clamped before persistence; truncated artifacts fail hard (never splice). Sizes are tripwired so growth is argued.
- **Retention and cleanup:** runs persist newest 3 per `(job_url, kind)` pair; validation/repair artifacts never cache (verdicts age); the run row is immutable history; the aggregate document is live state (join on source-text-hash for staleness detection). Historic max-depth runs still parse and render but cannot be re-generated or edited section-by-section.
- **Budget enforcement:** outer deadline checked between stage calls (not just at boundaries) and enforced in the repair loop. Per-call bounds hold the sequence even if one call times out. Retry-after-timeout is eliminated for completion and stream entry points (sequence budget = per-call bound); embed sites deliberately preserve it via `send_embed_with_retry` (per-attempt bound + sequence-budget multiplier for cold-start recovery).
- **Context isolation:** each stage receives only what it needs (prior artifact fenced, job ad fenced, user input fenced). The stage cannot access globals, priors beyond the one it consumes, or anything except its schema inputs.

## Related

- `apps/desktop/src-tauri/src/pipeline/resume/` — stage orchestration + prompts + validation
- `apps/desktop/src-tauri/src/validate/content/` — deterministic content validators (factual, alignment, consistency, language, voice, ATS, letter); `language.rs` submodule added for R8 line-cap compliance
- `apps/desktop/src-tauri/src/documents/evidence/mod.rs` — evidence extraction and ranking
- `packages/prompts/scripts/gen-prompts-rust.ts` — prompt codegen (TS → Rust frozen blocks)
- ADR-010 (untrusted-input fencing) — fence tag patterns, hostile-input boundaries
- ADR-017 (persisted caches) — KvCache strategy for stages
- ADR-027 (diagnostics privacy) — content-free observability
