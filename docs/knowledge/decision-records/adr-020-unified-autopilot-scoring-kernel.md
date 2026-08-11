# ADR-020: Unified autopilot scoring — keyword-coverage kernel + metric relabel

Last updated: 2026-08-11

**Status:** Accepted

## Context

Two incompatible "% match" metrics existed:

1. **Autopilot ranking** used a naive Jaccard `simple_similarity` (unweighted intersection ÷ union) embedded-free on job keywords.
2. **Jobs page** showed a _combined_ semantic score (0.6·embedding_cosine + 0.4·ats_keywords).

Autopilot's `simple_similarity` was crude (failed to weight keyword importance, conflated "finance" skill with finance-sector keywords) and never aligned with the full `score_one` semantics. Users saw a single number on both pages but could not map their understanding of "match" between them.

## Decision

**Unify scoring: Autopilot now ranks using the shared `documents::keywords::coverage_score` kernel** (the ATS keyword algorithm used by the Jobs page for the `ats` component of the combined score).

**Delete `simple_similarity`.** The keyword-coverage algorithm is the canonical **embedding-free keyword-based ranker** — see `apps/desktop/src-tauri/src/documents/keywords.rs` → `coverage_score()` for the implementation. It is embedding-free, deterministic, and zero API calls (safe for headless Autopilot).

**Autopilot's displayed "% match" is now pure keyword-coverage (embedding-free), NOT the Jobs page combined metric.** The Jobs page combines semantic + keyword signals (see `apps/desktop/src-tauri/src/commands/match_resume.rs` → `score_one()` for the exact weights); Autopilot uses keyword coverage alone. Rename the Autopilot metric in UI/analytics as "Keyword Coverage %", clearly distinct from "Match %" (the combined Jobs metric). The two metrics are complementary: Autopilot ranks fast and deterministically on keywords alone; the Jobs page weighs semantic meaning more heavily.

## Consequences

- **Autopilot is simpler and faster (by default):** when semantic scoring is disabled, no embedding calls; when enabled, the re-rank is bounded by top-N and a wall clock. No in-memory `simple_similarity` overhead in either path; ranking uses only the stemmed keyword set + one cache lookup per job (pre-filter) or reuses cached embeddings (post-filter).
- **Keyword coverage is the canonical keyword-only scoring branch**, owned by the documents module and tested extensively in `documents/keywords.rs`. The formula is a single source of truth for the default path.
- **User expectation alignment:** The Autopilot % is now clearly labeled "keyword coverage" not "overall match %", preventing confusion with the semantic score on the Jobs page.
- **Formula versioning:** The composite PK in the match-score cache (`posting_vectors` + `match_scores` tables) includes the formula version, so a future change to the keyword algorithm automatically invalidates old cached results.
- **Trade-off (by default):** When semantic scoring is disabled, Autopilot uses a pure keyword ranker, so jobs that score low on keywords but high on semantics (e.g., untraditional role descriptions) may be deprioritized. This is by design: Autopilot trades semantic sensitivity for speed and determinism when the user has opted out of embeddings. When enabled, re-ranking runs through the same combined kernel as the Jobs page (transparent formula, capped N). The user can always manually evaluate any job on the Jobs page.

## Addendum — opt-in semantic re-rank (2026-08-11)

When the user enables semantic scoring app-wide, Autopilot ranking adopts a two-phase design: a keyword-coverage prefilter (unchanged: `coverage_score`, keyword filters, `minMatchScore`, cluster dedup) followed by a bounded semantic re-rank of the top `SEMANTIC_RERANK_MAX` (20) cluster canonicals through `match_resume::score_one`, the same combined kernel the Jobs page uses. **When semantic scoring is OFF (the default), Autopilot behaves exactly as ADR-020 describes: zero embedding calls, deterministic keyword-only ranking, safe for headless scheduling.**

The re-rank phase is controlled by the `job_preferences.semantic_scoring` toggle, charged per actual provider round-trip against the shared `PROVIDER_DAILY_MAX`, and bounded by a wall clock (`SEMANTIC_RERANK_MAX × 15s`), cancellation, and explicit per-job degrade rules. A per-job embed/provider failure downgrades that job to its keyword score with `scoreSource: keyword`; the loop continues. The result is two blocks (re-ranked head by combined score, keyword tail by coverage), preventing a never-re-ranked keyword score from outranking a re-ranked one. The label `autopilot.scoreLabel.{coverage,combined}` flips per job with `scoreSource`.

The pre-processing pipeline (translation via `translate_if_needed`, locale resolution) runs for every surface that renders "Match %" — Jobs page and Autopilot both — via `MatchSurface::translates()`, ensuring the two surfaces cannot produce different "Match %" for the same pair. The Extension stays zero-egress by structural design (`score_adhoc_keyword_only`).

A pass the wall clock cuts off still reports its PARTIAL counts plus a distinct `rerank_timeout` step: it has already spent embeds and promoted jobs, so reporting nothing would make the run's step log read as keyword-only.

Implementation: `commands/autopilot/rerank.rs::semantic_rerank_phase` (phase 2 entry point — split out of `commands/autopilot.rs` for the LOC cap, no behaviour change), `commands/match_resume.rs::score_one` (shared kernel), `documents::embed_charged` (per-round-trip charge choke point), `RERANK_DEGRADE_BREAKER = 3` (consecutive degrade limit per run). See `commands/autopilot/tests.rs` and `commands/match_resume/test.rs` for mutation-verified guards on cache identity, budget enforcement, and the mixed-shape degrade boundary. Related: `docs/knowledge/matching-algorithm.md`, and the "Ranking via keyword-coverage" row in `docs/ARCHITECTURE_STATUS.md`.

## Related

- `docs/ARCHITECTURE.md` — updated to document the two scoring branches (keyword-coverage for Autopilot, combined for Jobs analysis).
- `docs/knowledge/matching-algorithm.md` — thin pointer to `documents::keywords::coverage_score`.
- `recommend/mod.rs` — batched keyword matching; `commands/autopilot.rs::build_found_job` — sorting logic.
