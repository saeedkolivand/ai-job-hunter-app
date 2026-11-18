//! Shell-boundary seam to the posting history store (ghost-job signals).
//! Not an IPC command: the ingest paths (`scrape::run`, `autopilot::run`) call it.
//!
//! Call sites build a "search key" (what was searched) and pass the per-board
//! summaries; this module turns them into the per-source evidence the store
//! needs (see `posting_history`). Callers must NOT record a cancelled run.

use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::autopilot::AutopilotTarget;
use crate::db::now_ms;
use crate::error::AppResult;
use crate::posting_history::{source_id, GhostSignals, PostingHistoryStore};
use crate::scraping::{BoardScrapeSummary, BoardSearchInput, JobPosting};

/// One scraped posting, owned, so the unfiltered batch can be kept while the
/// postings themselves are filtered and moved.
pub(crate) struct SeenPosting {
    company: String,
    title: String,
    url: String,
    board: String,
}

pub(crate) fn seen_of(postings: &[JobPosting]) -> Vec<SeenPosting> {
    postings
        .iter()
        .map(|p| SeenPosting {
            company: p.company.clone(),
            title: p.title.clone(),
            url: p.url.clone(),
            board: p.source.clone(),
        })
        .collect()
}

/// The search a manual scrape ran: everything that narrows or widens its results.
pub(crate) fn input_key(i: &BoardSearchInput) -> String {
    format!(
        "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        i.query,
        i.location,
        i.country_code,
        i.date_filter,
        i.job_type,
        i.work_types,
        i.experience_level,
        (i.easy_apply, i.actively_hiring, i.verified),
        i.companies,
        i.pages
    )
}

/// The search an autopilot run ran (its target, minus the board list).
pub(crate) fn target_key(t: &AutopilotTarget) -> String {
    format!(
        "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        t.query,
        t.location,
        t.country_code,
        t.date_filter,
        t.work_types,
        t.watched_companies_only,
        t.pages
    )
}

/// Boards that ran to completion: no error, no skip, no partial harvest. An
/// empty successful board counts: it is evidence of absence.
fn ok_boards(summaries: &[BoardScrapeSummary]) -> impl Iterator<Item = &str> {
    summaries
        .iter()
        .filter(|s| s.error.is_none() && s.skipped.is_none() && s.truncated.is_none())
        .map(|s| s.board.as_str())
}

/// Record one scrape and return each ghost key's signals. Best-effort like
/// every harvest seam: a missing store or a write error yields no signals
/// (logged by code only), never a failed run.
pub(crate) fn record_scrape(
    app: &AppHandle,
    seen: &[SeenPosting],
    search_key: &str,
    summaries: &[BoardScrapeSummary],
) -> HashMap<String, GhostSignals> {
    let ok: Vec<String> = ok_boards(summaries)
        .map(|b| source_id(b, search_key))
        .collect();
    let sources: Vec<String> = seen
        .iter()
        .map(|p| source_id(&p.board, search_key))
        .collect();
    let recorded = app.try_state::<PostingHistoryStore>().map(|store| {
        let rows = seen.iter().zip(&sources).map(|(p, s)| {
            (
                p.company.as_str(),
                p.title.as_str(),
                p.url.as_str(),
                s.as_str(),
            )
        });
        store.record_scrape(rows, &ok, now_ms())
    });
    best_effort(recorded)
}

/// `None` = store not managed. Either that or an error degrades to "no signals".
fn best_effort(
    recorded: Option<AppResult<HashMap<String, GhostSignals>>>,
) -> HashMap<String, GhostSignals> {
    match recorded {
        Some(Ok(signals)) => signals,
        Some(Err(e)) => {
            log::warn!("[posting_history] scrape not recorded: {}", e.code());
            HashMap::new()
        }
        None => HashMap::new(),
    }
}

#[cfg(test)]
mod tests;
