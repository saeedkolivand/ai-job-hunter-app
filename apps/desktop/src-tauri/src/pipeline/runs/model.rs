//! The run store's row types, byte caps, and the clamp that enforces them on
//! the two free-form JSON columns (`artifact_json`/`metrics_json`).

/// Byte cap on ONE event's `artifact_json`, MARKER INCLUDED.
///
/// An artifact is a stage SUMMARY — counts, section keys, a stopped reason —
/// never the generated document, so 16 KiB is roughly two orders of magnitude
/// more than any honest payload needs. The cap exists for the dishonest one: a
/// stage that accidentally hands its whole model output to the recorder would
/// otherwise write a multi-megabyte row per stage, for every stage, forever.
/// Oversized values are TRUNCATED (never dropped) so the trail still shows the
/// stage ran — see [`clamp_artifact`], which cuts on a UTF-8 boundary and
/// appends [`TRUNCATION_MARKER`] so a reader can tell.
///
/// The marker is charged AGAINST the cap, not added on top of it: a cap that a
/// clamped value can exceed is not a cap.
pub const ARTIFACT_CAP_BYTES: usize = 16 * 1024;

/// Byte cap on ONE run's `metrics_json`, MARKER INCLUDED — the run-level twin
/// of [`ARTIFACT_CAP_BYTES`].
///
/// Smaller because the payload is smaller: run metrics are counts and durations
/// (token totals, per-stage milliseconds), never text. 4 KiB fits hundreds of
/// numeric fields while still bounding a caller that hands the recorder
/// something it should not have — and, on the import path, a hand-edited backup
/// that would otherwise restore a multi-megabyte row permanently.
pub const METRICS_CAP_BYTES: usize = 4 * 1024;

/// Appended to a clamped JSON column so truncation is visible rather than
/// silent. Deliberately not valid JSON: a truncated value is NOT a parseable
/// value, and a reader that tries must fail rather than read half an object as a
/// whole one.
pub const TRUNCATION_MARKER: &str = "…[truncated]";

// A cap smaller than the marker it reserves room for would underflow the clamp's
// body budget. Compile-time, so shrinking a cap past its marker fails the BUILD.
const _: () = assert!(ARTIFACT_CAP_BYTES > TRUNCATION_MARKER.len());
const _: () = assert!(METRICS_CAP_BYTES > TRUNCATION_MARKER.len());

/// Runs kept per `(job_url, kind)`. Three is "the current one plus the two you
/// might want to compare it against": run history is a debugging aid, and the
/// fourth attempt at the same posting has never been the interesting one.
///
/// The partition is per posting AND per [`RunRow::kind`], not global: a user who
/// runs one job repeatedly cannot evict every other job's history, and — since
/// these tables host every staged run — three résumé runs cannot evict the same
/// posting's agent-run trail. `kind` is this module's first-class discriminator,
/// so it discriminates retention too.
pub const RETENTION_RUNS_PER_JOB: usize = 3;

/// Byte cap on an IMPORTED `id` / `run_id`.
///
/// Ids are GENERATED, never typed: the widest this app can produce is a uuid
/// (36 bytes), so 128 is more than three times the real ceiling while still
/// bounding a hand-edited bundle. Rejected, not truncated — see [`super::import_export::check_len`].
pub const IMPORT_ID_CAP_BYTES: usize = 128;

/// Byte cap on an IMPORTED `job_url` — deliberately the loosest of the three.
///
/// A real posting URL is 60–300 bytes, but board URLs carry tracking query
/// strings and this column is a RETENTION KEY: a rejected import is worse than
/// a long URL, so the cap sits at the de-facto HTTP request-line ceiling
/// (2 KiB) rather than at anything measured from today's boards.
pub const IMPORT_JOB_URL_CAP_BYTES: usize = 2_048;

/// Byte cap on the small label columns — `kind`, `depth`, `status`,
/// `stopped_reason`, and an event's `stage`.
///
/// Every value these hold is a short backend-chosen token (`"resume"`,
/// `"full"`, `"running"`, `"max_tool_calls"`, `"draft"`); the widest today is
/// 14 bytes. 64 leaves room for names nobody has invented yet without leaving
/// the door open for a megabyte of prose.
pub const IMPORT_LABEL_CAP_BYTES: usize = 64;

/// Max runs one imported bundle may carry.
///
/// Retention keeps [`RETENTION_RUNS_PER_JOB`] runs per `(job_url, kind)`, so
/// 5 000 runs is ~1 600 postings' worth of full history for a single kind —
/// far past what a real backup holds, which is exactly what a hostile-input
/// bound should be. It bounds what gets PERSISTED and the per-row SQLite work,
/// NOT the transient parse: `data_import` has already read the file into a
/// `String` and `serde_json` has already built the bundle by the time this is
/// checked. A byte cap is not a memory cap.
pub const IMPORT_MAX_RUNS: usize = 5_000;

/// Max events one imported bundle may carry — ten per run at
/// [`IMPORT_MAX_RUNS`], which is a five-stage run's `start`/`finish` pairs.
/// Same "bounds the writes, not the parse" caveat as the run cap.
pub const IMPORT_MAX_EVENTS: usize = 50_000;

/// One recorded run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRow {
    pub id: String,
    /// The posting this run was for — half of the retention key, the other half
    /// being [`Self::kind`] (see [`super::store::PipelineRunStore::prune`]).
    pub job_url: String,
    /// Which kind of run this is (`"resume"`, `"agent"`, …). The discriminator
    /// that lets one pair of tables host every staged run.
    pub kind: String,
    /// The flow's depth/profile label (e.g. `"brief"`/`"full"`), free-form.
    pub depth: String,
    /// `"running"` until a terminal update lands.
    pub status: String,
    pub started_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    /// The wire form of [`crate::pipeline::budget::StoppedReason`], stored as
    /// TEXT rather than an enum column so a variant added later (Phase 3 adds
    /// three) needs no migration and an older bundle still restores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_reason: Option<String>,
    /// Free-form run metrics as a JSON object string (token counts, durations),
    /// clamped to [`METRICS_CAP_BYTES`] at every write.
    #[serde(default = "empty_json_object")]
    pub metrics_json: String,
}

/// One stage event within a run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventRow {
    pub run_id: String,
    /// Monotonic per-run ordinal. Part of the primary key, so a replayed event
    /// overwrites rather than duplicating.
    pub seq: u32,
    pub ts: u64,
    pub stage: String,
    /// The stage lifecycle phase — a CLOSED vocabulary, unlike
    /// [`RunRow::stopped_reason`]: `start`/`finish`/`error` and nothing else,
    /// frozen by `PIPELINE_STAGE_PHASES` in
    /// `packages/shared/src/events/pipeline.ts` and enforced at the SCHEMA by a
    /// CHECK (see [`super::store::CREATE_PIPELINE_RUNS_SQL`]), so a bogus phase cannot enter
    /// through the write path OR through a hand-edited backup.
    ///
    /// Typed `String` rather than an enum because it is a wire/row value shared
    /// with the TS contract; the CHECK is what makes the set closed, and
    /// `phase_check_matches_the_generated_contract` fails if the two drift.
    pub phase: String,
    /// The stage's summary payload, already clamped to [`ARTIFACT_CAP_BYTES`].
    pub artifact_json: String,
}

pub(super) fn empty_json_object() -> String {
    "{}".to_string()
}

/// Clamp one free-form JSON column to `cap` BYTES INCLUSIVE of the truncation
/// marker, cutting on a UTF-8 character boundary and marking the cut.
///
/// Byte-based (not char-based) because the cap protects the DB file, and a
/// char-based cap on multi-byte text bounds nothing useful — a 16k-char CJK
/// artifact is 48 KB. Pure, so the boundary arithmetic is directly testable.
///
/// The marker's own length is RESERVED out of the cap, so the returned string is
/// never longer than `cap`. Both call sites pass a cap larger than the marker
/// (asserted at compile time above), so the subtraction cannot underflow.
fn clamp_json(value: &str, cap: usize) -> String {
    if value.len() <= cap {
        return value.to_string();
    }
    // Walk back to the last char boundary at or below the body budget so the
    // result is always valid UTF-8 (`String` cannot hold anything else).
    let mut end = cap - TRUNCATION_MARKER.len();
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{TRUNCATION_MARKER}", &value[..end])
}

/// Clamp one event's `artifact_json` to [`ARTIFACT_CAP_BYTES`].
pub fn clamp_artifact(artifact: &str) -> String {
    clamp_json(artifact, ARTIFACT_CAP_BYTES)
}

/// Clamp one run's `metrics_json` to [`METRICS_CAP_BYTES`].
pub fn clamp_metrics(metrics: &str) -> String {
    clamp_json(metrics, METRICS_CAP_BYTES)
}
