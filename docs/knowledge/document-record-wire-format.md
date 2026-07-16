# DocumentRecord wire/storage vs app-model divergence

This divergence is **intentional layering** — do not "fix" it.

## The three representations

| Layer              | Format                                            | Owner                                                                                                  |
| ------------------ | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| SQLite columns     | `id`, `created_at`, `is_default`, `keywords_json` | `documents/mod.rs` (accessed positionally; serde-irrelevant)                                           |
| Backup-bundle JSON | `_id`, `createdAt`, `isDefault`, `keywordsJson`   | `documents/mod.rs` `DocumentRecord` serde renames                                                      |
| App model (TS)     | `id`, `importedAt`                                | `DocumentRecord` in `packages/shared`; bridged by `RawDoc`/`normalise` in `renderer/lib/doc-record.ts` |

## Serde renames are the on-disk backup format

`DocumentRecord` in `apps/desktop/src-tauri/src/documents/mod.rs` carries four field-level serde renames:

- `id` → `"_id"`
- `created_at` → `"createdAt"`
- `is_default` → `"isDefault"`
- `keywords_json` → `"keywordsJson"`

These are the **persisted format of every `ajh-backup-*.json` bundle** written by `DataStore::export` / `DataStore::import` (orchestrated in `commands/data.rs`). Every backup ever written by a user uses these keys.

**Changing them without a guarded migration = silent data-loss on restore.** Never rename silently; only via a versioned migration that reads the old key and writes the new one.

## Renderer bridge

The renderer receives raw IPC JSON with `_id`/`createdAt` and bridges to the app model via:

- `RawDoc` — wire type with `_id`, `createdAt`, and backend-only `name`/`text` fields
- `normalise(raw: RawDoc): DocumentRecord` — maps `_id → id`, `createdAt → importedAt`; derives `source` from file extension when absent
- `isRawDoc` — type guard

All live in canonical module `apps/desktop/src/renderer/lib/doc-record.ts` (single source; import from here, not from useResumeInput).

## Owning symbols

- Rust struct + serde renames: `documents/mod.rs` `DocumentRecord`
- Backup orchestration: `commands/data.rs` `data_export` / `data_import`
- DataStore trait: `data_store.rs` `pub trait DataStore`
- DataStore impl for DocumentStore: `documents/mod.rs`
- TS bridge: `renderer/lib/doc-record.ts` `RawDoc` / `normalise` / `isRawDoc`
