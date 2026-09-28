//! Per-board reliability history (Track B1).
//!
//! Every scrape already computes a [`BoardScrapeSummary`] per board and then
//! throws it away everywhere except the last autopilot run — so a board that has
//! been failing for a week is indistinguishable from one that simply found
//! nothing today. This store keeps the *derived* answer to "is this board
//! working?" across runs, and the engine folds each run's summaries into it.
//!
//! ## Shape: aggregate state, not a run log
//!
//! The table holds **exactly one row per board**, upserted on every run — so it
//! is bounded by the number of DISTINCT board keys, not by time or by the
//! autopilot cadence, and there is no growth axis to prune and no retention
//! window to age out. That is a deliberate departure from a per-run history
//! table: everything the badge (and the "why did this job source fail?"
//! diagnostic) needs is a fold over the runs, and a bounded ring of raw rows
//! would additionally *lie* once the last success aged out of the window
//! (`last_success_at` would read `NULL` = "never worked" for a board that worked
//! fine last month).
//!
//! **That bound is only real because of a filter the CALLER applies.** `board`
//! is this table's PRIMARY KEY and it originates as a renderer-supplied string
//! (`commands::scrape` clones `req.boards` into the engine; the generated
//! `ScrapeBoardsRequest.boards` is an unvalidated `Vec<String>`), and the engine
//! deliberately lets an unknown id through to an ordinary error summary rather
//! than dropping it. So [`BoardHealthStore::record_run`] must only ever be
//! handed summaries whose board id the scraper RESOLVER recognised —
//! `ScraperEngine::record_health` filters on `resolvable_boards` for exactly
//! this reason. Without that filter a looping or XSS'd renderer could create
//! unbounded rows here, which is the threat `commands::scrape`'s limiter
//! already exists to stop. Guarded by
//! `engine::test::an_unresolvable_board_id_never_creates_a_health_row`.
//!
//! ## `skipped` is not `error`
//!
//! A skipped board (`needs-login` / `needs-company` / `needs-keys`) was never
//! contacted, so it verifies nothing: [`fold`] leaves **every** field untouched
//! for a skip — including `last_run_id`, which names the run that PRODUCED the
//! state and must therefore never name a run in which the board was not fetched.
//! A skip neither counts as a failure nor clears an existing failure streak — a
//! board that broke on Tuesday and has been skipped since Thursday is still
//! reported as broken since Tuesday, by Tuesday's run id.
//!
//! ## Correlation id
//!
//! No new id is minted: the scrape's existing `job_id` (`db::new_job_id`, the
//! same id `jobs_cancel` and the progress events use) is stored as
//! `last_run_id`, which is what turns a chip into something greppable in the
//! logs. It is stamped inside [`fold`]'s `Ok`/`Error` arms only, so it always
//! names a run that actually contacted the board.
//!
//! Wired like the other L1 stores: `db::open` + a transactional, position-indexed
//! migration (ADR-022), wiped on factory reset via `Resettable` (registered in
//! `commands::privacy`). Deliberately **not** part of the backup bundle
//! (`DataStore`), for the same reason `email_watch` isn't: it is machine-local
//! bookkeeping about *this* install's network luck, and restoring it onto another
//! machine would assert a history that machine never had.

mod fold;
mod store;
mod types;

pub use store::BoardHealthStore;
pub use types::{BoardHealth, BoardHealthEntry, BoardHealthStatus};

#[cfg(test)]
mod tests;
