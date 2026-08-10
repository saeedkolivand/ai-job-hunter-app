//! Cursor-paginated `list_models` control flow, shared by every adapter that
//! paginates.
//!
//! Anthropic (`after_id`) and Gemini (`pageToken`) both cursor-paginate
//! `list_models` with the identical control flow — this ONE copy is that flow,
//! generic over the cursor type (`String` for both today). Living here once
//! means a future hardening (e.g. a stricter progress guard) can't apply to
//! one adapter and silently not the other, which is exactly the defect class
//! this codebase keeps re-discovering (see `docs/knowledge/automation-domain.md`
//! / the PR history around this feature).
//!
//! Split out of `mod.rs` (which is at its R8 LOC cap) exactly like
//! [`super::trace`] — every item is re-exported from `super`, so no call site
//! moves.

use crate::error::{AppError, AppResult};

/// Outcome of one paginated `list_models` iteration — see [`pagination_step`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaginationStep<T> {
    /// Fetch another page with this cursor.
    Continue(T),
    /// A genuine stopping point: [`advance_cursor`] reports NO continuation
    /// value present at all. The caller returns its accumulated results as
    /// `Ok`. Reserved STRICTLY for this case — never for a stalled cursor
    /// (see [`Self::Stalled`]), which looks the same from "did the loop
    /// stop" alone but means the opposite thing.
    Done,
    /// The provider reported another page exists (a non-empty cursor) but
    /// handed back the SAME cursor that fetched the page just parsed —
    /// neither a clean end-of-pages nor genuine progress. A prior fix
    /// stopped the loop here to avoid an infinite re-fetch, but folding this
    /// into [`Self::Done`] converted a hang into silent truncation: the
    /// caller must reject, not return the partial catalogue as `Ok`.
    Stalled,
    /// Ran out of the caller's page budget while the cursor was STILL
    /// genuinely advancing — there IS more catalogue this fetch didn't
    /// cover. The caller must reject rather than silently return an
    /// incomplete list.
    Incomplete,
}

/// Whether a freshly-reported cursor represents genuine pagination progress
/// from `current` — the three-way outcome [`pagination_step`] builds its
/// page-budget check on top of. Pure so the progress-guard is unit-testable
/// without a network mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorProgress<T> {
    /// No cursor at all — a clean end-of-pages.
    Done,
    /// A cursor IS present, but it's identical to `current` — see
    /// [`PaginationStep::Stalled`].
    Stalled,
    /// A genuinely new cursor — safe to continue.
    Continue(T),
}

/// Compare a freshly-reported `next` cursor against `current` (the one that
/// fetched the page just parsed). `None` (no cursor at all) and "the same
/// cursor came back" are DIFFERENT outcomes — the former is a clean stop,
/// the latter means the provider claims more data exists but gave no way to
/// reach it, which must surface as an error rather than being silently
/// treated as "done".
pub fn advance_cursor<T: PartialEq>(current: &Option<T>, next: Option<T>) -> CursorProgress<T> {
    match next {
        None => CursorProgress::Done,
        Some(next) if current.as_ref() == Some(&next) => CursorProgress::Stalled,
        Some(next) => CursorProgress::Continue(next),
    }
}

/// One step of a page-budget-bounded pagination loop's control flow: given
/// the 0-based index of the page JUST fetched, the caller's own page-budget
/// bound (each adapter's `MAX_LIST_MODELS_PAGES`), and the raw `next` cursor
/// that page reported, decide whether to continue, stop cleanly, stop
/// stalled, or stop incomplete. Pure (no I/O) so the exact page-budget
/// boundary AND the stalled-cursor guard are unit-testable without live HTTP
/// round-trips — each adapter's `list_models` loop calls this once per page
/// and dispatches on the result, so this function (not a hand-duplicated
/// copy per adapter) is what actually runs in production.
pub fn pagination_step<T: PartialEq>(
    page_index: usize,
    max_pages: usize,
    current: &Option<T>,
    next: Option<T>,
) -> PaginationStep<T> {
    match advance_cursor(current, next) {
        CursorProgress::Done => PaginationStep::Done,
        CursorProgress::Stalled => PaginationStep::Stalled,
        CursorProgress::Continue(id) if page_index + 1 < max_pages => PaginationStep::Continue(id),
        CursorProgress::Continue(_) => PaginationStep::Incomplete,
    }
}

/// Race `fut` against the cumulative pagination `deadline`, converting a
/// timeout into `AppError::Network`. Used for EVERY network I/O step of a
/// paginated `list_models` fetch — the send, the error-body read, and the
/// JSON parse — not just the initial `send()`. Wrapping only `send()` was
/// the exact gap that let a stalled body read blow straight through
/// `LIST_MODELS_TOTAL`: `send()` resolves once headers arrive, so a
/// provider whose BODY is slow to arrive after that point escaped a
/// deadline that only covered the send. One shared wrapper for every step
/// means a future 4th I/O call in this loop can't repeat that gap by
/// omission.
pub async fn bounded<F: std::future::Future>(
    deadline: tokio::time::Instant,
    provider: &str,
    fut: F,
) -> AppResult<F::Output> {
    tokio::time::timeout_at(deadline, fut)
        .await
        .map_err(|_elapsed| {
            AppError::Network(format!(
                "{provider}: timed out listing models across multiple pages"
            ))
        })
}
