# ADR-036 — Cross-autopilot best matches: fuzzy clustering, rank by ADR-020 two-block rule

**Status:** Accepted

**Date:** 2026-08-27

**Deciders:** repo owner, main session

## Context

[ADR-029](adr-029-cross-board-job-clustering-recompute-at-ingest.md) established clustering as a recompute-at-_ingest_ operation over a **single record's** found jobs. A user running two autopilots (e.g., one for "Rust" and one for "Berlin") can see the same job twice — once from each autopilot's last run. De-duplication happens only within a single autopilot's state.

A cross-autopilot surface (a Best Matches page) aggregates found jobs across ALL non-archived autopilots into a deduplicated ranked list. This requires three decisions:

1. Whether to merge results at the renderer (IPC-free; data already in React Query cache) or in Rust.
2. How to rank a cross-record result set when ADR-020's two-block rule applies per-kernel, not per-union.
3. Whether clustering happens at ingest (impossible — belongs to no single record) or query time (new).

## Decision

### (a) Query-time clustering, union deduplicated before clustering

Best matches computed at **request time** over a union of all autopilots' found jobs. The union is
deduplicated on `canonical_job_key` before clustering — exactly as `merge_found_jobs` dedupes per
record — to ensure `cluster_id` is globally unique by construction (fixing a pre-existing defect where
two items with the same key could seed two clusters with the same id).

Paused autopilots contribute rows to the union. Archived autopilots do not (they are dropped at
load time per ADR-022).

**Why not renderer-side:** All three alternatives (flat de-dup by URL, clustering via existing
`canonical_job_key` table, merging via the React Query layer's existing `autopilot_list` payload)
were measured and rejected:

- Flat de-dup loses salary/board data from all but the first source.
- Renderer-side clustering requires either a **second** TypeScript normalization/clustering mirror
  (immediate drift risk, violates ADR-029's "NO TypeScript mirror") or shipping the clustering
  delta from Rust and applying it on the already-cached payload (architectural coupling between
  two independent cache strategies).
- React Query invalidation to trigger the merge reads authoritative state from a cache that may be
  stale (a long-running session with no new autopilot-list refetch will merge against outdated
  data). Honest fuzzy matching requires fresh query-time visibility.

### (b) Ranking: tier-first is a contradiction; use ADR-020 two-block rule instead

**Original plan error:** "rank tier-first" and "population = qualifying tier High" are mutually
inconsistent. `qualifies` keeps only rows scoring >= its kernel's high cut; every survivor has
`tier_rank = 2` (High). The tier term is a constant for all matches. A comparator reducing to
`score_desc` across two scales (Coverage/Combined) violates [ADR-020](adr-020-unified-autopilot-scoring-kernel.md)'s
addendum: never order a Combined and Keyword row by score alone.

**Fix:** Use [ADR-020](adr-020-unified-autopilot-scoring-kernel.md)'s **two-block rule**:

1. Combined-scored blocks first (highest scale).
2. Keyword-scored blocks second (lower scale).
3. Within each block: score desc, then `key` asc (stable tiebreaker).

This ensures genuine semantic hits (Combined/full-JD) sort ahead of fuzzy matches (Keyword),
regardless of their individual numeric scores. A keyword match scoring 95 will not displace a
combined match scoring 75.

### (c) Population: select-then-qualify per-member

The best member is selected via the two-block rule (§(b): Combined block always beats Keyword
block regardless of score), then qualification is tested against **that member's own kernel's**
high cut. This is a select-first, qualify-second order, not a qualify-by-OR-across-members order.

**Example:** a cluster with one member scoring Combined 40 and another scoring Keyword 60 has
best member = Combined 40 (the block rule chooses Combined). Since Combined's cut is 75 (from
`MATCH_TIER_CUTS.combined.high` in `packages/shared/src/schemas/index.ts`, generated to Rust
in `ipc_contracts/match_tiers.rs`), the cluster is dropped: 40 < 75. This is the conservative
direction intentionally — a semantic verdict of 40 is more trustworthy than a snippet-derived
Keyword score of 60. The cluster would be dropped even if it contained an unscored member; this
mirrors the tightening ADR-029 §(g) documents for `minMatchScore`.

Payload cap (defined in `best_matches.rs`) is a guard on wire size and renderer latency, not a
selection criterion. Qualifying count before the cap is reported as `total`, so the UI can surface
"N results, showing {cap}" if `total > matches.length`.

### (d) Cross-autopilot sources and aggregation

Each row carries an array `sources: AutopilotBestMatchSource[]` listing every autopilot that
surfaced the job. At minimum, one source; at maximum, the count of non-archived autopilots at query
time — paused autopilots contribute, so "active" would understate the ceiling.

`autopilotCount` reports the count of **distinct autopilots contributing at least one qualifying
row**. A single autopilot surfacing 5 qualifying jobs increments the count by 1.

### (e) Dismissed jobs

A dismissal in the Best Matches surface persists to the `InteractionStore.upsert` the exact same
way a dismissal in a single-autopilot row does. Dismissal identity matches on `canonical_job_key`
derived from the persisted job's `url + title + company`, tested against every `cluster_members[i]`
(including the canonical) — per-member matching, not cluster-id matching. This decouples the
dismiss logic from cluster stability: a row dismissed, then its autopilot archived/re-run/re-seeded,
will still read as dismissed on the next refetch.

### (f) Paused vs archived distinction

A paused autopilot is still in `AutopilotStore` with `paused: true`. Its found jobs are included
in the union and eligible for clustering, so a job it originally sourced still appears in Best
Matches (attributed to it in `sources` with `paused: true`). The user can see what the paused
autopilot has found and take action without resuming it.

An archived autopilot is removed from the store and invisible at query time — its found jobs are
not included. (Archived state is load-time terminal per ADR-022; restoration is out of scope.)

### (g) Interaction type: `'dismissed'`

A fifth value `'dismissed'` joins `['viewed', 'opened', 'applied', 'bookmarked']` in the
`interactionType` union (shared across the application). Every surface that enumerates interaction
types (`INTERACTION_TYPES` constant, `InteractionRow` renderer, etc.) must include a `dismissed`
entry with icon, label, and i18n keys. Unknown types coerce to `'viewed'` in the persistence layer;
an explicit `"dismissed"` arm prevents silent read-back as viewed.

## Alternatives rejected

| Alternative                                                              | Why rejected                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Flat de-dup by URL in the renderer                                       | Loses valuable breadth: salary, board, source attribution from all but the first occurrence. A user seeing "Acme: Rust Engineer $150k" should know if it appeared on three boards.                                                                                                                                                                                                                                                  |
| Renderer-side clustering (reuse `canonical-job-key.ts`)                  | Requires either (a) a second TypeScript normalization/clustering mirror (immediate drift risk against `scraping/cluster/`), or (b) shipping clustering deltas from Rust and merging onto stale cache data. (a) violates ADR-029; (b) couples two cache strategies and hides real-time changes.                                                                                                                                      |
| React Query layer merge (invalidate on set, re-fetch on compare)         | Invalidation triggers a refetch; the refetch is authoritative for freshness. But the merge runs DURING the fetch (comparing old data to new to avoid duplication). Timing race: long-running sessions can merge against data older than the last fresh fetch, silently losing a cross-autopilot duplicate that arrived since.                                                                                                       |
| Tier-first ranking                                                       | Tier is constant for all qualifying rows (all are tier High). Comparator degenerates to score-only, violating ADR-020's explicit rule against cross-scale ordering.                                                                                                                                                                                                                                                                 |
| Single-scale ranking (all Combined, all Keyword, keep unscored separate) | Autopilots run independently; a job may score Keyword via one and Combined via another (different engines, prompts, datasets). Union merges across scales by construction. Lossy separation requires picking a "winner" scale per job, which either discards half the matches (pick best source) or re-ranks per source (multiplying result count). Single-scale surfaces (individual autopilot's found-job list) remain unchanged. |

## Consequences

- **Honest fuzzy matching at query time.** Fresh visibility into the union avoids stale-cache merge races and hidden duplicates. Cost: per-request clustering (53 ms at 7.2k jobs, 159 ms at 15k, quadratic for single-job blocks past ~2000 items). Mitigated by making the command `async` and running in a thread pool, not on the IPC thread.
- **Deduplication one level up.** `merge_found_jobs` (per-record) + union dedupe (before clustering) ensures the invariant "one canonical key per cluster" by construction, fixing a pre-existing defect in ADR-029's implementation.
- **Two-scale ranking enforces the semantics ADR-020 intended.** Combined hits sort ahead of Keyword regardless of score, so a 40-point semantic match will not be truncated by a 60-point fuzzy match.
- **Dismissed state survives re-clustering.** A job dismissed, then its autopilot re-run/re-seeded (new cluster id), will still read as dismissed because dismissals match on member identity, not cluster id. Non-destructive.
- **Paused autopilots remain visible.** Useful for auditing what a paused search found without resuming it. Archived autopilots drop (terminal state per ADR-022).
- **Interaction type unification.** A fifth enum member `'dismissed'` requires every enumeration to be updated (compile-time catch at the time). This makes future additions similarly obvious (unlike using string literals, where a new type needs implicit fallbacks in parsing and explicit update in every surface separately). Lessons recorded on this shape for the next person (a helper lifting from per-record to cross-record contract invalidates its preconditions).
- **IPC contract change.** New command `AutopilotContract.bestMatches()` + `AutopilotBestMatchesResult` + `AutopilotBestMatch` + `AutopilotBestMatchSource`. Owning symbols: `packages/shared/src/ipc/contracts/autopilot.ts`, `apps/desktop/src-tauri/src/commands/autopilot/best_matches.rs`, `apps/desktop/src/renderer/features/best-matches/`, `apps/desktop/src/renderer/routes/best-matches.tsx`.

## Related

- [ADR-020](adr-020-unified-autopilot-scoring-kernel.md) — Two-block ranking rule, high/medium cuts per scale.
- [ADR-029](adr-029-cross-board-job-clustering-recompute-at-ingest.md) — Per-record clustering at ingest, tombstone verdicts, canonical preference.
- [ADR-022](adr-022-atomic-store-transactions-and-centralized-db.md) — Transactional stores, backup bundles, load tolerance.
- `apps/desktop/src-tauri/src/commands/autopilot/best_matches.rs` — Rust implementation (pure `compute_best_matches`, async IPC wrapper).
- `apps/desktop/src/renderer/features/best-matches/` — Renderer surfaces, sort options, empty states.
- `docs/knowledge/automation-domain.md` → Best Matches subsection (shape and IPC contract).
