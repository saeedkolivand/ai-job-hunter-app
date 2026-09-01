//! Lexical (keyword) ranking via SQLite's built-in FTS5 module, over an
//! EPHEMERAL, `Connection::open_in_memory()` index rebuilt fresh per search.
//!
//! No new dependency: `rusqlite` (`bundled`) already compiles FTS5 into this
//! binary (`libsqlite3-sys`'s build script passes `-DSQLITE_ENABLE_FTS5`
//! unconditionally), and this module speaks to it through plain SQL only —
//! `rusqlite`'s `vtab` feature (for AUTHORING a virtual table in Rust) is
//! neither needed nor enabled.
//!
//! No persisted posting text: `postings::PostingsCache` is in-memory and
//! ephemeral by design (see its module doc), so there is nothing to persist
//! an index OVER — this index is rebuilt from whatever slice of the live
//! cache the caller passes to [`LexicalIndex::build`], and is dropped with
//! the search that built it.

use rusqlite::{params, Connection};

use crate::error::AppResult;

/// One document to index. `id` is the caller's own identity for the row —
/// deliberately NOT an FTS5 column (see [`LexicalIndex::build`]): SQLite's
/// `rowid`, not a text column, is what carries it, so it can never
/// participate in a text match and never needs a `bm25()` weight of its own.
pub struct LexicalDoc<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub company: &'a str,
    pub location: &'a str,
    pub description: &'a str,
}

/// Column weights fed to `bm25()`, in column-declaration order (title,
/// company, location, description).
///
/// **Chosen, not measured — stated plainly rather than hidden.** There is no
/// click-through data for this corpus to derive weights from, so these are a
/// reasoned default: a title hit is the single strongest topical signal a job
/// search has (searching "react developer" should surface a posting TITLED
/// that over one that mentions React once in an unrelated paragraph); a
/// company hit answers a distinct, still-strong intent ("show me Acme
/// postings"); location is usually filtered separately by the UI already, so
/// it gets a REDUCED weight (0.5 — an active discount below the 1.0
/// baseline, not merely "no extra boost") to keep an incidental location
/// mention from outweighing a real title/company match; description carries
/// most of the corpus's actual text and keeps the baseline weight of 1.0.
/// Revisit with real usage data before treating these as tuned.
pub const BM25_WEIGHTS: (f64, f64, f64, f64) = (3.0, 2.0, 0.5, 1.0);

/// Turn a raw, untrusted search-box string into a `MATCH` expression FTS5
/// cannot fail to parse: every whitespace-separated token becomes a quoted
/// phrase (embedded `"` doubled, FTS5's own escape), joined with FTS5's
/// implicit `AND`. Without this, an FTS5 query-syntax character typed by a
/// user (`-`, `*`, `:`, an unbalanced `"`, the bare word `NEAR`) either
/// throws a parse error or silently changes the query's meaning (`-golang`
/// is a NOT clause, not the substring "-golang") — quoting every token as a
/// literal phrase removes FTS5's operator grammar entirely, at the cost of
/// no longer supporting exact query-syntax power use nobody asked for here.
fn sanitize_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|tok| format!("\"{}\"", tok.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// An in-memory FTS5 index, built fresh per search (see module doc).
pub struct LexicalIndex {
    conn: Connection,
    /// `row_ids[i]` is the caller-supplied id for FTS5 `rowid` `i + 1` — see
    /// [`Self::build`] for why the id lives here instead of as a table column.
    row_ids: Vec<String>,
}

impl LexicalIndex {
    /// Build a fresh in-memory FTS5 table from `docs`, one INSERT
    /// transaction. The FTS5 table declares exactly the four TEXT columns
    /// [`BM25_WEIGHTS`] has a weight for (title, company, location,
    /// description) — `id` is inserted as an explicit `rowid` instead of a
    /// fifth column, which sidesteps entirely the question of whether an
    /// `UNINDEXED` column still occupies a `bm25()` weight slot: with `id`
    /// never a column at all, the weight list and the column list are
    /// trivially the same length.
    pub fn build(docs: &[LexicalDoc<'_>]) -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE postings USING fts5(title, company, location, description);",
        )?;
        let mut row_ids = Vec::with_capacity(docs.len());
        {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO postings (rowid, title, company, location, description) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )?;
                for (i, doc) in docs.iter().enumerate() {
                    let rowid = (i + 1) as i64;
                    stmt.execute(params![
                        rowid,
                        doc.title,
                        doc.company,
                        doc.location,
                        doc.description
                    ])?;
                    row_ids.push(doc.id.to_string());
                }
            }
            tx.commit()?;
        }
        Ok(Self { conn, row_ids })
    }

    /// Best-first matching ids for `query`, at most `limit`.
    ///
    /// Empty on anything that isn't a clean hit: an empty/whitespace-only
    /// query, or a `prepare`/`query_map` failure — FTS5 itself should never
    /// reject [`sanitize_query`]'s output, but this degrades to "no lexical
    /// hits" rather than failing the whole search if it somehow does, matching
    /// this crate's degrade-never-fail-a-command posture for scoring paths.
    pub fn search(&self, query: &str, limit: usize) -> Vec<String> {
        let sanitized = sanitize_query(query);
        if sanitized.is_empty() {
            return Vec::new();
        }
        let (w_title, w_company, w_location, w_description) = BM25_WEIGHTS;
        // `ORDER BY bm25(...)` ascending (the default) is correct as written:
        // FTS5's bm25() returns SMALLER values for a BETTER match.
        let sql = "SELECT rowid FROM postings WHERE postings MATCH ?1 \
                   ORDER BY bm25(postings, ?2, ?3, ?4, ?5) LIMIT ?6";
        let Ok(mut stmt) = self.conn.prepare(sql) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(
            params![
                sanitized,
                w_title,
                w_company,
                w_location,
                w_description,
                limit as i64
            ],
            |row| row.get::<_, i64>(0),
        ) else {
            return Vec::new();
        };
        rows.filter_map(Result::ok)
            .filter_map(|rowid| self.row_ids.get((rowid - 1) as usize).cloned())
            .collect()
    }
}
