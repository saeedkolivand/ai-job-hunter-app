# ADR-020: Unified autopilot scoring — keyword-coverage kernel + metric relabel

Last updated: 2026-07-16

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

- **Autopilot is simpler and faster:** no embedding calls, no in-memory `simple_similarity` overhead; ranking uses only the stemmed keyword set + one cache lookup per job.
- **Keyword coverage is now the canonical embedding-free scoring branch**, owned by the documents module and tested extensively in `documents/keywords.rs`. The formula is a single source of truth.
- **User expectation alignment:** The Autopilot % is now clearly labeled "keyword coverage" not "overall match %", preventing confusion with the semantic score on the Jobs page.
- **Formula versioning:** The composite PK in the match-score cache (`posting_vectors` + `match_scores` tables) includes the formula version, so a future change to the keyword algorithm automatically invalidates old cached results.
- **Trade-off:** Autopilot uses a pure keyword ranker, so jobs that score low on keywords but high on semantics (e.g., untraditional role descriptions) may be deprioritized. This is by design: Autopilot trades semantic sensitivity for speed and transparency (no black-box embedding call). The user can manually evaluate high-semantic-low-keyword jobs on the Jobs page.

## Related

- `docs/ARCHITECTURE.md` — updated to document the two scoring branches (keyword-coverage for Autopilot, combined for Jobs analysis).
- `docs/knowledge/matching-algorithm.md` — thin pointer to `documents::keywords::coverage_score`.
- `recommend/mod.rs` — batched keyword matching; `commands/autopilot.rs::build_found_job` — sorting logic.
