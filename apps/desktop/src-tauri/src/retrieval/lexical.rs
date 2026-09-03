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

/// How one query's quoted tokens are joined into the `MATCH` expression.
///
/// The tokens themselves are quoted IDENTICALLY either way (see
/// [`sanitize_query`]) — this picks the connective only, never the escaping,
/// so neither mode can be the one that lets an operator through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// FTS5's implicit `AND`: EVERY token must appear in a document for it to
    /// match at all. Right for a search BOX, where each word the user typed
    /// is a filter they added deliberately.
    All,
    /// Explicit `OR`: a document matches on ANY token, and `bm25()` ranks by
    /// how many of them it matched and how rare they are. Right for a
    /// QUESTION — "How do I export my resume as a PDF?" is a sentence, not a
    /// filter list, and under [`QueryMode::All`] its function words ("how",
    /// "my", "as") turn it into a conjunction no help entry satisfies, so the
    /// arm returns zero hits for a perfectly answerable question.
    ///
    /// Those same function words are then the mode's own problem: each one is
    /// an OR branch matching most of the corpus. The caller passes the list to
    /// drop ([`LexicalIndex::search_any`]'s `stopwords`), because which words
    /// carry no signal is a property of the corpus's LANGUAGE, which this
    /// module deliberately knows nothing about.
    Any,
}

/// ASCII tokens shorter than this are dropped in [`QueryMode::Any`] —
/// "I"/"a"/"my" carry no topical signal but each add an OR branch that matches
/// most of the corpus. Not applied to [`QueryMode::All`], where every token
/// NARROWS the result and dropping one would silently widen a user's filter.
///
/// **ASCII only, because character count is not word length outside it.** A
/// CJK content word is routinely ONE character (中文 "書", 日本語 "本"), so a
/// bare char-count filter deleted the only real term in a Chinese or Japanese
/// question and left the query to the all-short fallback below — the same
/// English-only assumption `STOPWORDS` carries in the matching path, and the
/// same failure: a non-Latin question silently retrieves on noise.
///
/// A query made ENTIRELY of such tokens keeps them rather than sanitizing to
/// the empty string, which `search` reads as "nothing to search for" and
/// answers with zero hits — a one-letter question is a bad query, not a query
/// that should silently return nothing.
const ANY_MIN_TOKEN_CHARS: usize = 2;

/// Turn a raw, untrusted search-box string into a `MATCH` expression FTS5
/// cannot fail to parse: every whitespace-separated token becomes a quoted
/// phrase (embedded `"` doubled, FTS5's own escape), joined per `mode`.
/// Without this, an FTS5 query-syntax character typed by a user (`-`, `*`,
/// `:`, an unbalanced `"`, the bare word `NEAR`) either throws a parse error
/// or silently changes the query's meaning (`-golang` is a NOT clause, not
/// the substring "-golang") — quoting every token as a literal phrase removes
/// FTS5's operator grammar entirely, at the cost of no longer supporting
/// exact query-syntax power use nobody asked for here.
///
/// ONE quoting implementation for both modes on purpose: an `OR` variant with
/// its own escaping would be a second place for that hardening to drift out
/// of, including the NUL-byte case `LexicalIndex::search` documents.
fn sanitize_query(query: &str, mode: QueryMode, stopwords: &[&str]) -> String {
    let quote = |tok: &str| format!("\"{}\"", tok.replace('"', "\"\""));
    match mode {
        QueryMode::All => query
            .split_whitespace()
            .map(quote)
            .collect::<Vec<_>>()
            .join(" "),
        QueryMode::Any => {
            let mut tokens: Vec<String> = query
                .split_whitespace()
                // ASCII-gated on purpose — see [`ANY_MIN_TOKEN_CHARS`].
                .filter(|tok| !(tok.is_ascii() && tok.chars().count() < ANY_MIN_TOKEN_CHARS))
                .filter(|tok| !is_stopword(tok, stopwords))
                .map(&quote)
                .collect();
            // Covers BOTH filters above: a question made entirely of function
            // words ("What is it?") would otherwise sanitize to the empty
            // string, which `search_in` answers with zero hits — reported as
            // `Ran`, and on a default install that is the only arm there is.
            // Falling back to the unfiltered tokens ranks it badly rather
            // than silently answering nothing.
            //
            // This is only the TOKEN-list half of that fallback. A question
            // whose surviving tokens are all absent from the corpus ("Where
            // is my stuff?" — `stuff` is in no entry) sanitizes to a
            // NON-empty expression that still matches zero rows, which this
            // check cannot see; `search_in` catches that one on the RESULT
            // set. Two halves of one rule, deliberately in the two places
            // that can each observe their own half.
            if tokens.is_empty() {
                tokens = query.split_whitespace().map(quote).collect();
            }
            tokens.join(" OR ")
        }
    }
}

/// Whether a RAW query token is on the drop list.
///
/// Folded before the comparison, because tokens reach here exactly as typed:
/// `"How"` and `"work?"` are what `split_whitespace` yields, so the obvious
/// `stopwords.contains(&tok)` silently matches neither — a filter that no-ops
/// on precisely the tokens it was added for, with every lowercase unpunctuated
/// test still passing. Lowercased, and trimmed of leading/trailing
/// non-alphanumerics (FTS5's own tokenizer does the same thing later, so this
/// folding does not change which documents a surviving token matches).
fn is_stopword(token: &str, stopwords: &[&str]) -> bool {
    if stopwords.is_empty() {
        return false;
    }
    let folded = token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    !folded.is_empty() && stopwords.contains(&folded.as_str())
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
    /// `Ok(Vec::new())` on an empty/whitespace-only query (nothing to search
    /// for) or a clean zero-match result — both are a genuine "no hits", not
    /// a failure. `Err` on anything FTS5 itself rejects.
    ///
    /// **Empirically verified, not theorised** (a security-review claim that
    /// a bare punctuation query like `-` or `!!!` breaks this did NOT
    /// reproduce: `sanitize_query`'s quoting defeats FTS5's own operator
    /// grammar for every character tested — `-`, `*`, `:`, `NEAR`, `AND`,
    /// unbalanced `"`, empty phrases). What DOES reproduce: a query
    /// containing an embedded NUL byte reaches FTS5's query-expression
    /// parser as a string it reads as truncated, and fails with
    /// `"unterminated string"` on the FIRST row fetch — a genuine `rusqlite`
    /// error this method used to swallow into a silent "zero hits" via
    /// `filter_map(Result::ok)`, matching what a caller could not tell apart
    /// from an honest empty result. Returning `Result` (rather than
    /// swallowing here) lets the CALLER decide how to report a real failure
    /// — `commands::hybrid_search` maps `Err` to `ArmStatus::Unavailable` —
    /// which is an L3 reporting decision, not this L1 module's to make.
    pub fn search(&self, query: &str, limit: usize) -> AppResult<Vec<String>> {
        // No drop list: under [`QueryMode::All`] every token NARROWS the
        // result, so dropping one would silently widen a user's own filter.
        self.search_in(query, QueryMode::All, &[], limit)
    }

    /// [`Self::search`]'s sibling for a QUESTION rather than a search box:
    /// same index, same quoting, [`QueryMode::Any`] instead of the implicit
    /// AND, so `bm25()` ranks by how many of the question's terms a document
    /// matched instead of requiring all of them.
    ///
    /// Separate method rather than a mode parameter on [`Self::search`]: the
    /// postings search box must stay on AND (every word the user typed is a
    /// filter), so the two callers state which contract they want at the call
    /// site instead of passing a flag that could be flipped for both at once.
    ///
    /// `stopwords` are the question's own function words to drop before the
    /// OR-join, lowercased and already folded by the caller's table. Passed IN
    /// rather than looked up here: which words carry no signal depends on the
    /// corpus's language, and this module indexes text without knowing (or
    /// wanting to know) what language it is in. Pass `&[]` to drop nothing.
    ///
    /// **Dropping a token can never turn hits into a miss.** A drop list only
    /// costs recall, so both ways it can zero out a result are undone rather
    /// than reported as an honest "no answer": if it empties the TOKEN list
    /// the unfiltered tokens are used instead ([`sanitize_query`]), and if
    /// the surviving tokens match no ROW the unfiltered expression is run
    /// once more ([`Self::search_in`]). A non-empty result is never re-ranked
    /// — that would undo the filtering itself.
    pub fn search_any(
        &self,
        query: &str,
        limit: usize,
        stopwords: &[&str],
    ) -> AppResult<Vec<String>> {
        self.search_in(query, QueryMode::Any, stopwords, limit)
    }

    fn search_in(
        &self,
        query: &str,
        mode: QueryMode,
        stopwords: &[&str],
        limit: usize,
    ) -> AppResult<Vec<String>> {
        let sanitized = sanitize_query(query, mode, stopwords);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }
        let hits = self.match_ids(&sanitized, limit)?;
        // The RESULT-set half of the drop list's fallback (see
        // `sanitize_query`'s own note). A dropped token can only ever cost
        // recall, so a filtered query that matched NOTHING has nothing to
        // lose by being asked again unfiltered: "Where is my stuff?" keeps
        // only `stuff`, which is in no help entry, and the arm reported
        // `Ran` with zero hits for a question the unfiltered expression
        // answers at rank 2 — on a default install (`semantic_scoring` off)
        // that is the only arm there is.
        //
        // Narrow on purpose: ONLY when a drop list was actually applied
        // (`Any` + non-empty `stopwords`), only when the two expressions
        // really differ (nothing was dropped ⇒ nothing to retry), and only
        // on a genuinely EMPTY result — never a re-rank of a non-empty one,
        // which would undo the filtering the list exists for. At most one
        // extra query per search, against an in-memory index.
        if !hits.is_empty() || mode != QueryMode::Any || stopwords.is_empty() {
            return Ok(hits);
        }
        let unfiltered = sanitize_query(query, mode, &[]);
        if unfiltered.is_empty() || unfiltered == sanitized {
            return Ok(hits);
        }
        self.match_ids(&unfiltered, limit)
    }

    /// Run one already-sanitized `MATCH` expression and map its rows back to
    /// caller ids. Split out of [`Self::search_in`] so the fallback above
    /// re-runs the query without a second copy of the SQL, the weights or
    /// the row-id mapping.
    fn match_ids(&self, expression: &str, limit: usize) -> AppResult<Vec<String>> {
        let (w_title, w_company, w_location, w_description) = BM25_WEIGHTS;
        // `ORDER BY bm25(...)` ascending (the default) is correct as written:
        // FTS5's bm25() returns SMALLER values for a BETTER match.
        let sql = "SELECT rowid FROM postings WHERE postings MATCH ?1 \
                   ORDER BY bm25(postings, ?2, ?3, ?4, ?5) LIMIT ?6";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(
            params![
                expression,
                w_title,
                w_company,
                w_location,
                w_description,
                limit as i64
            ],
            |row| row.get::<_, i64>(0),
        )?;
        let mut out = Vec::new();
        for row in rows {
            // Propagate on the FIRST error rather than skipping the row: an
            // error here is the query EXPRESSION failing to evaluate (see
            // the NUL-byte case above), not one row's data being bad, so the
            // whole result set is untrustworthy once it happens.
            let rowid = row?;
            if let Some(id) = self.row_ids.get((rowid - 1) as usize) {
                out.push(id.clone());
            }
        }
        Ok(out)
    }
}
