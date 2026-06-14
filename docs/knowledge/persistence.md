# Persistence Layer — SQLite, Transactions, Atomicity

Canonical sources:

- `apps/tauri/src-tauri/src/db.rs` — centralized connection + WAL setup
- `apps/tauri/src-tauri/src/data_store.rs` — trait definition
- Individual stores: `ai_generations/mod.rs`, `applications/mod.rs`, `documents/mod.rs`, etc.

## Central Connection Setup

Every store must open SQLite connections via **`db::open(path)`** (never `Connection::open()` directly). This ensures:

```rust
pub fn open(path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;
    conn.set_busy_timeout(Duration::from_secs(5))?;  // 5s timeout for lock contention
    conn.execute("PRAGMA journal_mode = WAL")?;       // Write-Ahead Logging
    conn
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
- **Status writes** (`jobs/update_status`): update row + insert history event in one transaction.
- **Migrations** (schema changes): run migration body + bump `PRAGMA user_version` in one transaction.

On crash or error, the transaction rolls back; the database remains in its previous consistent state.

## DataStore Trait

Every persistent store implements the `DataStore` trait (defined in `data_store.rs`):

```rust
pub trait DataStore: Send + Sync {
    fn export(&self) -> Result<serde_json::Value, Error>;
    fn import(&self, data: serde_json::Value) -> Result<(), Error>;
}
```

### Implementations

| Store                 | Location                 | Responsibility                      |
| --------------------- | ------------------------ | ----------------------------------- |
| `DocumentStore`       | `documents/mod.rs`       | Resumes, embeddings, keyword caches |
| `ApplicationsStore`   | `applications/mod.rs`    | Applied jobs, status, activity      |
| `AiGenerationsStore`  | `ai_generations/mod.rs`  | Generated cover letters, summaries  |
| `JobPreferencesStore` | `job_preferences/mod.rs` | Saved filters, board preferences    |
| `JobTrackerStore`     | `jobs/mod.rs`            | Scrape queue, run logs              |
| `ReferralsStore`      | `referrals/mod.rs`       | Referral tracking                   |
| `ContactProfileStore` | `contact_profile/mod.rs` | Saved address, phone, contact info  |
| `CredentialsStore`    | `credentials/mod.rs`     | OS keychain binding                 |

### Backup & Restore

The `commands/data.rs` module orchestrates **full backup/restore** across all stores:

```rust
pub async fn data_export(state: State<'_, AppState>) -> AppResult<ExportBundle> {
    let bundle = ExportBundle {
        version: 1,
        timestamp: now(),
        documents: state.documents.export()?,
        applications: state.applications.export()?,
        // ... all stores
    };
    Ok(bundle)
}

pub async fn data_import(bundle: ExportBundle, state: State<'_, AppState>) -> AppResult<()> {
    // Pre-validate all sections first (before touching any store)
    // Then import each store atomically (independently)
    state.documents.import(bundle.documents)?;
    state.applications.import(bundle.applications)?;
    // ... each import is atomic in isolation
    // Known limitation: stores are in separate SQLite files, so no
    // cross-file rollback if a later store fails after earlier commits
}
```

### Restore Atomicity

**Each store's import is individually atomic** (pre-validate + transaction within one SQLite file). However, **full cross-file atomicity is not implemented**: the bundle is pre-validated before any mutation begins (preventing invalid data from being written to any store), but if a later store's write fails at the SQLite level (e.g., disk error) after an earlier store has already committed, the earlier store's changes are not rolled back. True cross-file rollback would require a unified database schema — that is a known limitation out of scope for the current design.

### Resettable Registry

The `Resettable` trait (defined in `data_store.rs`) gate access to factory-reset. Each store implements:

```rust
pub trait Resettable {
    fn clear_all(&mut self) -> AppResult<()>;  // Wipe all data
}
```

Implemented by: `DocumentStore`, `ApplicationsStore`, `AiGenerationsStore`, `JobPreferencesStore`, `JobTrackerStore`, `ReferralsStore`, `ContactProfileStore`.

**When adding a new persisted table to an existing store:** extend that store's `clear_all()` method to `DELETE FROM` the new table. Add a unit test to verify the table is empty after `clear_all()`.

See **ADR-009**: Resettable registry for the full design.

## Performance

- **Caching**: `match_scores` + `posting_vectors` tables cache expensive computations (embeddings, keyword coverage).
- **Composite primary keys**: Encode formula version + input hash, so schema/algorithm changes automatically invalidate stale cached results.
- **Pruning**: `KvCache::prune()` removes old vectors on schedule; `system_set_performance_mode` triggers maintenance.

## Related

- **ADR-022**: Atomic store transactions — full rationale.
- **PATTERNS.md § 14**: Database transactions & atomicity — code examples.
- **ARCHITECTURE_STATUS.md**: Persistence infrastructure status.
