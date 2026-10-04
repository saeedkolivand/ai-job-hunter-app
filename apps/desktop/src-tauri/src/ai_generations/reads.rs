//! The read side of the `ai_generations` store: the full list, the per-posting
//! finders, and the url sets the cascade and the applied badge derive from.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Persistence still lives entirely inside
//! [`super::AiGenerationStore`], on the SAME connection.

use rusqlite::params;

use super::rows::{row_to_record, SELECT_COLS};
use super::{AiGenerationRecord, AiGenerationStore};

impl AiGenerationStore {
    pub fn list(&self) -> Vec<AiGenerationRecord> {
        let conn = self.conn.lock();
        conn.prepare(&format!("{SELECT_COLS} ORDER BY created_at DESC"))
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], row_to_record)
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    }

    /// Most-recent record linked to `job_url`, if any — the row a per-job save
    /// merges into so each job keeps one application aggregate.
    pub(super) fn find_by_job_url(&self, job_url: &str) -> Option<AiGenerationRecord> {
        if job_url.is_empty() {
            return None;
        }
        let conn = self.conn.lock();
        conn.prepare(&format!(
            "{SELECT_COLS} WHERE job_url = ?1 ORDER BY created_at DESC LIMIT 1"
        ))
        .ok()
        .and_then(|mut stmt| stmt.query_row(params![job_url], row_to_record).ok())
    }

    /// The aggregate row for one posting, matched the way
    /// [`save_application`](Self::save_application) matches it: on the
    /// NORMALIZED url first, falling back to the raw one for a row written
    /// before normalization. Two lookups, one rule — a reader that matched only
    /// the raw url would miss the row every writer since normalization has been
    /// updating, which on a query-id board (Indeed) is most of them.
    ///
    /// The staged pipeline's read path: a run's document and its quality report
    /// live HERE, not in the run store, so `resumePipeline.get` joins the two.
    pub fn find_for_job(&self, job_url: &str) -> Option<AiGenerationRecord> {
        let normalized = crate::applications::normalize_job_url(job_url);
        self.find_by_job_url(&normalized).or_else(|| {
            (normalized != job_url)
                .then(|| self.find_by_job_url(job_url))
                .flatten()
        })
    }

    /// Distinct non-empty `job_url`s that have at least one saved generation —
    /// the set used to derive a found job's `applied` flag.
    pub fn applied_job_urls(&self) -> std::collections::HashSet<String> {
        let conn = self.conn.lock();
        conn.prepare("SELECT DISTINCT job_url FROM ai_generations WHERE job_url != ''")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| row.get::<_, String>(0))
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    }

    /// The distinct, non-empty posting urls a set of generations belongs to.
    ///
    /// **Read BEFORE the delete, because the delete is what makes it
    /// unanswerable.** Deleting a generated résumé has to cascade into
    /// `pipeline_runs` — a max-depth run persists the full strategy (the whole
    /// employment history) and the full evidence map (verbatim résumé quotes)
    /// in its event trail, and the aggregate row is the only thing that ever
    /// pointed at them. The join is by `job_url` because the aggregate is one
    /// row per posting, which makes the mapping exact rather than heuristic.
    ///
    /// An empty url is skipped: an unlinked generation has no posting, and
    /// `PipelineRunStore::delete_for_job` would refuse it anyway (matching every
    /// empty-url run would delete other postings' history).
    ///
    /// **A failed READ degrades to "no urls", which is the accepted orphan
    /// case.** A transient `SQLITE_BUSY` here is indistinguishable from a
    /// generation that had no posting, so the cascade silently skips and the
    /// trail outlives its owner — the same window the caller's own doc records
    /// for a crash between the delete and the purge, reached a different way.
    /// Returning an error instead would mean failing the DELETE the user asked
    /// for because a bookkeeping read lost a race, which is the worse trade.
    pub fn job_urls_for(&self, ids: &[String]) -> Vec<String> {
        if ids.is_empty() {
            return Vec::new();
        }
        let conn = self.conn.lock();
        let mut urls: Vec<String> = Vec::new();
        // CHUNKED: the selection is the user's, and an unbounded `IN (?, …)`
        // fails to prepare past SQLite's host-parameter limit — which for THIS
        // read means "no urls", i.e. a cascade that silently does not happen.
        for chunk in ids.chunks(crate::db::MAX_SQL_PARAMS) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT DISTINCT job_url FROM ai_generations \
                 WHERE id IN ({placeholders}) AND job_url != ''"
            );
            let Ok(mut stmt) = conn.prepare(&sql) else {
                continue;
            };
            let rows = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), |row| {
                row.get::<_, String>(0)
            });
            if let Ok(rows) = rows {
                urls.extend(rows.filter_map(Result::ok));
            }
        }
        urls.sort();
        urls.dedup();
        urls
    }
}
