# Matching Algorithm — Single-Source Keyword Coverage Kernel

Canonical source: `apps/desktop/src-tauri/src/documents/keywords.rs` → `coverage_score()`

## Overview

The AI Job Hunter uses **two complementary scoring strategies**:

1. **Keyword-coverage scoring** (Autopilot + fast ATS screening): **pure keyword-based scoring** — embedding-free by default (when semantic scoring is disabled); when enabled, the top candidates re-rank through the combined kernel with per-job keyword-only degrade (see ADR-020 addendum).
2. **Combined scoring** (Jobs page analysis, and Autopilot's opt-in re-rank): a hybrid of semantic embedding similarity and the keyword ATS score — semantically heavier, but it requires an embedding lookup. The weights live in the code (see "Jobs Page Combined Score" below); this document deliberately does not restate them.

## Keyword-Coverage Kernel

The `coverage_score()` function (in `documents::keywords`) is the **single source of truth** for keyword-only scoring (the default Autopilot path). It powers:

- **Autopilot ranking** (`commands::autopilot::build_found_job` → `coverage_score()`): filters + sorts candidates by keyword match %.
- **ATS component** of the Jobs page combined score.
- **Gap analysis** in resume feedback (which skills are missing).

### Algorithm

For the exact algorithm steps, parameters, and implementation, see `apps/desktop/src-tauri/src/documents/keywords.rs` → `coverage_score()`. The implementation includes:

- Language detection via `whatlang`.
- Snowball stemming for the detected language (English, German, French, etc.).
- Keyword coverage: the share of the job's keyword set matched by the résumé (`|job ∩ résumé| / |job|`), rounded to a 0–100 percentage.
- Word-boundary detection to prevent false matches (e.g., "finance" vs. "refinance").
- Unstemmed, readable gap terms surfaced in match explanations.

## Autopilot Ranking

Two phases. **Phase 1 always runs and is embedding-free**; phase 2 runs only when the user has enabled semantic scoring app-wide (default OFF), in which case a scheduled run makes zero embed calls and does not even resolve the scoring state.

**Phase 1 — keyword prefilter** (`commands/autopilot.rs` → `build_found_job()`):

1. Fetch job postings.
2. For each job, call `coverage_score()` (cached result if in the `match_scores` table).
3. Filter by the `minMatchScore` threshold (cluster-aware, ADR-029 §g).
4. Sort by coverage % descending.

**Phase 2 — bounded semantic re-rank** (`commands/autopilot/rerank.rs` → `semantic_rerank_phase()`):

5. Re-score the top `SEMANTIC_RERANK_MAX` **cluster canonicals** through the same `match_resume::score_one` kernel the Jobs page uses — including the full pre-processing pipeline (translation, locale resolution), so the two "Match %" surfaces cannot disagree on the same pair.
6. Degrade per job, never per run: an embed/provider failure leaves THAT job on its keyword score and the loop continues. The daily ceiling, cancellation, a run of consecutive degrades, and a wall clock each stop the loop, leaving every unvisited job keyword-scored. A run never fails because of scoring.
7. Sort as **two blocks** — re-ranked head by combined score, keyword tail by coverage — because one axis over two scales would let a never-re-ranked keyword score outrank a re-ranked one.

**Autopilot's displayed score** is therefore per-job: a Low/Medium/High MatchBand whose variant (and tier cut points, and metric label) **flips with that job's `scoreSource`** — `coverage` for a keyword score, `combined` for a re-ranked one; '~'-prefixed and muted when provisional from an aggregator snippet. When one list holds both, each row also shows its metric so the two-block order does not read as a sorting bug.

## Jobs Page Combined Score

The Jobs page shows a **combined score** when analyzing a resume against a job. This hybrid approach weights semantic embedding similarity and keyword-based ATS scoring. See `apps/desktop/src-tauri/src/commands/match_resume.rs` → `score_one()` for the exact formula and weights.

This hybrid approach is slower (requires embedding lookup) but more semantically aware than keyword coverage alone.

## Caching

Both scores are cached in SQLite:

- `posting_vectors` table: stores embeddings (keyed by job_id, one row per posting); text_hash + provider/model columns pin the embedded text and embedding space so mismatches are cache misses.
- `match_scores` table: composite PK encodes formula version, so changes to the keyword algorithm automatically invalidate old cached results.

## Language Detection in Scoring

Before scoring, the pipeline detects the target language (the language the output résumé/letter must use). **Two independent detectors decide this question:**

- The **renderer** uses **franc** (`packages/shared/src/language-detection.ts`) to pick the target language from the job ad.
- The **Rust validation layer** uses **whatlang** (`apps/desktop/src-tauri/src/documents/keywords.rs::detected_language`) to verify the generated output matches the target.

When the two detectors disagree on the job ad's language, the validation guard goes quiet rather than raising a false Critical — consistent with the validation module's posture: a check that cannot be made reliably goes quiet rather than guesses. This is a real limit and belongs here for context: coverage score and keyword-only scoring use language detection via `coverage_score()`; the renderer's language choice and Rust's validation are not perfectly in sync, but disagreement is rare and is handled gracefully.

Language-specific stemming for keyword matching uses `languages_align` (`documents/keywords.rs`), a separate function from `detected_language`; the two ask different questions and must never drift. See `detected_language`'s doc comment and the language-validation module docs for details.

## Testing

Keyword-coverage tests live in `documents/keywords.rs::tests` (unit tests for stemming, matching, language detection), `commands/autopilot/tests.rs` (ranking uses the shared kernel; the phase-2 gate, cost bounds and degrade rules), and `commands/match_resume/test.rs` (the combined formula's weights, the degrade boundary, and the per-round-trip embed charge — all driven through the real `score_one` against a real `DocumentStore`). See ARCHITECTURE_STATUS.md for the full coverage.

## Intentional simplification: flat keyword coverage

`keyword_coverage()` in `documents/keywords.rs` weights every JD keyword equally. ATS knockout gating (hard-vs-nice-to-have distinction) and tiered keyword importance are **deliberately deferred**.

Rationale: the match score is a **guidance estimate** surfaced to the user, not a real ATS verdict. The UI frames it accordingly — the score helps the user decide whether to apply; it does not simulate the employer's ATS system. Implementing knockout gating would require reliable JD parsing for requirement tiers, which is outside the current scope.

If knockout gating is added in future, the entry point is `documents/keywords.rs::keyword_coverage` and the hybrid formula in `commands/match_resume.rs::score_one`.

## UI Rendering

Both metrics are rendered using the `MatchBand` component (`apps/desktop/src/renderer/lib/match-band.tsx`), which provides formula-aware visualization with variant-specific score thresholds. See the component's `scoreTier()` function for the current cut points; thresholds are tuned to the underlying formula (keyword-only vs. hybrid semantic+ATS).

## Related Decisions

- **ADR-020**: Unified autopilot scoring — explains why keyword-coverage is the single source for Autopilot.
- **ADR-022**: Atomic store transactions — covers caching strategy.
