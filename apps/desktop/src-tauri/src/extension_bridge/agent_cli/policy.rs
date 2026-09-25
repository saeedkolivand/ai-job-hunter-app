//! ADR-038 §1 — the command policy table: every `#[tauri::command]` site
//! registered in `tauri::generate_handler!` (`lib.rs`, row count pinned by
//! `tests::policy_table_row_count_is_pinned`, never restated here),
//! classified by [`Effect`]. Phase 1 (this table) shipped with
//! nothing dispatching through it; Phase 2 (`super::super::agent_call`) reads
//! it to drive `agent call <ns>:<command>` — [`Effect::Read`] AND
//! [`Effect::Reversible`] rows dispatch directly (Phase 4), and
//! [`Effect::Irreversible`] rows dispatch only after a `--confirm` ceremony
//! whose expected value is named per-row by [`ProofSource`] (Phase 3) — but
//! ONLY [`Effect::Read`] rows are curated-tier-eligible; the value here is
//! the exactness test at the bottom: it is ADR-014's
//! (`docs/knowledge/decision-records/adr-014-cli-agent-shell-plugin-static-
//! allowlist.md`) static-allowlist invariant applied to *inbound* dispatch,
//! so a new command that lands in `generate_handler!` without a row here
//! fails CI instead of shipping silently reachable by a future caller.
//!
//! Four commands here carry zero renderer references (never called from the
//! UI, per ADR-038's own Context section) — flagged per-row below: `boards::
//! boards_list`, `privacy::privacy_clear_data`,
//! `support::support_get_system_info`, `resume::extract_resume`. One of the
//! four (`privacy_clear_data`) is destructive; a second (`resume::
//! extract_resume`) is `NotExposed` — zero UI callers turned out to matter
//! for more than dead-code hygiene once this table could dispatch it by name
//! (security review round 3, that row's own comment).
//!
//! ## Where the classification RULES live
//! In [`types`] — split out from this file under R8's LOC cap so the table
//! below has room to grow. That module owns [`Effect`]/[`ProofSource`]/
//! [`LookupInput`]/[`PolicyEntry`] together with the rule each variant
//! follows, and is re-exported here so every `policy::Effect`-style call
//! site is unaffected. Read it before adding or reclassifying a row.

// `POLICY`/`Effect`/`PolicyEntry` are now consumed by ADR-038 §2's
// `agent_call` dispatcher (`super::super::agent_call`) — kept for the odd
// field a future row might carry unread by any current match arm, mirroring
// the same allow every other exhaustively-matched policy-style table in
// this crate carries defensively.
#![allow(dead_code)]
mod types;
// Flat at `policy::` so the split is invisible to every existing call site
// (`agent_call`, `agent_call::proof`, `agent_cli::mcp`, `extension_bridge`'s
// tests) — see types.rs's own doc.
pub(crate) use types::{Effect, LookupInput, PolicyEntry, ProofSource};

// One shard per command domain, each holding a contiguous run of `POLICY`
// rows in the same order `lib.rs`'s `generate_handler!` uses. The array is
// concatenated from them by the const fn below, because `include!` cannot splice
// array ELEMENTS — the row order and every row's own bytes are unchanged.
mod rows_ai_embeddings_and_config;
mod rows_autopilot_and_notifications;
mod rows_bridge_and_email_watch;
mod rows_core_and_ai_generation;
mod rows_discovery_and_account;
mod rows_pipeline_resume_and_documents;

/// The table's total row count, summed from the shards at compile time so a row
/// added to one shard can never be silently dropped from the table by another.
const fn count(parts: &[&[PolicyEntry]]) -> usize {
    let mut total = 0;
    let mut i = 0;
    while i < parts.len() {
        total += parts[i].len();
        i += 1;
    }
    total
}

/// The shards, in `generate_handler!` order.
const PARTS: [&[PolicyEntry]; 6] = [
    rows_core_and_ai_generation::CORE_AND_AI_GENERATION,
    rows_ai_embeddings_and_config::AI_EMBEDDINGS_AND_CONFIG,
    rows_pipeline_resume_and_documents::PIPELINE_RESUME_AND_DOCUMENTS,
    rows_discovery_and_account::DISCOVERY_AND_ACCOUNT,
    rows_autopilot_and_notifications::AUTOPILOT_AND_NOTIFICATIONS,
    rows_bridge_and_email_watch::BRIDGE_AND_EMAIL_WATCH,
];

/// `count(PARTS)`, so the table's length is derived from the shards themselves
/// rather than restated here.
const TOTAL: usize = count(&PARTS);

/// Copy every shard into one array. `PolicyEntry` is `Copy`, so the seed value
/// is overwritten in full before `join` returns — no `unsafe`, no `MaybeUninit`.
/// The `assert!` is what keeps the seed from ever surviving into the result: it
/// pins the shards handed in to be the very `PARTS` whose sum is `TOTAL`. (The
/// seed is read from the first shard's first row, so an emptied leading shard
/// fails const eval on that index rather than padding the table with it.)
const fn join<const N: usize>(parts: [&[PolicyEntry]; N]) -> [PolicyEntry; TOTAL] {
    assert!(N == PARTS.len() && count(&parts) == TOTAL);
    let mut out = [parts[0][0]; TOTAL];
    let mut written = 0;
    let mut p = 0;
    while p < N {
        let shard = parts[p];
        let mut i = 0;
        while i < shard.len() {
            out[written] = shard[i];
            written += 1;
            i += 1;
        }
        p += 1;
    }
    out
}

/// Every registered command, grouped by source module in the same order
/// `lib.rs`'s `generate_handler!` list uses (so the two are easy to diff by
/// eye, not just by the test below).
const TABLE: [PolicyEntry; TOTAL] = join(PARTS);
pub(crate) const POLICY: &[PolicyEntry] = &TABLE;

#[cfg(test)]
mod tests;
