# Matching Algorithm — Single-Source Keyword Coverage Kernel

Canonical source: `apps/tauri/src-tauri/src/documents/keywords.rs` → `coverage_score()`

## Overview

The AI Job Hunter uses **two complementary scoring strategies**:

1. **Keyword-coverage scoring** (Autopilot + fast ATS screening): embedding-free, deterministic keyword matching with language-aware stemming.
2. **Combined scoring** (Jobs page analysis): hybrid (0.6 semantic embedding + 0.4 keyword ATS).

## Keyword-Coverage Kernel

The `coverage_score()` function (in `documents::keywords`) is the **single source of truth** for embedding-free scoring. It powers:

- **Autopilot ranking** (`autopilot::ranking` → `coverage_score()`): filters + sorts candidates by keyword match %.
- **ATS component** of the Jobs page combined score.
- **Gap analysis** in resume feedback (which skills are missing).

### Algorithm

1. **Detect language** from job description (via `whatlang` library).
2. **Stem keywords** using Snowball for the detected language (e.g., English, German, French).
3. **Match candidate resume** against job keywords:
   - Extract all words from resume + job description.
   - Build stemmed keyword set from both.
   - Score: `coverage = intersection / union` (Jaccard).
   - **Word-boundary detection** prevents false matches (e.g., "finance" sector keyword matches only word-boundary "finance", not "refinance").
4. **Detect sector/role** only when keywords are insufficient (sparse job posting).
5. **Surface unstemmed, readable gap terms** (e.g., "AWS") instead of stemmed forms.

### Inputs & Outputs

```rust
pub fn coverage_score(
    resume_text: &str,
    job_text: &str,
    job_description: &str, // optional; used for language detection
) -> CoverageResult {
    coverage_percent: f32,     // 0–100
    matched_keywords: Vec<String>,
    gap_terms: Vec<String>,
}
```

## Autopilot Ranking

Autopilot's ranking pipeline (in `autopilot::ranking` + `recommend::batch_match`):

1. Fetch job postings.
2. For each job, call `coverage_score()` (cached result if in `match_scores` table).
3. Filter by `minMatchScore` threshold.
4. Sort by coverage % descending.
5. Return top results for the user to approve/apply.

**Autopilot's displayed "% match" = keyword-coverage %, clearly labeled "Keyword Coverage %" in UI** (distinct from the Jobs page "Match %" combined score).

## Jobs Page Combined Score

The Jobs page shows a **combined score** when analyzing a resume against a job:

```
Match % = 0.6 × semantic_score + 0.4 × ats_keywords_score
```

- **Semantic score**: embedding-based cosine similarity (0–1).
- **ATS keywords score**: derived from `coverage_score()` (0–100 %, converted to 0–1).

This hybrid approach is slower (requires embedding lookup) but more semantically aware.

## Caching

Both scores are cached in SQLite:

- `posting_vectors` table: stores embeddings (keyed by job_id + text_hash + embedding_space).
- `match_scores` table: composite PK encodes formula version, so changes to the keyword algorithm automatically invalidate old cached results.

## Testing

Keyword-coverage tests live in `documents/keywords.rs::tests` (unit tests for stemming, matching, language detection) and `recommend/tests.rs` (batch matching + ranking). See ARCHITECTURE_STATUS.md for the full coverage.

## Related Decisions

- **ADR-020**: Unified autopilot scoring — explains why keyword-coverage is the single source for Autopilot.
- **ADR-022**: Atomic store transactions — covers caching strategy.
