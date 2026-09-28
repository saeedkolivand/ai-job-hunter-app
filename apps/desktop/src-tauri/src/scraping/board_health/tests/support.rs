//! Shared fixtures for the [`super::derivation`]/[`super::persistence`] suites:
//! a fixed epoch-ms base and the three `BoardScrapeSummary` builders every
//! test composes from.

use super::super::fold::fold;
use super::super::*;
use crate::scraping::engine::BoardScrapeSummary;

/// A fixed epoch-ms base so every expected timestamp in this file is a literal.
/// 2026-01-01T00:00:00Z.
pub(super) const T0: u64 = 1_767_225_600_000;
pub(super) const DAY: u64 = 24 * 60 * 60 * 1000;

/// `fold` with a fixed run id — the derivation tests below are about the
/// counters and timestamps; run-id stamping has its own tests.
pub(super) fn fold_at(
    prev: Option<BoardHealth>,
    summary: &BoardScrapeSummary,
    now: u64,
) -> BoardHealth {
    fold(prev, summary, "job-test", now)
}

pub(super) fn ok(board: &str, count: usize) -> BoardScrapeSummary {
    BoardScrapeSummary {
        board: board.to_string(),
        count,
        error: None,
        skipped: None,
        truncated: None,
        notes: Vec::new(),
        health: None,
    }
}

pub(super) fn failed(board: &str, reason: &str) -> BoardScrapeSummary {
    BoardScrapeSummary {
        error: Some(reason.to_string()),
        ..ok(board, 0)
    }
}

pub(super) fn skipped(board: &str, reason: &str) -> BoardScrapeSummary {
    BoardScrapeSummary {
        skipped: Some(reason.to_string()),
        ..ok(board, 0)
    }
}
