# ADR-029 — Cross-board job clustering: recompute-at-ingest, pair tombstones only

**Status:** Accepted
**Date:** 2026-07-21
**Deciders:** repo owner, main session

## Context

Stage-1 dedup (Trust Program PR E, 2026-07-10) collapses only exact `canonical_job_key` matches
via `dedup_cross_source` in the engine and `merge_found_jobs` in autopilot. Documented gap: aggregator
redirect URLs and board direct URLs never collapse; fuzzy company/title matching is absent. Example:
`Acme GmbH` + `Senior Rust Developer (m/w/d) – Berlin` vs `Acme` + `Senior Rust Developer` stays
two rows, two notifications.

Storage reality: no SQL postings table exists. Manual-scrape postings live in-memory in `PostingsCache`
(dropped after `commands/scrape.rs:199`); autopilot found-jobs are `Vec<FoundJob>` in `autopilots.json`;
`posting_vectors` is lazily populated on the match path and `FoundJob` has no posting id. A `cluster_id`
column on postings is therefore impossible; cluster state must reside elsewhere or be recomputed.

## Decision

### (a) Persistence: recompute-at-ingest, pair-tombstone verdicts only

Clustering is a deterministic pure function recomputed at every ingest. Cluster membership itself is
not persisted. Durable state = user "not a duplicate" verdicts only: unordered `canonical_job_key`
pairs in a new SQLite store `dedup.db` (`DedupStore`, table
`dedup_tombstones(key_a, key_b, created_at, PRIMARY KEY(key_a, key_b))` with invariant
`key_a < key_b`). Wired per ADR-022 (`db::open`, transactional migration), ADR-009 (`Resettable`),
and `DataStore` backup bundle (section key `dedupTombstones`). Cluster annotations are serde-defaulted
fields on `FoundJob` (persisted in `autopilots.json`, already in backups) and in-memory patches on
`PostingsCache` items. Non-destructive: no row deleted or hidden; members remain listed on the
canonical row.

### (b) One pure module `scraping/cluster/`, called from two L3 sites

`recluster_postings_cache(app)` is called after the engine returns in `commands/scrape.rs` and after
`scrape_url` single-adds, before `job_complete`. Also invoked on autopilot as a batch pass before
`minMatchScore` retain and a full-list pass inside `record_run` after `merge_found_jobs`. The engine
remains store-blind; tombstone queries require app state.

### (c) Matching: normalized company block + title first-token, then cosine or trigram-Jaccard

Blocking: exact normalized-company match AND normalized-title first-token match; no cross-block
comparison (kills Senior vs. Junior distinctions). Within a block: cosine similarity ≥
`CLUSTER_COSINE_MIN = 0.92` when BOTH candidates have cached `posting_vectors` (reusing `cosine`
from `commands/ai_provider`); otherwise trigram-Jaccard of normalized titles ≥
`CLUSTER_TITLE_TRIGRAM_JACCARD_MIN = 0.90`. Honesty: cached-vector hit rate ≈ 0 on fresh scrapes
and structurally 0 on autopilot (no posting id) — the string path is primary.

### (d) Normalization in-repo, no new crates

Company: fold + strip trailing legal suffixes (`gmbh & co. kg`, `gmbh & co kg`, `gmbh`, `ag`, `se`,
`kg`, `e.v.`, `ev`, `inc`, `llc`, `ltd`, `co` — whole trailing tokens only) + collapse
punctuation/whitespace. Title: fold + strip gender parentheticals (`(m/w/d)`-family, `(all genders)`,
bare trailing `m/w/d`) + trailing location/remote segment; never strip seniority. Folding: explicit
map `ä→ae`, `ö→oe`, `ü→ue`, `ß→ss` (e.g. Müller ≡ Mueller), ~30 Latin-diacritic entries, strip
combining marks U+0300–U+036F. Use trigram-Jaccard; do not introduce strsim or unicode-normalization
crates.

### (e) Canonical member preference: has_description > direct board > aggregator > newest, then key asc

`cluster_id` = first member's key; deterministic tiebreaker ensures stable clusters across reruns.
NO TypeScript mirror of normalization/clustering — Rust delivers opaque `clusterId` and
`clusterMembers[].key`; the renderer groups and echoes keys only. Existing `canonical-job-key.ts`
mirror remains untouched.

### (f) "New jobs: N" = count of clusters whose members are ALL first-seen this run

A known job resurfacing on another board contributes 0 to the new-jobs count. `tray::on_new_jobs`
receives cluster counts unchanged.

### (g) minMatchScore is cluster-aware

A cluster passes iff its best member passes score thresholds. Passing clusters keep ALL members
(a below-bar copy still contributes source chip, salary data). Fully-unscored clusters keep
today's keep-unscored behavior.

### (h) Split, no undo in v1

One IPC command `dedup_mark_not_duplicate { memberKey, otherKeys[≤32], autopilotId? }` inserts pair
tombstones then recomputes cache and that autopilot record. `assign_clusters` refuses a join when
a tombstone exists against ANY current member; splits survive every re-scrape. Recovery from a wrong
split is deferred (fast-follow).

### (i) Agency flag computed in Rust at ingest

Built-in const lists (Hays, Michael Page, Randstad, Adecco, Robert Half, Academic Work + tokens
`personalberatung|recruiting|staffing`) merged with new `JobPreferences.extraAgencyCompanies?: string[]`
(existing store/schema, dedicated single-column setter mirroring PR #695 `setSalaryExpectation`
pattern). Legal-suffix and gender-tag lists remain const (closed sets). Hide-filter is renderer session
state (`hideAgency` on `JobsSlice`).

## Alternatives rejected

- **Clusters+members SQLite store** — references ephemeral rows; second source of truth; drift risk;
  id-lifecycle gains for no user value.
- **strsim/unicode-normalization crates** — ~45 stdlib lines suffice; blocking does the
  discriminative work.
- **Clustering inside the engine** — engine is store-blind; threading tombstones through L2 adds
  complexity.
- **Cluster-scoped tombstones** — cluster ids shift under recompute; pair verdicts are stable and
  canonical.
- **TypeScript normalization mirror** — second drift-guarded lockstep for logic the renderer never
  executes.

## Consequences

One implementation serves both manual-scrape and autopilot surfaces; notification counts become cluster
counts (known job resurfacing no longer notifies). Split verdicts live in `dedup.db`, join backup
round-trip, and privacy reset via standard registries. `FoundJob` gains serde-defaulted fields (old
records load unchanged). Live-stream rows remain unclustered until the completion refetch (~1 s).
`minMatchScore` semantics are deliberately loosened per (g): a weak member can now hide behind a
strong cluster mate. The same rule also tightens one edge: a cluster whose best-scored member is
below the bar is dropped whole even if it contains an unscored member (previously the unscored copy
survived); this is intended and locked by test `mixed_cluster_with_below_bar_scored_representative_is_dropped_even_with_unscored_member`.
Normalization trade-off: the bare dash-tail strip (trailing segments after a dash) means same-company titles differing only by a single trailing word (team/department/tech qualifiers — e.g. Platform vs Payments suffix) normalize identically and cluster together; recovery is the user split (pair tombstone verdict). Cosine path is dormant until vectors exist at ingest time — making embeddings
primary would require ingest-time `ai_provider` calls, explicitly out of scope. Fast-follows
(recorded in `docs/knowledge/scraping-domain.md` at feature close): split-undo, ingest-time embeddings,
agency-list growth.

Owning symbols: `scraping/cluster/{mod,normalize}.rs`, `dedup/mod.rs` (`DedupStore`), `commands/scrape.rs::recluster_postings_cache`, `autopilot/mod.rs::record_run`, `commands/dedup.rs::dedup_mark_not_duplicate`.
