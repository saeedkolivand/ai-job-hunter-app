//! The staged résumé pipeline's IPC surface (L3).
//!
//! Five commands over `pipeline::resume`, plus [`hooks::RunHooks`] — the ONE
//! place a `pipeline:stage` event is emitted or a run row is written.
//!
//! ## What is backend-owned
//!
//! * **Routing** — `Completer::from_active`, never the request (task #25).
//! * **The budget** — `Budget::RESUME_QUALITY`, a compile-time constant. The
//!   wire schema has no `maxSteps`/`maxTokens`/`runTimeout` field to bind, so a
//!   compromised renderer has no unbounded-spend knob (pinned by
//!   `run_request_carries_only_identity_no_budget`).
//! * **The inputs, two ways in per side, ID WINS.** The résumé and the posting
//!   each resolve from EITHER a server-side lookup (`DocumentStore` by
//!   `resumeId`, the live postings cache by `jobId`) OR renderer-supplied text
//!   (`resumeText`/`jobAdText`) — added so a pasted job ad or an Autopilot
//!   found job, neither of which has a cache id or a stored résumé id, can
//!   still start a staged run. [`resume_source`]/[`job_source`] are the pure
//!   decision: a nonempty id ALWAYS wins over its matching text field (a run
//!   that sends both never silently prefers the text), and a miss on the id
//!   path is a hard error — it never falls back to the text field. Renderer
//!   text still reaches a prompt ONLY through the existing `fenced(...)` paths
//!   (ADR-010); this does not add a second way in for untrusted text.
//!
//! ## Where a run's pieces live
//!
//! The run store (`pipeline_runs.db`) holds the LIFECYCLE — status, stopped
//! reason, metrics, the per-stage trail. The DOCUMENT and its quality report
//! live in `ai_generations`, keyed by the posting url, because that is already
//! the per-job aggregate every other surface reads. `get`/`listForJob` join the
//! two rather than storing a second copy of a résumé.
//!
//! ## The two halves diverge, on purpose (the settled semantics)
//!
//! A run row is IMMUTABLE history; the aggregate is LIVE state. So the document
//! `get` returns is the posting's current résumé — which may be newer than the
//! run that produced it, because four things can move it: a later run,
//! [`resume_pipeline_regenerate_section`], the renderer's re-check save, and the
//! user's own editing (applying a "Remove" verdict is an ordinary hand edit).
//! Nothing tries to prevent that; the rules that make it coherent are:
//!
//! * **Run-owned, never overwritten:** `status`, `stoppedReason`, `metrics`,
//!   `depth`, `startedAt`/`finishedAt` and the stage trail. They describe what
//!   THIS run did.
//! * **Aggregate-owned, always current:** `resumeText` and `report`. Both come
//!   from `find_for_job`, so an older run's `get` shows the newest document — see
//!   [`ensure_latest_run`] for why the write commands refuse rather than fork.
//! * **`report.<slot>.sourceTextHash` is the join between them.** When it stops
//!   matching `resumeText`, the report describes an EARLIER version of the
//!   document; the renderer renders that as "checked before your edits" until a
//!   re-check. Stale-and-labelled is the honest state, and it is why an edit
//!   never has to invalidate a report.
//! * **A verdict survives the move.** [`report::record_decision`] stamps by
//!   `issueKey` inside the persisted wrapper and reads no text, and the editor's
//!   save path (`AiGenerationStore::update_texts`) never touches
//!   `quality_report`. What a caller must NOT do is write a fresh wrapper whose
//!   slot drops `fabrications`: `merge_quality_report` merges per TOP-LEVEL key,
//!   so an incoming `resume` slot replaces the stored one WHOLE. Carrying the
//!   review list forward is the writer's obligation (the renderer's
//!   `mergeRecheckedReport` does it; `report::build` reissues it from the fresh
//!   report). Both halves are pinned by
//!   `an_edit_that_moves_the_document_leaves_every_verdict_landable` and
//!   `a_save_replaces_a_slot_whole_so_the_writer_owns_the_review_list`.

pub mod hooks;
pub mod max;
pub mod notify;
mod persist;
// `pub(crate)`, not `mod`: each command stays reachable at
// `commands::resume_pipeline::<file>::<command>` for `shell/handler.rs`'s
// `generate_handler!` list, which needs the real path — `#[tauri::command]`
// generates hidden sibling items the macro looks up alongside the function
// itself, which a `pub use` re-export does not carry along.
pub(crate) mod read;
pub(crate) mod regenerate;
pub mod report;
mod resolve;
pub(crate) mod run;
mod save;

/// Re-exported so the tests that pin the decision name it, not the file it
/// happens to live in — `run`/`persist` (the two call sites) import directly
/// from `save`.
#[cfg(test)]
pub(crate) use save::{save_verdict, SaveVerdict};

#[cfg(test)]
mod tests;

// Test-only surface: these stay callable as `super::…`/`super::super::…` from
// the test tree without every topic file spelling out which sibling module
// actually owns the decision. Same visibility as the items' own — a
// `pub(super)` declared in a child module is `pub(in commands::resume_pipeline)`,
// and a re-export can only match it, never widen it.
#[cfg(test)]
pub(in crate::commands::resume_pipeline) use persist::empty_record;
#[cfg(test)]
pub(in crate::commands::resume_pipeline) use read::wire_artifact;
#[cfg(test)]
pub(in crate::commands::resume_pipeline) use regenerate::{
    ensure_latest_run, normalize_regenerated_projects, recomputed_status, run_wrote_a_resume,
};
// `resolve`'s pure decisions, re-imported so the test tree can keep reading
// `super::…`/`super::super::…` — `run`/`persist` import these directly from
// `resolve` instead.
#[cfg(test)]
use resolve::{
    clamp_request, job_ad_for_persist, job_meta_from_request, job_source, resume_source,
};

/// The `pipeline_runs.kind` discriminator for a résumé run.
///
/// Deliberately the FLOW, not the depth: the store's retention partitions on
/// `(job_url, kind)`, so every résumé run of a posting shares one three-run
/// history regardless of depth, while a future agent run of the same posting
/// keeps its own. Depth rides in the `depth` column, where it can change
/// without re-partitioning anyone's history.
pub const RUN_KIND: &str = "resume";

/// The `pipeline_runs.depth` value every NEW run writes. The column itself
/// stays (existing rows carry `"fast"`/`"quality"`/`"max"`, and the renderer
/// still needs the historic vocabulary to render an old run's label) — this
/// is the one value a run started by this build can produce, now that `fast`
/// never reached this pipeline and `max` was removed. No migration touches an
/// existing row.
const RUN_DEPTH: &str = "quality";

const STATUS_RUNNING: &str = "running";
pub(crate) const STATUS_COMPLETED: &str = "completed";
pub(crate) const STATUS_NEEDS_REVIEW: &str = "needsReview";
pub(crate) const STATUS_FAILED: &str = "failed";
pub(crate) const STATUS_CANCELLED: &str = "cancelled";
