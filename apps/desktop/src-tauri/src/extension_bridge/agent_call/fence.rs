//! Fencing scraped job-posting text on the way OUT of a dispatched command's response — a
//! different axis from the raw-data decision in `agent_call.rs`'s own module doc (ADR-038's own
//! amendment paragraph). [`fence_scraped_fields`] is the ONE entry point every dispatch response
//! walks through; everything else here is its own internal machinery.
//!
//! R8 LOC-cap split (`docs/architecture-rules.md`), the same move `agent_call/reshape.rs` and
//! `agent_call/proof.rs` already made: this is the FENCING unit — the audited field-name/shape
//! tables and the walk itself — so nothing about policy, refusal vocabulary or dispatch travelled
//! with it. `reshape.rs`'s own inbound mirror ([`super::reshape::unfence_named_fields_recursive`])
//! stays where it is; it reaches these tables and [`fence_scraped_fields`] the SAME way it always
//! did — `use super::*` — since a sibling module's `pub(super)` item is exactly as reachable
//! through the parent as a item defined directly in the parent would be. Every item below is
//! `pub(super)` (visible within `agent_call`) for that reason: `agent_call/tests.rs` exercises
//! this machinery directly, the same as it did before the split.
//!
//! Split further under this same R8 cap, by concern: the audited field-name tables
//! (`tables`), the shape-anchor tables for the non-name-keyed rules (`shape_tables`), the
//! name-keyed recursive walk itself (`named_fields`), and the scrape-diagnostics/leaf-fencing
//! helpers that walk delegates to (`shape_helpers`).

use serde_json::Value;

mod named_fields;
mod shape_helpers;
mod shape_tables;
mod tables;

pub(super) use named_fields::fence_named_fields_recursive;
pub(super) use shape_tables::{is_application_answer_shaped, APPLICATION_ANSWER_QUESTION_FIELD};
pub(super) use tables::FENCE_FIELD_NAMES;

/// Fence every [`FENCE_FIELD_NAMES`] string (or string array element)
/// anywhere in `data`'s tree — recurses through the WHOLE response (not just
/// a top-level object/array, MEDIUM fix — security review round 1), and runs
/// UNCONDITIONALLY for every dispatched command rather than gating on a
/// command allowlist (HIGH fix — security review round 2): a new command
/// whose response embeds one of these EXACT field names is fenced
/// automatically, without needing an entry added here first. Also fences any
/// unclassified string field on a [`JOB_POSTING_ANCHOR_FIELDS`]-detected
/// object (HIGH fix — security review round 3), closing the residual gap a
/// field-name allowlist alone cannot: `JobPosting.extra`'s board-chosen keys.
/// See `every_known_posting_text_carrier_is_a_real_freely_
/// dispatchable_policy_row` (tests) for the audited list of rows this is
/// known to protect.
///
/// Some rules are keyed on an object's SHAPE rather than a field name,
/// because a name alone cannot tell two carriers apart:
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] fences a scraped ATS `question`
/// without touching `InterviewQuestion.question`;
/// [`JOB_RECORD_ANCHOR_FIELDS`] exempts a job's own `result` so a generation
/// read back through `jobs_get` is not labelled as scraped posting text; and
/// [`SCRAPE_SUMMARY_ANCHOR_FIELDS`]/[`BOARD_HEALTH_ANCHOR_FIELDS`] fence the
/// board-WRITTEN strings on a `BoardScrapeSummary`/`BoardHealth` without
/// touching this app's own same-named `error` strings — including inside
/// that exempt `result`, which is where a completed `scrape_boards` job puts
/// them.
pub(super) fn fence_scraped_fields(data: &mut Value) {
    fence_named_fields_recursive(data);
}
