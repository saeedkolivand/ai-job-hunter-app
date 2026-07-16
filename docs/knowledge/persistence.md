# Persistence Layer — SQLite, Transactions, Atomicity

Last updated: 2026-07-16

Canonical sources:

- `apps/desktop/src-tauri/src/db.rs` — centralized connection + WAL setup
- `apps/desktop/src-tauri/src/data_store.rs` — trait definition
- Individual stores: `ai_generations/mod.rs`, `applications/mod.rs`, `documents/mod.rs`, etc.

## Central Connection Setup

Every store must open SQLite connections via **`db::open(path)`** (never `Connection::open()` directly). This ensures:

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

Every persistent store implements the `DataStore` trait (defined in `data_store.rs:16-26`):

```rust
pub trait DataStore {
    fn key(&self) -> &'static str;
    fn export(&self) -> serde_json::Value;
    fn import(&self, data: &Value) -> AppResult<usize>; // Returns record count imported
}
```

### Implementations

| Store                 | Location                              | Responsibility                                     |
| --------------------- | ------------------------------------- | -------------------------------------------------- |
| `DocumentStore`       | `documents/mod.rs:1034`               | Resumes, embeddings, keyword caches                |
| `ApplicationStore`    | `applications/mod.rs:1351`            | Applied jobs, status, activity                     |
| `AiGenerationStore`   | `ai_generations/mod.rs:499`           | Generated cover letters, summaries                 |
| `JobPreferencesStore` | `job_preferences/mod.rs:161`          | Saved filters, board preferences                   |
| `ContactProfileStore` | `contact_profile/mod.rs:700`          | Saved address, phone, contact info                 |
| `ReferralStore`       | `referrals/mod.rs:210`                | Referral tracking                                  |
| `AiConfigStore`       | `ai_config/mod.rs:386`                | AI provider config (base_url provenance, ADR-0012) |
| `SpendStore`          | `spend/mod.rs:253`                    | AI spend records                                   |
| `NotificationStore`   | `notifications/mod.rs`                | Persisted notifications + actions                  |
| `EmailWatchStore`     | `email_watch/mod.rs:89`               | Email account + watch state (ADR-0013)             |
| `KvCache`             | `lib/kv_cache.rs`                     | Generic key-value cache (transient)                |
| `InteractionStore`    | Exported inline by `commands/data.rs` | Generated autopilot interactions                   |

### Backup & Restore

The `commands/data.rs` module orchestrates **full backup/restore** across all stores (lines 137, 170):

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

The `Resettable` trait (defined in `data_store.rs:38-41`) gates access to factory-reset:

```rust
pub trait Resettable {
    fn reset(&self);  // Wipe all data (infallible by design)
}
```

Registered and called from `commands/privacy.rs:33-110`. Implemented by: PostingsCache, InteractionStore, JobTracker, CredentialStore, AutopilotStore, DocumentStore, AiGenerationStore, ApplicationStore, JobPreferencesStore, ContactProfileStore, AiConfigStore, ReferralStore, NotificationStore, KvCache, SpendStore, EmailWatchStore.

**When adding a new persisted table to an existing store:** extend that store's `reset()` method to `DELETE FROM` the new table. Add a unit test to verify the table is empty after `reset()`.

See **ADR-009**: Resettable registry for the full design (or query the current registry in `commands/privacy.rs` for the canonical list).

## Performance

- **Caching**: `match_scores` + `posting_vectors` tables cache expensive computations (embeddings, keyword coverage).
- **Composite primary keys**: Encode formula version + input hash, so schema/algorithm changes automatically invalidate stale cached results.
- **Pruning**: `KvCache::prune()` removes old vectors on schedule; `system_set_performance_mode` triggers maintenance.

## Related

- **ADR-022**: Atomic store transactions — full rationale.
- **PATTERNS.md § 14**: Database transactions & atomicity — code examples.
- **ARCHITECTURE_STATUS.md**: Persistence infrastructure status.
