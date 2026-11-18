# ADR-052 — Ghost-job signals from a persisted posting history

**Status:** Accepted

**Date:** 2026-11-18

**Deciders:** repo owner, contributor

## Context

Job boards recycle and abandon postings. Users want a hint that a listing has been open for a long
time or keeps coming back. Neither can be derived from a single scrape: it needs memory of what was
seen earlier. The dedup stores hold verdicts that a re-scrape rebuilds; an observation history is
different, because a past sighting cannot be re-observed.

## Decision

1. **Our own ghost key, not the canonical key.** Postings are matched by normalized company + title
   (`posting_history::ghost_key`), not by `canonical_job_key`, which is url-first. A repost usually
   arrives under a new url, so a url-first key would see a different posting. Trade-off: two distinct
   roles with the same company and title collapse into one history row.
2. **Repost = a return from the same source.** A repost counts only when the same source (a board
   plus the normalized search, `posting_history::source_id`) completed an ingest without the posting
   and the posting later returned there, after the gap. A board cut off at its item cap is not
   complete, so it is no evidence of absence and its marker does not advance. A first design used one global "last ingest"
   marker; outages, narrower searches, stopped runs and restores all looked like disappearances and
   produced false reposts. Known ceiling: a posting that moves between sources never counts. We trade
   recall for precision, because a false "reposted" badge is worse than a missed one.
3. **The history is persisted, backed up and synced; the markers are not.** Observations cannot be
   rebuilt, so the history travels in the backup bundle and syncs per record (ADR-051), and factory
   reset wipes it. Rows from a peer are clamped on the way in (`insert_conn`). The per-source markers
   are device-local: a marker says what this device scraped, which says nothing about another device,
   so they are never backed up or synced and a restore clears them. A row with no recorded source
   never counts as a repost, and a row written by sync carries none (the peer's source says nothing
   about this device's markers).

Signals are advisory only. They are shown and exposed to agents, and never filter, rank or hide a
posting.

## Consequences

- A synced peer can claim an older first-seen or replace a row, so signals can read louder as well as
  quieter. Accepted: they stay advisory.
- Recall is lower than a global rule would give, by design (decision 2).
- Thresholds are named constants beside the module; changing them needs no new decision.
- Agents read the same signals through the posting projections (see `docs/knowledge/agent-cli.md`).
