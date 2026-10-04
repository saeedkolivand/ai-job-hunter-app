//! The answer list of an [`Application`](super::Application): the append-only merge the
//! extension's `answers.save` uses, the by-question merge every in-app writer uses,
//! and the shared question normalizer / id minting / cumulative cap.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). The two merges are deliberately different rules
//! (existing wins vs incoming wins) and must be read side by side.

use rusqlite::params;
use uuid::Uuid;

use super::{ApplicationStore, MAX_TOTAL_ANSWERS};
use crate::ai_generations::ApplicationAnswer;
use crate::db::{now_ms, ts_to_db};
use crate::error::{AppError, AppResult};

impl ApplicationStore {
    /// Append newly-captured extension answers onto Application `id`'s answer
    /// list — an APPEND-only dedup merge, `pub(crate)` so the extension
    /// bridge's `answers.save` handler
    /// (`extension_bridge::answers_save::resolve_answers_save`) is the first
    /// caller. Deliberately independent of
    /// [`Self::merge_answers_by_question`] (`upsert_internal`'s meta-merge
    /// path, used by every in-app writer — `ai_generations_save`, manual
    /// edits, legacy-generation backfill): that path lets `incoming` win for
    /// a matching question, because those writers legitimately re-save
    /// EDITED text for a question the user already answered. THIS method
    /// must never do that — a stray/duplicate extension re-capture of the
    /// same page must never clobber an answer the user already reviewed —
    /// so here the EXISTING answer always wins and only genuinely new
    /// questions are appended.
    ///
    /// Dedup key: the NORMALIZED question text (trim + lowercase + collapse
    /// internal whitespace runs) compared against the CURRENT answers — an
    /// existing answer for a given question always wins and is NEVER
    /// overwritten, so a re-capture of the same page only ever adds
    /// genuinely new questions. The dedup set also accumulates across
    /// `incoming` itself, so two same-normalized entries in one call collapse
    /// to a single added answer rather than two. A blank (post-trim) question
    /// is dropped.
    ///
    /// The dedup READ and the WRITE happen in the SAME transaction (via
    /// [`Self::row_by_id_conn`], not the self-locking [`Self::get`] — calling
    /// `get` here would deadlock the non-reentrant `parking_lot::Mutex`), so
    /// there is no earlier separate read a concurrent caller could race
    /// against; only `answers` + `updated_at` are touched. Returns the count
    /// of NEWLY ADDED answers (`0` when every captured question was already
    /// present or blank).
    ///
    /// Capped at [`MAX_TOTAL_ANSWERS`] merged answers per application: once
    /// that many are stored, further incoming entries are dropped rather than
    /// appended (they count toward the caller's `skipped`, same as a dedup
    /// hit — never rejected outright).
    pub(crate) fn merge_answers(
        &self,
        id: &str,
        incoming: Vec<ApplicationAnswer>,
    ) -> AppResult<usize> {
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;

        let existing = Self::row_by_id_conn(&tx, id)?
            .ok_or_else(|| AppError::Validation(format!("application not found: {id}")))?;

        let mut seen: std::collections::HashSet<String> = existing
            .answers
            .iter()
            .map(|a| normalize_question(&a.question))
            .collect();

        let mut merged = existing.answers;
        let mut added = 0usize;
        for ans in incoming {
            if merged.len() >= MAX_TOTAL_ANSWERS {
                break; // per-application cap hit — remaining entries count as skipped
            }
            let key = normalize_question(&ans.question);
            if key.is_empty() || !seen.insert(key) {
                continue; // blank question, or an existing answer already wins
            }
            merged.push(ApplicationAnswer {
                id: make_answer_id(),
                question: ans.question,
                answer: ans.answer,
            });
            added += 1;
        }

        if added > 0 {
            // `?` (not `.unwrap_or_else(|_| "[]".into())`): a serialize failure
            // must abort the transaction (never committed, so the existing
            // `answers` column is untouched) rather than writing an empty `[]`
            // that would silently wipe every previously-stored answer.
            let answers_json = serde_json::to_string(&merged)?;
            tx.execute(
                "UPDATE applications SET answers = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, answers_json, ts_to_db(now_ms())],
            )?;
        }
        tx.commit()?;
        Ok(added)
    }

    /// Merge `incoming` onto `existing` by NORMALIZED question text —
    /// `upsert_internal`'s `answers` merge path, used on every in-app
    /// writer's re-upsert (`ai_generations_save`'s AI-generated answer set,
    /// `track_manual`, the legacy-generation backfill). Unlike
    /// [`Self::merge_answers`] (the extension's separate append-only
    /// capture path, where an EXISTING answer always wins), here `incoming`
    /// wins for a matching question: `ai_generations_save` re-saves the
    /// CURRENT full answer set on every call, including in-app edits to a
    /// question the user already answered, and "existing wins" would
    /// silently discard that edit. Existing answers for a question NOT
    /// present in `incoming` are preserved untouched — this is what fixes
    /// the previous wholesale-replace data-loss hazard, where a non-empty
    /// `meta.answers` simply became the whole stored list, dropping every
    /// answer another writer (e.g. the extension's `answers.save`) had
    /// appended in between. An empty `incoming` is a no-op. Same
    /// [`MAX_TOTAL_ANSWERS`] cap as `merge_answers`, but the cap only ever
    /// blocks a genuinely NEW question: a same-question replacement is a
    /// swap for an entry already removed from `merged` below, so it is
    /// always applied regardless of the cap — otherwise a legacy row that
    /// already sits at/over the cap (seeded before this cap existed) would
    /// have the matching existing answer removed to make room, then the cap
    /// check block the incoming replacement from ever being pushed back in,
    /// making the question vanish entirely instead of being rewritten.
    pub(super) fn merge_answers_by_question(
        existing: Vec<ApplicationAnswer>,
        incoming: Vec<ApplicationAnswer>,
    ) -> Vec<ApplicationAnswer> {
        if incoming.is_empty() {
            return existing;
        }
        let existing_keys: std::collections::HashSet<String> = existing
            .iter()
            .map(|a| normalize_question(&a.question))
            .collect();
        let incoming_keys: std::collections::HashSet<String> = incoming
            .iter()
            .map(|a| normalize_question(&a.question))
            .filter(|k| !k.is_empty())
            .collect();
        // Existing answers survive as-is unless `incoming` carries a
        // (re)answer for the same question — those are dropped here and
        // replaced below.
        let mut merged: Vec<ApplicationAnswer> = existing
            .into_iter()
            .filter(|a| !incoming_keys.contains(&normalize_question(&a.question)))
            .collect();

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for ans in incoming {
            let key = normalize_question(&ans.question);
            if key.is_empty() || !seen.insert(key.clone()) {
                continue; // blank question, or a duplicate within this same incoming batch
            }
            // A same-question replacement always wins (see doc comment above);
            // only a brand-new question is subject to the cap. `continue`
            // (not `break`) so a later replacement in this same batch still
            // gets applied even after a run of new-question entries hit it.
            if !existing_keys.contains(&key) && merged.len() >= MAX_TOTAL_ANSWERS {
                continue; // per-application cap — this new-question entry is dropped
            }
            merged.push(ans);
        }
        merged
    }
}

/// Fresh id for an answer merged in by [`ApplicationStore::merge_answers`] —
/// same shape as [`make_application_id`].
fn make_answer_id() -> String {
    format!("ans-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}

/// Normalize a question for dedup comparison in [`ApplicationStore::merge_answers`]
/// (and reused by `extension_bridge::answers_suggest`'s matcher): trim, lowercase,
/// collapse whitespace — "Why  this role?" and "why this role?" dedup to one key.
pub(crate) fn normalize_question(q: &str) -> String {
    q.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
