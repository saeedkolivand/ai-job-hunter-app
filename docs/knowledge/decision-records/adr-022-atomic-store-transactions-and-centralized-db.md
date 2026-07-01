# ADR-022: Atomic store transactions + centralized db::open

Last updated: 2026-06-14

**Status:** Accepted

## Context

SQLite operations were scattered:

- Stores opened connections independently without pooling or timeout configuration.
- Multi-step operations (clear + repopulate, status write + history event) were split across separate `.execute()` calls, allowing corruption if a process crash or Ctrl+C occurred mid-operation.
- No consistent WAL (write-ahead logging) or busy-timeout policy across the codebase.
- Import operations could partially succeed, leaving the database in an inconsistent state.

## Decision

**Centralize all SQLite connection setup via `db::open()`** (defined in `apps/desktop/src-tauri/src/db.rs`):

```rust
pub fn open(path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;
    conn.set_busy_timeout(Duration::from_secs(5))?;
    conn.execute("PRAGMA journal_mode = WAL")?;
    conn
}
```

Every store (`DocumentStore`, `ApplicationsStore`, `AiGenerationsStore`, etc.) calls `db::open()` instead of `Connection::open()` directly. This ensures:

- **5-second busy timeout** prevents "database is locked" on concurrent writes.
- **WAL mode** ensures durability and fast reads during writes.
- **Single policy point:** updating timeout or pragmas applies everywhere.

**Wrap multi-step operations in SQLite transactions:**

- Import (clear + repopulate): `BEGIN TRANSACTION` → validate all data → clear table → insert → `COMMIT`.
- Status writes: `BEGIN` → update row → insert history event → `COMMIT`.
- Migrations: `BEGIN` → run migration body → bump `PRAGMA user_version` → `COMMIT`.

**Map SQLite-boundary errors to `AppError::Storage`** instead of stringly-typed `Err(String)`, enabling proper error serialization and diagnostics.

## Consequences

- **Crash-safe:** operations either fully succeed or fully roll back; no partial writes.
- **Concurrent-write safe:** the 5-second busy timeout prevents lock contention on the typical workflow (background scraper + foreground UI).
- **WAL mode benefits:** read transactions don't block writes; the app remains responsive during large imports.
- **Testability:** transactions are easy to verify (test helper that rolls back after each assertion).
- **Cost:** WAL uses ~2× disk space for the log file (temporary during writes, then checkpointed). Acceptable for local-first app.

## Related

- `docs/knowledge/persistence.md` — thin pointer to `db::open()` and transaction patterns.
- `commands/data.rs` — backup/restore uses transactions for atomic full-snapshot import.
- `ai_generations/mod.rs`, `applications/mod.rs` — store implementations route through `db::open()`.
- Lesson: "Route stores through one `db::open` for WAL+busy_timeout and wrap clear+repopulate in a txn."
