# ADR-039 — Hybrid postings search: lexical FTS5 + dense cosine + RRF fusion + optional LLM rerank

**Status:** Accepted

**Date:** 2026-09-01

**Deciders:** repo owner, main session

## Context

The Jobs page allows users to search through the current scrape's postings to find relevant opportunities. The initial implementation was keyword-only (lexical matching). A richer search experience requires combining lexical and semantic (embedding-based) retrieval, then optionally reranking through an LLM for user-facing applications.

Three questions drove the design:

1. **Where does the posting text live?** The app has no persisted posting-text store — `postings::PostingsCache` is in-memory and cleared on the next scrape. Persisting postings would require a new database, migrations, a retention policy, and a new privacy surface (ADR-027 support-bundle inclusion). Searching only the current scrape is acceptable: users apply to jobs before the next scrape finishes.

2. **Can lexical indexing be added without new dependencies?** FTS5 is already compiled into the shipped binary (`rusqlite` feature `bundled` passes `-DSQLITE_ENABLE_FTS5` unconditionally). The `vtab` feature was intentionally not enabled; that is for authoring custom SQLite virtual tables in Rust.

3. **Does semantic ranking need an ML runtime?** A cross-encoder (matching.reranker style) would require a tokenizer + inference library (candle/ort) linked into a three-OS Tauri build — a risk for binary size and CI complexity. The chosen path uses the existing LLM provider (Ollama or cloud) via the structured-completion route, adding zero egress hosts and working offline.

## Decision

**1. The corpus is the in-memory `PostingsCache`, rebuilt fresh per scrape, not persisted.** There is no posting text stored anywhere in the app (`postings/mod.rs:1-7`); the cache is a `Vec<Value>` that "intentionally avoids a full DB dependency", cleared on the next scrape. `posting_vectors` stores embeddings and a hash to detect text changes, but not text. Persisting postings was rejected: it would add a migration, a retention obligation, and a new ADR-027 support-bundle/privacy surface, in exchange for cross-session search. Consequence: search covers the current scrape only.

**2. FTS5 needs no new dependency — it is already in the binary.** `rusqlite` feature `bundled` → `libsqlite3-sys` passes `-DSQLITE_ENABLE_FTS5` unconditionally, so FTS5 is available at compile time. The `vtab` feature was explicitly not enabled; that is for authoring virtual tables in Rust. An ephemeral in-memory SQLite connection (opened fresh per search, dropped after) holds the FTS5 index — no persistence, no migrations.

**3. Vectors are `f32`, not the existing `f64`.** The in-memory `PostingsCache` holds live `EmbeddingVector`s and is bounded by `DENSE_CANDIDATE_MAX` (see `retrieval/mod.rs`); it is small enough that an approximate nearest-neighbor (ANN) index is unnecessary on scale grounds alone. `f32` halves memory and is lossless for cosine-distance ranking (no precision loss on angle). Rejected: adding an ANN library (hnsw, usearch, sqlite-vec) — unnecessary complexity for a small, ephemeral cache.

**4. Reranking is LLM listwise through the existing structured-completion path, not a cross-encoder.** A cross-encoder would require a tokenizer + ML runtime (candle/ort) in a three-OS Tauri build — rejected on binary size and build risk. The chosen path re-scores the top-K results via `commands::hybrid_search`'s `Reranker` port (a trait declared in `retrieval::rerank` and implemented in the command handler), using the existing provider routing. It adds zero egress hosts and works offline via Ollama.

**5. Ranking is scoped to an eligible-id allowlist.** The Jobs page applies cluster-canonical (ADR-029), agency, and work-type filters after the list is built. Ranking the whole cache would make "show me the top 10" render four rows, and the survivors would not be the ten best eligible postings. Eligible IDs are validated and filtered at L3 (the command layer) before the `retrieval` module receives any rows; ranking is then constrained to that already-eligible set.

**6. Degradation is reported, never hidden.** The `semantic_scoring` preference defaults to false (`job_preferences/mod.rs:257-268`), so lexical ranking is the default. If semantic scoring is enabled but embedding or reranking fails, the command returns an `arms` field in the reply with the structure `HybridSearchArms { lexical, dense, rerank }`, where each is one of `ran | skipped | unavailable`. A lexical-only list can never be presented as hybrid; the caller knows what surfaces it got.

## Consequences

### Positive

- **Zero new persistent schema.** The in-memory FTS5 index is built and dropped per search; `PostingsCache` holds embeddings in a `HashMap<String, EmbeddingVector>` keyed by job ID, with explicit invalidation via `remove()` when a posting's text changes (on `update_description` when the text differs, and on `add` only if the text differs from any existing cache entry).
- **Hybrid search is independent of Autopilot.** Autopilot uses `coverage_score()` (keyword keyword-coverage matching against a résumé); postings search ranks postings against a user query using lexical FTS5 and optional dense semantics. The two ranking surfaces do not interfere.
- **Offline semantic search via Ollama.** Reranking works without egress if the user has Ollama configured, same as generation and embeddings.
- **Graceful degradation on embedding failure.** If `get_embedding` fails or `semantic_scoring` is off, the reply carries that and the caller sees lexical rankings; no silent fallback.

- **The rerank gets its own limiter bucket** (`HYBRID_SEARCH_RERANK_BUCKET` in `limits/mod.rs`), rather than sharing the generation bucket, so a batch of Tailor generations cannot starve a job search or vice versa. Named here explicitly: the bucket inventory in `docs/knowledge/anti-abuse-limits.md` is a list that goes stale silently, and an omission there reads exactly like the overclaim this feature was built to fix.

### Tradeoffs

- **Search covers one scrape only.** Users cannot search across historical postings; the trade-off accepts this in exchange for no persistence complexity.
- **What this does and does not do.** The dense arm and rerank _re-order_ keyword-search results; they _retrieve_ only when keyword search returns zero hits across the entire corpus. Within that retrieval regime, only the first 40 postings (bounded by `DENSE_CANDIDATE_MAX`, documented in `retrieval/mod.rs`) are embedded and ranked. The rerank sees only the top-20 fused results, which in the mixed case (keyword hits exist) are keyword results only. A keyword miss is bridged only when the corpus-wide keyword search finds nothing, and then only over the first 40 postings in cache order. Widening the retrieval pool to the whole eligible set is a latency/cost tradeoff deliberately not made here.
- **Reranking is optional and defaults off.** Both the dense arm and the LLM rerank are gated on `job_preferences.semantic_scoring`, which reads `false` when unset. A default install runs lexical FTS5 only. This gate is enforced by a test which fails if the gate is removed.
- **Vectors are not persisted across scrapes.** Embeddings live in the in-memory cache; on the next scrape, if a posting URL changed but the text is the same, the embedding must be recomputed.

### What is measured and what is not

- **Ranking invariants** (RRF surfaces an id present in one list; BM25 column ordering; FTS5 operator safety) are asserted in CI tests.
- **Retrieval quality is not measured.** There is no labelled relevance dataset for this corpus, so the ranking order cannot be objectively scored against a ground truth.
- **Fusion and weight constants** (`RRF_K`, `BM25_WEIGHTS`, `RERANK_TOP_K`) are reasoned with cited sources (e.g. RRF literature, BM25 tuning guides), not tuned to this corpus. This repo's public posture on measurement (see `tests/eval.rs` header) is deterministic-first assertions with named budgets; extending that candour to retrieval is the intent here.

### Related decisions

- **ADR-020**: Unified autopilot scoring kernel (keyword-coverage). Postings search is a separate ranking surface using different algorithms.
- **ADR-029**: Cross-board job clustering. Postings search respects the cluster-canonical filtering and user's agency/work-type filters.
- **ADR-027**: Diagnostics-bundle privacy boundary. Postings search does not persist text, so no new privacy surface.
