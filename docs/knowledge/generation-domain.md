# Generation domain: the staged résumé pipeline

The generation system is a **deterministic pipeline** that enforces a core rule: the model decides _how_ to present verified evidence, never _what_ the candidate has done. This page maps the domain — stages, validation, fabrication review, cost shape, and eval strategy.

## Architecture: one quality pipeline

**Location:** [`apps/desktop/src-tauri/src/pipeline/resume/mod.rs`](../../apps/desktop/src-tauri/src/pipeline/resume/mod.rs)

The **quality pipeline** is the sole production path for apply-flow generation. It is a deterministic state machine with stages pinned by [`QUALITY_STAGES`](../../apps/desktop/src-tauri/src/pipeline/resume/mod.rs): `analyze_job`, `match_evidence`, `strategy`, `draft`, `cover_letter`, `validate`, `repair`, `humanize`. See the [`pipeline/resume/mod.rs`](../../apps/desktop/src-tauri/src/pipeline/resume/mod.rs) module doc for the per-stage call table (indicating which are base costs, optional, or bounded), the [`PipelineStage`](../../packages/shared/src/events/pipeline.ts) event vocabulary, and [`PipelineRunDetail`](../../packages/shared/src/ipc/index.ts) for the persisted run shape.

**Fast path (separate):** Resume Builder and AI Generate features use a one-shot generation entry point (renderer-side TS in `lib/generate/generation`), not the quality pipeline. The choice is entry-point-driven, not a user setting.

## Grounding: evidence is source-bound

**Location:** [`apps/desktop/src-tauri/src/pipeline/resume/stages/evidence.rs`](../../apps/desktop/src-tauri/src/pipeline/resume/stages/evidence.rs)

The `match_evidence` stage extracts and ranks candidate evidence (experiences, projects, skills) from the source résumé. The `draft` stage uses this ranked list as its input. The core rule is enforced at the boundary: [`ground()`](../../apps/desktop/src-tauri/src/pipeline/resume/stages/evidence.rs) validates that every quote in the model's output appears verbatim in the source. Non-verbatim quotes are dropped, never repaired.

This guarantee is load-bearing: Autopilot's own job-matching score is computed over the same evidence kernel, so the generated document's coverage claim cannot drift from the Jobs page's match score.

## Validation: deterministic content checks

**Location:** [`apps/desktop/src-tauri/src/validate/content/`](../../apps/desktop/src-tauri/src/validate/content/)

The `validate` stage runs modules under [`apps/desktop/src-tauri/src/validate/content/`](../../apps/desktop/src-tauri/src/validate/content/), each keyed by the codes it emits. See [`CONTENT_ISSUE_CODES`](../../apps/desktop/src-tauri/src/validate/content/mod.rs) for the registry, where every code carries a severity: **Critical** or **Warning**.

### Critical vs. Warning: the line and why it sits there

**Critical** issues represent something provably wrong in the source material or the generated document's alignment with it. Every Critical comes from a **deterministic comparison**, never from a model opinion:

- Source consistency: dropped employment role, unsupported date (not in source history), altered link.
- Language alignment: generated text reads as a different language than the target.

**Warning** issues are advisory — they note things worth reviewing but do not block export:

- Unsourced terms (the word is in the generated text but neither the source résumé nor the job ad).
- Keyword density (a term repeats too often; ATS scoring prefers variety).
- Structure (a bullet is very long; a section is empty).

The line reflects a practical constraint: **warnings _can_ escalate to Critical only via measurement, never by assumption.** The eval harness measures Warning false-positive rates on truthful documents (see [Eval](#eval) below); escalation decisions are data-driven, not policy-driven. See [ADR-032](decision-records/adr-032-generation-pipeline-ownership-and-rule-enforcement.md) for language-mismatch Critical enforcement.

## Repair loop: bounded and transparent

**Location:** [`apps/desktop/src-tauri/src/pipeline/resume/stages/repair.rs`](../../apps/desktop/src-tauri/src/pipeline/resume/stages/repair.rs)

When the `validate` stage finds Criticals, the `repair` stage attempts to fix them. The loop is **bounded by [`Budget::max_repair_attempts`](../../apps/desktop/src-tauri/src/pipeline/budget.rs)**: failing sections only, up to [`MAX_SECTIONS_PER_ROUND`](../../apps/desktop/src-tauri/src/pipeline/resume/stages/repair.rs) per round. After each round, validation reruns. If the number of Criticals increases or stays the same, the loop exits (no progress → stop). Terminal Criticals (surviving repair) flow to the fabrication-review gate.

All repair attempts are logged at the run level; the repair rounds and their outcomes are visible in the persisted run detail.

## Fabrication gate: refuse to save rather than flag

**Location:** [`apps/desktop/src-tauri/src/commands/resume_pipeline/save.rs`](../../apps/desktop/src-tauri/src/commands/resume_pipeline/save.rs) · UI: [`FabricationReview`](../../apps/desktop/src/renderer/components/generation/QualityReportPanel/FabricationReview.tsx)

When a run leaves the `repair` stage with unresolved Criticals, its status is set to `needsReview`. The UI renders the [`FabricationReview`](../../apps/desktop/src/renderer/components/generation/QualityReportPanel/FabricationReview.tsx) component, which shows each flagged claim with two choices:

- **Remove:** Delete the line from the document (an actual edit, not a flag).
- **Keep:** Accept the claim as-is.

**Nothing is saved without explicit verdict.** The run cannot transition to `completed` or `done` until every flagged claim has been decided. This is the **fabrication gate**: we refuse to save silently; you decide what stays.

The verdict is recorded separately (in the run's [`PipelineRunDetail.report.fabrications`](../../packages/shared/src/ipc/index.ts)) from the document text, so a user can change their mind by re-opening and re-deciding (the text and verdicts are separate facts).

## Cost accounting and call budget

**Locations:**

- Limits: [`apps/desktop/src-tauri/src/limits/mod.rs`](../../apps/desktop/src-tauri/src/limits/mod.rs)
- Spend ledger: [`apps/desktop/src-tauri/src/pipeline/resume/ledger.rs`](../../apps/desktop/src-tauri/src/pipeline/resume/ledger.rs)

The pipeline enforces per-provider daily ceilings and per-run budgets. See the module doc in [`apps/desktop/src-tauri/src/pipeline/resume/mod.rs`](../../apps/desktop/src-tauri/src/pipeline/resume/mod.rs) for the per-stage call table (indicating which are base costs, optional, or bounded). The shape: a **fixed base of grounding stages** every run pays for, **optional stages gated on flags** (including `draft` and `cover_letter`), a **bounded repair loop** (see [`Budget::max_repair_attempts`](../../apps/desktop/src-tauri/src/pipeline/budget.rs)), and humanize (bounded separately). Each provider (Ollama, OpenAI, Anthropic, Gemini) has daily ceilings defined in [`limits/mod.rs`](../../apps/desktop/src-tauri/src/limits/mod.rs). Cost is tracked per-call in a ledger; the pipeline checks remaining budget before every call and returns an error if the budget is exhausted.

See [`Anti-abuse limits`](anti-abuse-limits.md) for the full budget shape and per-provider ceilings.

## Untrusted-text fencing

**Location:** [`ADR-010`](decision-records/adr-010-untrusted-input-fencing.md) · Implementation: [`packages/prompts/src/generate/emphasis/emphasis.ts`](../../packages/prompts/src/generate/emphasis/emphasis.ts)

Company research retrieved from the web is wrapped in an XML fence by [`buildCompanyResearchBlock`](../../packages/prompts/src/generate/emphasis/emphasis.ts), which instructs the model that the block is untrusted, web-sourced material to be used only for company context and to ignore any instructions it contains.

Every prompt template consuming company research **must** call [`buildCompanyResearchBlock`](../../packages/prompts/src/generate/emphasis/emphasis.ts) — passing raw brief text is a HIGH security finding. The fence pattern is tested; a regression fails the test suite.

## Search and ranking

**Location:** [`ADR-039`](decision-records/adr-039-hybrid-postings-search-lexical-dense-rerank.md) · Implementation: [`apps/desktop/src-tauri/src/commands/hybrid_search.rs`](../../apps/desktop/src-tauri/src/commands/hybrid_search.rs)

The Jobs page search combines three ranking surfaces: lexical FTS5 (BM25), optional dense embeddings (cosine similarity), and optional LLM reranking. This is **separate** from Autopilot's job-matching score (which uses keyword-coverage matching against the résumé).

**What hybrid search does:**

- Retrieves: when keyword search returns hits, ranks them lexically and (optionally) semantically. When keyword search finds nothing, retrieves the first `DENSE_CANDIDATE_MAX` postings in cache order (the constant lives in `apps/desktop/src-tauri/src/commands/hybrid_search.rs`) and ranks them densely.
- Fuses: RRF (Reciprocal Rank Fusion) combines lexical and dense rankings if both are available.
- Reranks: optional LLM listwise reranking of the top-K fused results (off by default; gated on `job_preferences.semantic_scoring`; K is [`RERANK_TOP_K`](../../apps/desktop/src-tauri/src/retrieval/rerank.rs)).

**What it does not do:**

- Does not persist posting text across scrapes (postings live in the in-memory cache, cleared on the next scrape).
- Does not retrieve postings outside the first candidate pool (bounded by `DENSE_CANDIDATE_MAX` in [`commands/hybrid_search.rs`](../../apps/desktop/src-tauri/src/commands/hybrid_search.rs)).
- Does not measure retrieval quality (no labelled dataset exists).

See ADR-039 for the full design, including the tradeoffs and measurement boundaries.

## Eval

**Location:** [`tests/eval.rs`](../../apps/desktop/src-tauri/tests/eval.rs) · Fixtures: [`src/validate/content/fixtures/`](../../apps/desktop/src-tauri/src/validate/content/fixtures/)

The eval harness measures **only** the deterministic validator layer. It does not measure generation quality (requires a live model) or retrieval quality (no labelled dataset). The harness uses planted-defect fixtures (each differs from a clean version by roughly one edit) and truthful documents; see [`tests/eval.rs`](../../apps/desktop/src-tauri/tests/eval.rs) for the assertions it enforces. The table is printed to stdout under `cargo test --test eval -- --nocapture` and is a **local artifact** — nothing downstream parses it. This reflects the repo's posture: state which is measured and which is not, keep the measurement honest, and measure deeper as more labelled data arrives.

## Related

- [ADR-032](decision-records/adr-032-generation-pipeline-ownership-and-rule-enforcement.md) — Generation-pipeline ownership and core-rule enforcement.
- [ADR-010](decision-records/adr-010-untrusted-input-fencing.md) — Untrusted-input fencing for web-sourced research.
- [ADR-039](decision-records/adr-039-hybrid-postings-search-lexical-dense-rerank.md) — Hybrid postings search design.
- [Anti-abuse limits](anti-abuse-limits.md) — Per-provider daily ceilings and budget shape.
- [Matching algorithm](matching-algorithm.md) — Autopilot's job-matching kernel (separate from search ranking).
