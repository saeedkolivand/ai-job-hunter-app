# Persistence Layer — State Ownership, SQLite, Transactions, Atomicity

Last updated: 2026-08-18

Canonical sources:

- `apps/desktop/src-tauri/src/db.rs` — centralized connection + WAL setup
- `apps/desktop/src-tauri/src/data_store.rs` — trait definition
- Individual stores: `ai_generations/mod.rs`, `applications/mod.rs`, `documents/mod.rs`, etc.
- JSON-file stores: `autopilot/mod.rs`, `postings/mod.rs`, `notifications/mod.rs`
- In-flight status + reconciliation: `jobs/mod.rs`, `pipeline/runs/mod.rs`
- Enforcement: `scripts/check-event-subscriptions.mjs`

## State ownership: the persisted record is authoritative, events are notifications

Every long-running unit of work in this app lives in Rust and outlives the UI that
started it — a board scrape, an autopilot run, a résumé pipeline, an Ollama model
pull, an embeddings re-index. So:

> **The durable record is the truth. An event only says that the record changed.**

A surface reads the record; it may use the event to know _when_ to re-read.

**The converse is the half that gets skipped: an event may be missed.** A listener
is registered on mount and torn down on unmount, but the Rust work it describes
does not unmount with it. Everything emitted while the user was on another route —
_including the terminal event_ — is delivered to nobody, and a React Query
`onSuccess` invalidation belongs to an observer that no longer exists, so the
finished result never reaches the cache either.

Therefore **nothing may exist only as an event.** If a surface cannot re-derive its
state from a durable source on mount, that is a defect, not a style choice.

### Where "durable" lives — the transient boundary

Durable does not have to mean "on disk", and the question is not
important-versus-unimportant. It is:

> **Is there work outside this process that outlives the component?**

**No → component-local state is correct.** A live activity feed is a view of the
current moment: there is no per-run state to preserve, and a missed event costs it
no history it could have shown. Two entries in the subscription inventory are
recorded as accepted for exactly this reason — `features/monitoring/hooks/useActivityFeed.ts`,
and `features/dashboard/components/AISystemStatus/index.tsx`, whose reading is
re-derived from the job registry on mount.

**Yes → the state needs an owner that outlives the mount, plus a handle to
re-attach with.** In order of preference:

1. **Re-read the backend record.** Nothing is kept in sync, so nothing can desync:
   the autopilot card falls back to the persisted `runStatus` when this mount has
   no local state.
2. **Keep only the handle in the app-lifetime session store**
   (`renderer/store/session-store/`, in memory for the life of the process) and
   re-derive the rest from the backend on mount. **ADR-006 records the decision to
   have one app-wide session store, but do not read it for the rule — it is stale
   on both path and mechanism, and on this point it says the OPPOSITE**: that the
   store holds each session's streamed text and that surfaces "never duplicate
   session state locally". This section is the current rule; ADR-006 needs an
   amendment or a superseding record. The jobs scrape keeps
   `scrapeJobId` and lets a watchdog poll the authoritative terminal state;
   TailorFlow keeps `applicationApply.applyRun` and hands it back as
   `initialRunId`/`initialJobId`, after which the run's persisted event trail
   replays the stage counter.
3. **Store a derived value only alongside what it is derived from.** Two lifetimes
   desync: the jobs page kept its search signature but left the scrape form it is
   computed from in `useState`, so a route change reset the criteria, the
   comparison legitimately said "different search", and the append-only "Show more"
   button wiped the user's whole result set.

Legitimately transient even inside case 2: a streaming buffer the backend cache
re-hydrates, and refs that cannot outlive the call that needs them. **Still
event-only today:** the résumé pipeline's streamed draft text — a remount recovers
the run, the stage and eventually the saved document, but not the tokens that
streamed while the panel was unmounted.

### A persisted record can also be wrong: reconcile it at open

If the process dies mid-run, `status = running` outlives the run. Reading an
unreconciled record is reading a lie, so any store that records in-flight status
owes a sweep when it opens. Today they disagree, and the gaps are known:

| Record             | Sweep at open                                                                                                                                                                                             |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `jobs.db`          | Yes — in-flight statuses → `failed` / "Interrupted by app restart". Bounded by a 24 h `created_at` cutoff, so an older row stays `running` forever, invisibly: the same cutoff excludes it from the load. |
| `autopilots.json`  | Yes, unbounded — `mark_interrupted_runs` flips `InProgress` → `Interrupted` and returns the ids so the scheduler can retry.                                                                               |
| `pipeline_runs.db` | **None.** `PipelineRunStore::open` runs migrations and a URL normalizer, nothing else.                                                                                                                    |

The last one is user-visible, not cosmetic. `runs_for_job` orders by `started_at DESC`,
so a killed run stays the newest run for that posting, and `ensure_latest_run` keys
on **recency, not status** — every section regenerate and fabrication resolve for
that posting is then refused, telling the user to wait for a run that will never
finish. Adding a status sweep alone does not close it, because `ensure_latest_run`
never looks at status. Pinned by `tests/pipeline_kill_recovery.rs` (a real process
kill, with the `jobs.db` sweep as the positive control) and by the lockout test in
`commands/resume_pipeline/test.rs` — both written to go **red when the gap is
fixed**, so read such a failure as "closed, update the test".

### Accepted inconsistencies

Three stores sit outside the SQLite rules below entirely: whole-file JSON in the
data dir, an in-memory cache under a mutex, and mutators that cannot fail. No
`db::open`, no transaction, no WAL. The rationale recorded in the source is that a
small, bounded record set does not justify a DB dependency, and that a store with
no Tauri imports stays unit-testable without a runtime.

They are written down because an accepted exception is a decision, while an
undocumented one is indistinguishable from a bug. What they are inconsistent
_about_ is not only SQLite — it is each other:

| File                 | On write                                                   | On a corrupt file                                                                                                       | Failure surfaces as | In the backup bundle                                              |
| -------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ------------------- | ----------------------------------------------------------------- |
| `autopilots.json`    | `fs::write` in place, skipped when the bytes are unchanged | Per record — unparseable records are dropped and counted                                                                | `log::error`        | Yes (`autopilots`)                                                |
| `interactions.json`  | Temp file + rename, so the replace is atomic               | Moved aside to `interactions.json.corrupt`; if that rename fails, saves are blocked rather than overwrite the only copy | `log::error`        | Yes (`interactions`, exported inline rather than via `DataStore`) |
| `notifications.json` | `fs::write` in place                                       | Swallowed — a parse failure reads as an empty list, which the next save then writes over                                | `log::warn`         | **No**                                                            |

The last column is the one worth knowing. Notifications are wiped by factory reset
(the store is `Resettable`) but are not carried by backup/restore, because
`NotificationStore` is not a `DataStore` and the bundle has no section for it.
`EmailWatchStore` and `KvCache` are outside the bundle for the same structural
reason. Defensible — a capped, device-local list — but a decision, so it is
recorded here rather than left to be rediscovered.

Not in scope as exceptions: the small single-purpose JSON files that hold settings
or device state rather than records — `credential-meta.json`, `crash-reporting.json`,
`scraping-settings.json`, `locale.json`, and per-board `browser-state/<board>/`
cookies and auth status. They were never store candidates.

### Enforcement

This document is not the only thing holding the rule up:

- **AGENTS.md rule 16** states the renderer half: mount subscriptions from
  `routes/__root.tsx` or a provider, and read in-flight truth from the backend.
- **`pnpm check:event-subscriptions`** (`scripts/check-event-subscriptions.mjs`, run
  in the `lint-format` CI job) discovers the subscription-hook family from
  `services/` and fails until every subscribing file outside it is declared with its
  mount lifetime. A `route-scoped` entry needs a note saying what is dropped.
- **The `route-scoped` entries are a debt list, not an approval list.** Each names
  what that file loses today; the ones marked ACCEPTED are the transient-boundary
  cases above. Fixes land per feature, and the list cannot silently grow meanwhile.

## Central Connection Setup

Every SQLite store must open its connection via **`db::open(path)`** (never `Connection::open()` directly); the JSON-file stores named above do not use it at all. This ensures:

```rust
pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.set_busy_timeout(Duration::from_secs(5))?;  // 5s timeout for lock contention
    conn.pragma_update(None, "journal_mode", "WAL")?; // Write-Ahead Logging
    Ok(conn)
}
```

**Benefits:**

- **WAL mode**: Readers don't block writers; writes are durable immediately; reads are fast.
- **5-second busy timeout**: Prevents "database is locked" errors during concurrent access.
- **Single policy point**: Updating these settings applies app-wide without per-store changes.

## Atomic Transactions

Any **multi-step operation** must be wrapped in a SQLite transaction:

```rust
// Example: atomic import (clear + validate + repopulate)
let tx = conn.transaction()?;
{
    // Pre-validate all incoming data before mutating anything
    for item in &data {
        validate_item(item)?;
    }

    // Clear existing data
    tx.execute("DELETE FROM applications", [])?;

    // Repopulate with new data
    for app in &data {
        tx.execute(
            "INSERT INTO applications (url, title, ...) VALUES (?1, ?2, ...)",
            params![&app.url, &app.title, ...],
        )?;
    }
}
// Commits the entire transaction atomically
tx.commit()?;
```

**Applies to:**

- **Import operations** (`ai_generations::import`, `applications::import`): clear + repopulate.
- **Status writes** (`applications/mod.rs`): set status + append to status_events history in one transaction.
- **Migrations** (schema changes): run migration body + bump `PRAGMA user_version` in one transaction.

On crash or error, the transaction rolls back; the database remains in its previous consistent state.

## DataStore Trait

Every store carried by backup/restore implements the `DataStore` trait (defined in `data_store.rs`):

```rust
pub trait DataStore {
    fn key(&self) -> &'static str;
    fn export(&self) -> serde_json::Value;
    fn import(&self, data: &Value) -> AppResult<usize>; // Returns record count imported
}
```

### Implementations

| Store                    | Location                              | Responsibility                                     |
| ------------------------ | ------------------------------------- | -------------------------------------------------- |
| `DocumentStore`          | `documents/mod.rs`                    | Resumes, embeddings, keyword caches                |
| `ApplicationStore`       | `applications/mod.rs`                 | Applied jobs, status, activity                     |
| `AiGenerationStore`      | `ai_generations/mod.rs`               | Generated cover letters, summaries                 |
| `JobPreferencesStore`    | `job_preferences/mod.rs`              | Saved filters, board preferences                   |
| `ContactProfileStore`    | `contact_profile/mod.rs`              | Saved address, phone, contact info                 |
| `ReferralStore`          | `referrals/mod.rs`                    | Referral tracking                                  |
| `AiConfigStore`          | `ai_config/mod.rs`                    | AI provider config (base_url provenance, ADR-0012) |
| `SpendStore`             | `spend/mod.rs`                        | AI spend records                                   |
| `DedupStore`             | `dedup/mod.rs`                        | Dedup tombstones                                   |
| `DiscoveredCompanyStore` | `discovered/mod.rs`                   | Discovered companies                               |
| `PipelineRunStore`       | `pipeline/runs/mod.rs`                | Résumé pipeline runs + their stage events          |
| `AutopilotStore`         | `autopilot/mod.rs`                    | Autopilot records + run status (JSON file)         |
| `InteractionStore`       | Exported inline by `commands/data.rs` | Generated autopilot interactions (JSON file)       |

Persisted but **not** `DataStore` implementations, so they are outside the backup
bundle: `NotificationStore` (`notifications/mod.rs`), `EmailWatchStore`
(`email_watch/mod.rs`, ADR-0013) and `KvCache` (`pipeline/cache/`). All three are
still wiped by factory reset — see the `Resettable` registry below.

### Backup & Restore

The `commands/data.rs` module orchestrates **full backup/restore** across all stores. Its `ARRAY_SECTIONS` + `OBJECT_SECTIONS` consts are the bundle's section-key list, pinned by tests against each store's own `DataStore::key()` and against the import routing:

```rust
pub async fn data_export(app: AppHandle) -> Value {
    // Exports all DataStore impls + inline sections (autopilot interactions)
    // Returns untyped JSON with BUNDLE_VERSION=1
}

pub async fn data_import(app: AppHandle, bundle: Value) -> Value {
    // Pre-validates all sections (validate_sections) before any mutation
    // Then imports each store atomically (independently)
    // Known limitation: stores are in separate SQLite files, so no
    // cross-file rollback if a later store fails after earlier commits
}
```

Pre-validation prevents invalid data from being written to any store; each store's import is individually atomic within its SQLite file.

### Restore Atomicity

**Each store's import is individually atomic** (pre-validate + transaction within one SQLite file). However, **full cross-file atomicity is not implemented**: the bundle is pre-validated before any mutation begins (preventing invalid data from being written to any store), but if a later store's write fails at the SQLite level (e.g., disk error) after an earlier store has already committed, the earlier store's changes are not rolled back. True cross-file rollback would require a unified database schema — that is a known limitation out of scope for the current design.

### Resettable Registry

The `Resettable` trait (defined in `data_store.rs`) gates access to factory-reset:

```rust
pub trait Resettable {
    fn reset(&self);  // Wipe all data (infallible by design)
}
```

Most `impl Resettable` blocks live in `commands/privacy.rs` (a few sit with their own module, e.g. the extension bridge); registration happens through `manage_resettable` (mostly from `lib.rs::setup`), and `privacy_reset_app` iterates the registry. It is deliberately wider than the `DataStore` list above — a store can be resettable without being backed up (notifications, email watch, the KV cache), and transient state (`PostingsCache`, `JobTracker`, `CredentialStore`) is registered too. Query `commands/privacy.rs` for the canonical membership rather than trusting a copy here; a completeness test pins it.

**When adding a new persisted table to an existing store:** extend that store's `reset()` method to `DELETE FROM` the new table. Add a unit test to verify the table is empty after `reset()`.

See **ADR-009**: Resettable registry for the full design (or query the current registry in `commands/privacy.rs` for the canonical list).

## Performance

- **Caching**: `match_scores` + `posting_vectors` tables cache expensive computations (embeddings, keyword coverage).
- **Composite primary keys**: Encode formula version + input hash, so schema/algorithm changes automatically invalidate stale cached results.
- **Pruning**: `KvCache::prune()` removes old vectors on schedule; `system_set_performance_mode` triggers maintenance.

## Related

- **ADR-022**: Atomic store transactions — full rationale.
- **ADR-006**: Single app-wide session store — the renderer-side owner that outlives a
  mount. **Stale**: its path (`renderer/store/generation-store/`) no longer exists, and its
  "the store owns the streamed text" mechanism is superseded by the ladder above. Cited for
  the decision, not for the mechanism.
- **AGENTS.md rule 16** + `scripts/check-event-subscriptions.mjs`: the renderer half of state ownership, and its enforcement.
- **`docs/knowledge/event-system.md`**: the event channels themselves — registry, codegen, emission, cold-start buffering.
- **PATTERNS.md § 14**: Database transactions & atomicity — code examples.
- **ARCHITECTURE_STATUS.md**: Persistence infrastructure status.
