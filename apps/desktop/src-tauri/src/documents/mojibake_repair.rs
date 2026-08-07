//! `repair_pre_pdf_text_string_mojibake` migration body, split out of
//! `documents/mod.rs` to keep that file under the R8 module-size cap.
//!
//! One-shot repair for rows written before PR #955 fixed `pdf_text_string`
//! (see `extraction::pdf::pdf_text_string` / `repair_utf16_mojibake` for the
//! corruption shape). A pre-fix PDF import decoded a link anchor with
//! `String::from_utf8_lossy` and persisted the mojibake verbatim into
//! `documents.text`.
//!
//! Only rows with an embedded NUL are candidates — SQLite's `length()` stops
//! at the first NUL, so `instr(cast(text as blob), x'00')` is used instead
//! (`char(0)` cannot produce a NUL to `LIKE` against). Repair runs in RUST —
//! `replace()` on NUL-bearing TEXT is not dependable in SQLite.
//!
//! Snapshots every affected row FIRST, same transaction, before rewriting —
//! a safety net for an irreversible in-place edit of what may be the only
//! remaining copy of a résumé.
//!
//! Also DELETEs the row's `vectors` entry (not just clearing `keywords_json`
//! and `indexed`): `stale_documents` (`commands/ai.rs`) — the sole consumer
//! that decides what the auto-indexer re-embeds — asks only whether
//! `get_vector` returns a hit in the ACTIVE embedding space; it never reads
//! `indexed`. A prior version of this migration only flipped `indexed = 0`,
//! which was a complete no-op for re-embedding: the pre-repair vector still
//! matched the active space, so the document was never considered stale and
//! the embedding stayed permanently derived from the corrupt text. Deleting
//! the vector is what actually makes `get_vector` miss, so `stale_documents`
//! picks the document up on the next auto-index run. `indexed = 0` is kept
//! as an honest reflection of "no vector exists", not the mechanism.
//!
//! A per-row UPDATE failure PROPAGATES (`?`) instead of being
//! logged-and-skipped: on SQLite's "fatal" error classes
//! (SQLITE_FULL/IOERR/NOMEM/BUSY/INTERRUPT — sqlite.org/lang_transaction.html),
//! SQLite may silently roll back the WHOLE enclosing transaction to
//! autocommit. Swallowing the error would let the unconditional `PRAGMA
//! user_version = N` that follows (`db::run_migrations`) commit on its own —
//! durably marking this migration "done" though the row was never repaired,
//! with no future retry. Measured on a real WAL database: a `max_page_count`
//! clamp low enough to fail the UPDATE still let a swallowed error advance
//! `user_version`. Propagating returns `Err` before that line is ever
//! reached, so the whole migration rolls back for a clean retry next launch
//! — not fatal to startup either way, `lib.rs`'s setup hook treats a failed
//! `DocumentStore::open()` as non-fatal.

use rusqlite::{params, Connection};

pub(super) fn up(conn: &Connection) -> rusqlite::Result<()> {
    // Safety net: snapshot the pre-repair value of every row this migration
    // is about to touch, in this same transaction.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS documents_pre_mojibake_repair AS
         SELECT id, text FROM documents
         WHERE instr(cast(text as blob), x'00') > 0;",
    )?;

    let rows: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, text FROM documents WHERE instr(cast(text as blob), x'00') > 0")?;
        let mapped = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in mapped {
            match r {
                Ok(row) => out.push(row),
                // Never silently drop a row this migration was meant to
                // reach (e.g. a `text` value that ends up with BLOB storage
                // class — SQLite is dynamically typed).
                Err(e) => tracing::warn!(
                    "[db] repair_pre_pdf_text_string_mojibake: skipping a row that failed to map: {e}"
                ),
            }
        }
        out
    };
    for (id, text) in rows {
        let repaired = crate::extraction::pdf::repair_utf16_mojibake(&text);
        // `keywords_json = NULL` falls back to live extraction (see
        // `cache_document_keywords` in `mod.rs`). `indexed = 0` is cosmetic
        // bookkeeping, kept honest by the DELETE below — see the module doc.
        conn.execute(
            "UPDATE documents SET text = ?1, keywords_json = NULL, indexed = 0 WHERE id = ?2",
            params![repaired.as_ref(), id],
        )?;
        // The mechanism: without this, the pre-repair vector still matches
        // the active embedding space and `stale_documents` never re-embeds.
        conn.execute("DELETE FROM vectors WHERE doc_id = ?1", params![id])?;
    }
    Ok(())
}
