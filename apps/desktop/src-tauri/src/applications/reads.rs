//! The read side of the `applications` store: the lock-taking lookups the rest of
//! the app calls (`list`/`get`, the per-url and per-identity finders, and the
//! applied-url sets).
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Persistence still lives entirely inside
//! [`super::ApplicationStore`], on the SAME connection.

use super::rows::{row_to_application, SELECT_COLS};
use super::{decode_unreserved, Application, ApplicationStore};

impl ApplicationStore {
    pub fn list(&self) -> Vec<Application> {
        let conn = self.conn.lock();
        conn.prepare(&format!("{SELECT_COLS} ORDER BY updated_at DESC"))
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], row_to_application)
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    }

    pub fn get(&self, id: &str) -> Option<Application> {
        Self::row_by_id_conn(&self.conn.lock(), id).ok().flatten()
    }

    /// Most-recent Application for a normalized url, if any — the row a per-job
    /// upsert merges into so one job keeps a single aggregate. `pub(crate)` so
    /// `extension_bridge`'s `applied.check` handler can run the same read-only
    /// lookup (it never fetches or writes — see `resolve_applied_check`).
    pub(crate) fn find_by_job_url(&self, normalized: &str) -> Option<Application> {
        Self::row_by_job_url_conn(&self.conn.lock(), normalized)
            .ok()
            .flatten()
    }

    /// Most-recent Application whose stored `job_url` resolves to the same
    /// [`crate::scraping::scrape_url::job_identity`] as `identity` (issue
    /// #1214) — the identity-aware sibling of [`Self::find_by_job_url`], for a
    /// lookup whose exact normalized-key match misses but whose url still names
    /// the same posting (a regional LinkedIn host vs `www.linkedin.com`, a
    /// slugged `/jobs/view/` path vs a bare numeric one, a `currentJobId=`
    /// query form — every host/path variant `job_identity` folds onto one
    /// `(board, id)`). `pub(crate)` so `extension_bridge`'s `applied.check`
    /// handler can run it as the read-only fallback beside
    /// [`Self::find_by_job_url`] (it never fetches or writes — see
    /// `resolve_applied_check_url`).
    ///
    /// Mirrors [`Self::find_by_job_url`]'s "most recent wins" ordering
    /// (`created_at DESC`). The stored half decodes unreserved escapes first
    /// ([`decode_unreserved`]) so a percent-encoded stored spelling compares
    /// equal to its literal — exactly the symmetric leniency
    /// `extension_bridge::agent_read::job_is_applied` applies to its applied-url
    /// set (issues #1128/#1166); both halves of the compare must run it or the
    /// mirror-image direction breaks.
    ///
    /// Deliberately a scan over a LIGHT projection — one prepared query
    /// selecting only `id, job_url, created_at`, the `created_at` max tracked
    /// in Rust, then the winner loaded whole via [`Self::get`] — NOT
    /// [`Self::list`]'s whole-row select: answering "does any stored url carry
    /// this posting's identity?" must not materialise every heavy column
    /// (`job_description`, `job_summary`, `brief`, `notes`) or parse every
    /// row's `answers` JSON to do so. The matching key is `job_identity`'s
    /// per-board id extraction, computed in Rust, so there is no
    /// SQL-expressible predicate an index (or `WHERE` clause) could speed up,
    /// and an index over every conceivable identity is not worth its write
    /// cost for a read-only fallback.
    ///
    /// The honest cost shape: ONE light projection scan per missed url,
    /// bounded per request by the `applied.check.batch` form's own
    /// `MAX_BATCH_URLS` cap on how many urls one call can carry — so even a
    /// fully-missing 50-url batch is at most 50 such scans. Fine for a typical
    /// applications table; a very LARGE table scanned repeatedly per batch url
    /// is the case that would justify caching or an index — not measured
    /// today, and not added until then.
    pub(crate) fn find_by_job_identity(
        &self,
        identity: &(&'static str, String),
    ) -> Option<Application> {
        // The identity compare is per-board id extraction (Rust), so this is a
        // plain scan of the light projection — deliberate, see the doc above.
        let winner_id = {
            let conn = self.conn.lock();
            let mut stmt = conn
                .prepare("SELECT id, job_url, created_at FROM applications WHERE job_url != ''")
                .ok()?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .ok()?;
            // "Most recent wins" mirrors `find_by_job_url`'s
            // `ORDER BY created_at DESC LIMIT 1`; tracked here so the
            // projection needs no ORDER BY.
            let mut winner: Option<(String, i64)> = None;
            for row in rows.flatten() {
                let (id, job_url, created_at) = row;
                let decoded = decode_unreserved(&job_url);
                if crate::scraping::scrape_url::job_identity(&decoded).as_ref() != Some(identity) {
                    continue;
                }
                let newer = match &winner {
                    Some((_, at)) => created_at > *at,
                    None => true,
                };
                if newer {
                    winner = Some((id, created_at));
                }
            }
            winner.map(|(id, _)| id)
        };
        // The scan's lock guard dropped with the block above; `get` re-locks.
        winner_id.and_then(|id| self.get(&id))
    }

    /// Normalized non-empty urls of Applications that are NOT `saved` — the set
    /// that derives a found job's `applied` flag (was: "a generation exists").
    /// Best-effort: a query failure collapses to empty, same as "applied to
    /// nothing" — fine for this fn's own callers (a cosmetic badge, or a
    /// best-effort filter over an ALREADY-known-present store), but NOT fine
    /// for a caller that needs to tell that failure apart from a real empty
    /// set — see [`Self::applied_job_urls_checked`] for that case.
    pub fn applied_job_urls(&self) -> std::collections::HashSet<String> {
        self.applied_job_urls_checked().unwrap_or_default()
    }

    /// Same query as [`Self::applied_job_urls`], but `None` means the query
    /// itself failed (a locked or corrupt applications DB) rather than
    /// "queried fine, found nothing" — round-4 fix T3-cont (PR #1182 round-5):
    /// `extension_bridge::agent_read`'s `store_present` derives from this, not
    /// from `try_state().is_some()` alone, because a MANAGED-but-unreadable
    /// store previously produced the exact same empty set as a genuinely
    /// empty one, so `job`/`found-jobs` reported a confident `applied: false`
    /// for a DB read failure — precisely the "cannot tell" case the
    /// `appliedUnavailable` marker exists to surface. A per-row decode
    /// failure (a malformed stored value) still degrades that ONE row rather
    /// than failing the whole call, matching `applied_job_urls`'s prior
    /// leniency.
    pub fn applied_job_urls_checked(&self) -> Option<std::collections::HashSet<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT job_url FROM applications WHERE job_url != '' AND status != 'saved'",
            )
            .ok()?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0)).ok()?;
        Some(rows.filter_map(|r| r.ok()).collect())
    }
}
