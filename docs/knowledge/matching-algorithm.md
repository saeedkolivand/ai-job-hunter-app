# Matching Algorithm — Single-Source Keyword Coverage Kernel

Canonical source: `apps/tauri/src-tauri/src/documents/keywords.rs` → `coverage_score()`

## Overview

The AI Job Hunter uses **two complementary scoring strategies**:

1. **Keyword-coverage scoring** (Autopilot + fast ATS screening): **pure keyword-based scoring** — embedding-free, deterministic, zero API calls, safe for headless scheduling.
2. **Combined scoring** (Jobs page analysis): hybrid (**60% semantic embedding similarity + 40% keyword ATS**), semantically heavier but requires embedding lookup.

## Keyword-Coverage Kernel

The `coverage_score()` function (in `documents::keywords`) is the **single source of truth** for embedding-free scoring. It powers:

- **Autopilot ranking** (`autopilot::ranking` → `coverage_score()`): filters + sorts candidates by keyword match %.
- **ATS component** of the Jobs page combined score.
- **Gap analysis** in resume feedback (which skills are missing).

### Algorithm

For the exact algorithm steps, parameters, and implementation, see `apps/tauri/src-tauri/src/documents/keywords.rs` → `coverage_score()`. The implementation includes:

- Language detection via `whatlang`.
- Snowball stemming for the detected language (English, German, French, etc.).
- Keyword coverage: the share of the job's keyword set matched by the résumé (`|job ∩ résumé| / |job|`), rounded to a 0–100 percentage.
- Word-boundary detection to prevent false matches (e.g., "finance" vs. "refinance").
- Unstemmed, readable gap terms surfaced in match explanations.

## Autopilot Ranking

Autopilot's ranking pipeline (in `autopilot::ranking` + `recommend::batch_match`):

1. Fetch job postings.
2. For each job, call `coverage_score()` (cached result if in `match_scores` table).
3. Filter by `minMatchScore` threshold.
4. Sort by coverage % descending.
5. Return top results for the user to approve/apply.

**Autopilot's displayed "% match" = keyword-coverage %, clearly labeled "Keyword Coverage %" in UI** (distinct from the Jobs page "Match %" combined score).

## Jobs Page Combined Score

The Jobs page shows a **combined score** when analyzing a resume against a job. This hybrid approach weights semantic embedding similarity and keyword-based ATS scoring. See `apps/tauri/src-tauri/src/commands/match_resume.rs` → `score_one()` for the exact formula and weights.

This hybrid approach is slower (requires embedding lookup) but more semantically aware than keyword coverage alone.

## Caching

Both scores are cached in SQLite:

- `posting_vectors` table: stores embeddings (keyed by job_id + text_hash + embedding_space).
- `match_scores` table: composite PK encodes formula version, so changes to the keyword algorithm automatically invalidate old cached results.

## Testing

Keyword-coverage tests live in `documents/keywords.rs::tests` (unit tests for stemming, matching, language detection) and `recommend/tests.rs` (batch matching + ranking). See ARCHITECTURE_STATUS.md for the full coverage.

## Intentional simplification: flat keyword coverage

`keyword_coverage()` in `documents/keywords.rs` weights every JD keyword equally. ATS knockout gating (hard-vs-nice-to-have distinction) and tiered keyword importance are **deliberately deferred**.

Rationale: the match score is a **guidance estimate** surfaced to the user, not a real ATS verdict. The UI frames it accordingly — the score helps the user decide whether to apply; it does not simulate the employer's ATS system. Implementing knockout gating would require reliable JD parsing for requirement tiers, which is outside the current scope.

If knockout gating is added in future, the entry point is `documents/keywords.rs::keyword_coverage` and the hybrid formula in `commands/match_resume.rs::score_one`.

## Related Decisions

- **ADR-020**: Unified autopilot scoring — explains why keyword-coverage is the single source for Autopilot.
- **ADR-022**: Atomic store transactions — covers caching strategy.
