# ADR-017: Persisted, self-invalidating match-score & posting-vector caches

Last updated: 2026-06-14

**Status:** Accepted

## Context

The `match_resume` command (scoring a resume against a job posting via semantic embedding + ATS keyword coverage) was re-embedding the same job text on every call, because:

1. Job vectors were computed fresh every time; no durable cache existed.
2. Result scores (the combined semantic + ATS output) were never persisted, forcing recomputation across app restarts.

This meant cold-start on a list of 200 postings → 200+ Ollama embed calls, each blocking the UI. The recomputation is pure deterministic (no side effects), so caching is strictly safe.

## Decision

Add two new, self-invalidating SQLite tables to `DocumentStore` (`apps/desktop/src-tauri/src/documents/mod.rs`):

### `posting_vectors` table

- **Schema:** `(job_id TEXT PRIMARY KEY, text_hash TEXT, vector TEXT, provider TEXT, model TEXT, dim INTEGER, created_at INTEGER)`
- **Purpose:** Cache job-posting embedding vectors (one row per unique job ID).
- **Invalidation:** A row is a HIT iff `provider` + `model` match the active `EmbeddingConfig` AND `text_hash` matches the current job text (post-translation). A new provider/model or changed job text = miss.
- **Lifecycle:** Persists across app restarts. Evicted only when embedding space changes (`ai_set_embedding_config` detects via `embedding_space_changed()`).

### `match_scores` table

- **Schema:** `(resume_id, job_id, provider, model, semantic_enabled, formula_version, job_text_hash, score_json TEXT, created_at INTEGER)` with composite PK across all seven columns.
- **Purpose:** Cache the final `MatchScore` result (semantic/ATS/combined/gaps/recommendations).
- **Invalidation:** The composite key encodes every input that changes the score:
  - `resume_id`, `job_id` — specific match pair
  - `provider`, `model` — embedding space (if changed, all rows orphaned)
  - `semantic_enabled` — whether semantic scoring was skipped (0/1 flag)
  - `formula_version` — weighting constants or keyword-stem logic version
  - `job_text_hash` — SHA-256 of final job text (post-translation)
- **Lifecycle:** Persists. A hit requires exact match on all key columns; any change = miss. No sweep code needed.

### Central resolver: `posting_vector_or_embed()`

- **Signature:** `async fn posting_vector_or_embed(app: &AppHandle, job_id: &str, text: &str) -> Option<EmbeddingVector>`
- **Behavior:**
  1. Read embedding config + cached vector + compute SHA-256 (all before `.await` to avoid DB lock across async).
  2. Call `posting_vector_is_fresh()` — predicate that returns true iff space matches AND `text_hash` matches the stored hash.
  3. Cache hit → return cached vector; miss → embed, upsert to `posting_vectors`, return result.
- **Location:** `documents/mod.rs` (same as the table definitions).
- **Why separate from `PostingsCache.vectors`:** PostingsCache holds raw scraped text; a translated job text must never land there (the vector is for translated text, the cache key is "raw"). `posting_vectors` is for post-translation vectors only — the resolver's private cache.

### Match result caching in `match_resume.rs`

- **Path:** `commands/match_resume.rs` — wraps compute in cache check.
- **Formula version constant:** `const MATCH_FORMULA_VERSION: i64 = 1;` — bump when the `0.6 * semantic + 0.4 * ats` weighting, keyword stemmer logic, or any other scoring input changes.
- **Invariant (errors-never-cached):** Error early-returns (missing resume, missing job, fetch failure) MUST precede the first `get_match_score()` call. The cache is read+written only after those guards. Unit test: `errors_never_populate_match_scores_cache` in `documents/test.rs`.
- **Upsert:** On compute success, call `store.upsert_match_score(&cache_key, &s)` to persist the JSON result.

### SHA-256 hash function

- **Function:** `pub(crate) fn sha256_hex(text: &str) -> String` — deterministic, cross-session stable (unlike `DefaultHasher`).
- **Single source:** Both `posting_vectors.text_hash` and `match_scores.job_text_hash` key on this same hash.
- **Dependency:** `sha2 = "0.10"` (resolves to 0.10.9; already in use elsewhere in the codebase).

### Eviction on embedding-space change

- **Trigger:** `ai_set_embedding_config` detects via `embedding_space_changed(old, new)` helper — true if any field (provider/model/base_url) differs.
- **Action:** If true, call `store.clear_posting_vectors()` + `store.clear_match_scores()` to orphan all rows (old space entries are now unreachable; new embeds will miss and recompute).
- **Single source:** `embedding_space_changed()` lives in `documents/mod.rs` so the helper and its test are co-located.

## Trade-offs Evaluated

### 1. New `posting_vectors` table vs. reuse existing `vectors` table vs. JSON sidecar

**Chosen:** New table.

- `vectors` holds document embeddings (résumés); posting vectors are derived from post-translation job text. Mixing them violates separation of concerns and couples the foreign-key cleanup logic.
- JSON sidecar (on-disk next to a posting JSON file): no query support; full file rewrite on each upsert; harder to reason about consistency.
- New table: clean schema, indexed on job_id, queryable, transactional upserts alongside result cache.

### 2. Self-invalidating composite keys vs. event-driven invalidation

**Chosen:** Composite keys in `match_scores` PK.

- Event-driven (explicit sweep on config change): requires an eviction loop or TTL job; every new feature/field that affects matching must update the event handler or risk silent stale rows.
- Composite PK (formula_version + space + text_hash + semantic_enabled): every score-affecting input is encoded; a change to any produces a new key (miss). Insert-only documents make `resumeId` change a free miss (no old rows to sweep). Formula bumps are explicit one-liners.
- Downside: if a new field (e.g., `apply_stemmer_version`) is added to the scoring logic and the bump is forgotten, stale rows persist. Mitigation: unit tests that force a formula bump (test_score_changes_on_formula_bump).

### 3. Persisting results (`match_scores`) vs. recompute-from-vectors

**Chosen:** Persist results.

- Recompute-from-vectors: saves disk space; must recompute ATS (keyword extraction + stemming) and combine scores on every hit.
- Persist results: cold-start speed (one JSON deserialize vs. embedding + cosine + stemming), at the cost of unbounded growth (one row per unique (resume, job, space, formula, text) tuple).
- Unbounded-growth caveat: Addressed partly by space-change eviction; full TTL or per-document eviction tracked for Phase 3 (pre-embed-on-scrape will add a timestamp filter). Current mitigation: factory-reset + advisory UI warnings on large databases.

## Consequences

- **Cold-start:** Posting list with 200 items that were previously matched now hits the result cache; no embedding or scoring recomputation.
- **Embedding-space isolation:** `posting_vectors` rows are guaranteed to match the active space; a new provider/model triggers eviction and fresh embeds.
- **Self-invalidation:** Adding a new score-affecting input (stemmer version, keyword extraction change, weighting constant) is done by bumping `MATCH_FORMULA_VERSION` and updating the relevant code. No migration code needed; old rows naturally become unreachable.
- **Resettable registry coverage:** `DocumentStore::clear_all()` now wipes all four tables (documents, vectors, posting_vectors, match_scores) in one batch. Per ADR-009, the `Resettable` trait is extended; tests cover representative clear operations.
- **Storage grow unbounded:** Each unique (resume_id, job_id, provider, model, semantic_enabled, formula_version, job_text_hash) tuple adds a row. For typical usage (10–20 résumés, 100–500 scraped postings, 1 embedding space), this is negligible (~50–100 KB). A power user with 100 postings and 50 résumés in 2 spaces could accumulate 10 K rows (~5 MB). Phase 3 (pre-embed) will add more persistent data; full eviction / TTL strategy deferred to then.
- **Deferred:** Phase 3 will pre-embed postings on scrape (no job_text_hash key needed if every posting is unique); Phase 4 will add provider-aware scheduler concurrency; Phase 5 will batch-embed via a provider trait (avoid round-tripping single embeds). Current implementation is a stable foundation for all three.

## Phase K: Batch Scoring & Default Keyword-Only Path

**Status:** Shipped in Phase K (v0.100+).

The default scoring path is now **keyword-only** (`semanticScoring` defaults false → no embedding). The old per-row `ScoringScheduler` (CONCURRENCY=1) serialised N IPC calls and caused visible crawl even for cheap keyword-only work:

- **New command:** `match_resume_batch(resumeId, jobIds[], semanticScoringEnabled)` — scores all postings in **one Rust pass**; the per-job kernel (`score_one`) is shared with the legacy `match_resume` single-job path, ensuring identical logic.
- **Frontend:** `MatchScoresProvider` context distributes results per-row via `useRowMatchScore(jobId)` on-demand; `RowMatchScore` is now purely presentational (no scheduling logic).
- **Deleted:** `apps/desktop/src/renderer/providers/ScoringScheduler/` (dead) and `useJobMatchScores` batch hook (replaced by on-demand per-job `useRowMatchScore`).
- **Batch cap:** `MATCH_BATCH_MAX=1000` enforced in Rust (`commands/match_resume.rs`) — DoS guard against unbounded batch IPC.
- **Cache alignment:** The batch command shares the ADR-017 `match_scores` cache (keyword keys use `semantic_enabled=0` to keep keyword and semantic paths isolated in the composite PK).
- **Embedding-batch (Phase E):** Deferred — Ollama `/api/embed` batch + payload trim + warm-on-scrape are opt-in future work; do **not** ship or document as active.

## Testing

- Unit tests in `documents/test.rs`:
  - `posting_vector_cache_hit_on_space_and_text_match` — verify cache predicate.
  - `posting_vector_cache_miss_on_space_change` — confirm eviction.
  - `match_score_cache_hit_on_exact_key_match` — composite PK behavior.
  - `match_score_cache_miss_on_formula_bump` — formula version eviction.
  - `errors_never_populate_match_scores_cache` — error-path invariant.
  - `semantic_enabled_bit_consistency` — bit encoding matches cache key and skip logic.
  - `clear_all_wipes_caches` — Resettable coverage.
- Integration tests in `commands/test.rs`:
  - `match_resume_uses_cache_on_repeated_call` — end-to-end cache hit.
  - `match_resume_re_embeds_on_space_change` — eviction + fresh embed.

## References

- Implemented in: `apps/desktop/src-tauri/src/documents/mod.rs` (tables, helpers, store methods), `apps/desktop/src-tauri/src/commands/match_resume.rs` (MATCH_FORMULA_VERSION, cache wrap).
- Precursor work: ADR-009 (Resettable registry); ADR-003 (centralized net error layers); embedding storage schema in `documents/mod.rs`.
- Follow-up phases: ADR for Phase 3 (pre-embed-on-scrape) when it ships.
