//! `execute`'s input resolution: the clamp + the ID-WINS decision between a
//! server-side lookup and renderer-supplied text, for both the résumé and the
//! job ad. Split out of `mod.rs` to stay under R8's per-module LOC cap
//! (`docs/architecture-rules.md`) — every item here is `pub(crate)` because
//! `execute`/`persist_document` (in `mod.rs`) and this module's own tests
//! (`test.rs`, a sibling under `resume_pipeline`) all need them, not because
//! anything outside `commands::resume_pipeline` does.
//!
//! **The branch semantics live HERE, as pure functions over an already-decided
//! [`ResumeSource`]/[`JobSource`], not inline in `execute`.** A source-text
//! substring check on `execute`'s body ("the Store arm's text must not
//! mention `resume_text`") pins a SPELLING — a mutation that reads the same
//! value through a differently-named local (`let aliased = clamped.resume_text
//! .clone(); … .unwrap_or(aliased)`) passes it while changing the behavior it
//! exists to guard. [`resolve_resume`]/[`resolve_job`] close that off
//! structurally instead of lexically: the `Store`/`Cache` arms receive ONLY an
//! id and an injected lookup closure, so there is no `resume_text`/
//! `job_ad_text` value in scope for a mutation to reach for, accidentally or
//! otherwise. `execute` still owns the actual `DocumentStore`/postings-cache
//! calls (they need an `AppHandle` this crate has no test harness for) —
//! these functions take the lookup as a parameter so the DECISION is testable
//! without one.

use crate::error::{AppError, AppResult};
use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;

/// The renderer-supplied free text of one run request, CLAMPED server-side.
///
/// The wire schema's `.max(…)` caps are Zod, and Zod does not run on this
/// transport (`tauri-client` calls `invoke` directly, no parse on the way out),
/// so serde accepts whatever a direct IPC caller sends. Every other command
/// that takes this same text mirrors its caps in Rust —
/// `commands::resume::resume_validate_content` is where these constants live,
/// and they are HOISTED from there rather than re-declared, because two copies
/// of "50 requirements, 300 bytes each" is exactly how one of them drifts.
pub(crate) struct ClampedRequest {
    pub(crate) job_url: String,
    pub(crate) target_language: String,
    pub(crate) top_requirements: Vec<String>,
    pub(crate) cover_letter: String,
    /// `resumeId`, clamped — the only other renderer string on this command,
    /// besides `jobId`, that used to reach `execute` unbounded. It is echoed
    /// into a validation error message on a miss (renderer-visible) and into
    /// the run's `metrics_json` (`sourceResumeId`) on a hit, so an unbounded
    /// copy would let a hostile direct-IPC caller grow both without limit.
    pub(crate) resume_id: String,
    /// `jobId`, clamped — same treatment as `resume_id`, for the same two
    /// echo points on the job side (`"job not found in cache: {id}"` and the
    /// retention-key path a `Cache` run takes).
    pub(crate) job_id: String,
    /// The id-less résumé path (`resumeText`) — read only when
    /// [`resume_source`] resolves to [`ResumeSource::Text`].
    pub(crate) resume_text: String,
    /// The id-less job-ad path (`jobAdText`) — read only when [`job_source`]
    /// resolves to [`JobSource::Text`].
    pub(crate) job_ad_text: String,
    /// Posting identity for the text path only — mirrors what
    /// `job_meta_for` would have read off the cached posting.
    pub(crate) job_title: String,
    pub(crate) company_name: String,
    pub(crate) board: String,
}

// `include_cover_letter` is a plain `bool` — no free text to clamp, so it rides
// straight from `req` into `QualityInput` at the call site rather than through
// `ClampedRequest`, which exists only for fields that need one.

/// Byte cap on the request's `jobUrl` — mirrors the schema's `.max(2_048)`.
/// This value is a STORAGE key (the run row's retention partition and the
/// aggregate lookup), so an unbounded one writes an unbounded row.
pub(crate) const JOB_URL_CAP: usize = 2_048;

/// Byte cap on the request's `jobTitle`/`companyName`/`resumeId`/`jobId` —
/// mirrors the schema's `.max(512)`. The two ids share this cap with the
/// text-path identity fields rather than getting their own: an id is a
/// short opaque token in every store that issues one (`uuid`/nanoid-shaped),
/// so the SAME "short identifier" cap class applies, and a second
/// differently-named constant for the exact same bound is the kind of
/// drift `docs/architecture-rules.md` warns this file about elsewhere.
pub(crate) const JOB_IDENTITY_CAP: usize = 512;
/// Byte cap on the request's `board` — mirrors the schema's `.max(64)`. Board
/// identifiers are short slugs (`"linkedin"`, `"indeed"`, an aggregator name),
/// hence the much smaller cap than [`JOB_IDENTITY_CAP`].
pub(crate) const BOARD_CAP: usize = 64;

/// Clamp every renderer-supplied free-text field of a run request. Pure, so the
/// caps are a test rather than a claim.
pub(crate) fn clamp_request(req: &ResumePipelineRunRequest) -> ClampedRequest {
    use crate::applications::{clamp_to_bytes, MAX_JOB_DESCRIPTION_BYTES};
    use crate::commands::resume::{
        TARGET_LANGUAGE_CAP, TOP_REQUIREMENTS_CAP, TOP_REQUIREMENT_BYTES_CAP,
    };

    ClampedRequest {
        job_url: clamp_to_bytes(req.job_url.clone(), JOB_URL_CAP),
        target_language: clamp_to_bytes(req.target_language.clone(), TARGET_LANGUAGE_CAP),
        top_requirements: req
            .top_requirements
            .iter()
            .take(TOP_REQUIREMENTS_CAP)
            .map(|r| clamp_to_bytes(r.clone(), TOP_REQUIREMENT_BYTES_CAP))
            .collect(),
        cover_letter: clamp_to_bytes(req.cover_letter_text.clone(), MAX_JOB_DESCRIPTION_BYTES),
        resume_id: clamp_to_bytes(req.resume_id.clone(), JOB_IDENTITY_CAP),
        job_id: clamp_to_bytes(req.job_id.clone(), JOB_IDENTITY_CAP),
        // Same cap class as `resumeText`/`jobText` elsewhere
        // (`ResumeTrimSuggestionsRequestSchema`, `MatchResumeRequest`'s own
        // clamp) and as `cover_letter` above — all three are "a whole
        // résumé/posting", never a snippet.
        resume_text: clamp_to_bytes(req.resume_text.clone(), MAX_JOB_DESCRIPTION_BYTES),
        job_ad_text: clamp_to_bytes(req.job_ad_text.clone(), MAX_JOB_DESCRIPTION_BYTES),
        job_title: clamp_to_bytes(req.job_title.clone(), JOB_IDENTITY_CAP),
        company_name: clamp_to_bytes(req.company_name.clone(), JOB_IDENTITY_CAP),
        board: clamp_to_bytes(req.board.clone(), BOARD_CAP),
    }
}

/// Where one run's résumé text comes from — the pure half of the ID-WINS rule
/// `execute` applies. Pure and total over the two clamped inputs, so the rule
/// is a test rather than a claim; the actual `DocumentStore` lookup needs an
/// `AppHandle` this crate has no harness for, so it stays in `execute`
/// (behind [`resolve_resume`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResumeSource<'a> {
    /// A nonempty `resumeId` — look this up; a miss is a HARD ERROR, never a
    /// fallback to `resumeText`, even when the request also carries one.
    Store(&'a str),
    /// `resumeId` was empty and `resumeText` was not.
    Text(&'a str),
}

/// `None` when both are empty (or whitespace-only) — `execute` turns that into
/// a validation error, since a run needs a résumé from somewhere.
pub(crate) fn resume_source<'a>(
    resume_id: &'a str,
    resume_text: &'a str,
) -> Option<ResumeSource<'a>> {
    if !resume_id.trim().is_empty() {
        return Some(ResumeSource::Store(resume_id));
    }
    (!resume_text.trim().is_empty()).then_some(ResumeSource::Text(resume_text))
}

/// Char cap on a request-supplied id echoed into a validation-error message
/// (`"resume not found: …"` / `"job not found in cache: …"`, both renderer-
/// visible). The same rule, for the same reason ("every real id is a short
/// token, so an oversized one reaching a formatter is hostile"), the
/// now-deleted `agent::tools::ECHO_CAP`/`clamped_echo` used to enforce —
/// reimplemented locally rather than imported from `crate::prompt_fence`
/// (PR-5 step 1's extraction target): `ECHO_CAP`/`clamped_echo` were never
/// part of that move (verified: no surviving caller outside the deleted
/// `agent` module ever needed them), so this one-line reimplementation is the
/// permanent shape, not a stopgap. [`JOB_IDENTITY_CAP`] already bounds
/// `resume_id`/`job_id` for STORAGE (512 bytes); this is the tighter,
/// separate bound for what a caller actually SEES echoed back.
const ECHO_CHARS_CAP: usize = 64;

/// Clamp an id for the error message it is about to be formatted into,
/// char-boundary safe. See [`ECHO_CHARS_CAP`].
fn echoed(id: &str) -> String {
    id.chars().take(ECHO_CHARS_CAP).collect()
}

/// Resolve one run's résumé TEXT from an already-decided [`ResumeSource`] —
/// the ID-WINS rule's own behavior, not just the source it picked. `lookup`
/// is the `DocumentStore` read, injected so this is provable without an
/// `AppHandle`: the `Store` arm has no `resume_text` (or anything else) in
/// scope to fall back to on a miss, only `id` and `lookup`, so `lookup =
/// |_| None` against a `Store` choice MUST return `Err` — there is
/// structurally nothing else it could return, unlike an inline branch beside
/// a `clamped.resume_text` a future edit could reach for.
pub(crate) fn resolve_resume(
    choice: ResumeSource<'_>,
    lookup: impl Fn(&str) -> Option<String>,
) -> AppResult<String> {
    match choice {
        ResumeSource::Store(id) => lookup(id)
            .ok_or_else(|| AppError::Validation(format!("resume not found: {}", echoed(id)))),
        ResumeSource::Text(text) => Ok(text.to_string()),
    }
}

/// The job-ad twin of [`ResumeSource`] — same ID-WINS rule, over `jobId`
/// (the live postings cache) and `jobAdText`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobSource<'a> {
    /// A nonempty `jobId` — look this up in the postings cache; a miss is a
    /// HARD ERROR, never a fallback to `jobAdText`.
    Cache(&'a str),
    /// `jobId` was empty and `jobAdText` was not.
    Text(&'a str),
}

pub(crate) fn job_source<'a>(job_id: &'a str, job_ad_text: &'a str) -> Option<JobSource<'a>> {
    if !job_id.trim().is_empty() {
        return Some(JobSource::Cache(job_id));
    }
    (!job_ad_text.trim().is_empty()).then_some(JobSource::Text(job_ad_text))
}

/// The job-ad twin of [`resolve_resume`] — same structural close: the
/// `Cache` arm's error path has only `id` and the two injected lookups in
/// scope, never `job_ad_text`, so a miss can only ever error. `meta_from_request`
/// is called ONLY on the `Text` arm (it is cheap and pure, but calling it on
/// the `Cache` arm too would be the exact "did the branch actually pick a
/// side" bug this split exists to make unreachable).
pub(crate) fn resolve_job(
    choice: JobSource<'_>,
    lookup_text: impl Fn(&str) -> Option<String>,
    lookup_meta: impl Fn(&str) -> Option<crate::commands::match_resume::JobPostingMeta>,
    meta_from_request: impl FnOnce() -> crate::commands::match_resume::JobPostingMeta,
) -> AppResult<(String, crate::commands::match_resume::JobPostingMeta)> {
    match choice {
        JobSource::Cache(id) => {
            let job_ad = lookup_text(id).ok_or_else(|| {
                AppError::Validation(format!("job not found in cache: {}", echoed(id)))
            })?;
            let meta = lookup_meta(id).unwrap_or_default();
            Ok((job_ad, meta))
        }
        JobSource::Text(text) => Ok((text.to_string(), meta_from_request())),
    }
}

/// The posting identity `execute` builds on the TEXT path, where there is no
/// cached posting for `job_meta_for` to read. Pure so the field mapping is a
/// test rather than a claim.
pub(crate) fn job_meta_from_request(
    clamped: &ClampedRequest,
) -> crate::commands::match_resume::JobPostingMeta {
    crate::commands::match_resume::JobPostingMeta {
        company: clamped.company_name.clone(),
        title: clamped.job_title.clone(),
        url: clamped.job_url.clone(),
        board: clamped.board.clone(),
    }
}

/// What `persist_document` (`mod.rs`) should write into the aggregate's
/// `job_ad` column. `Cache` stays empty — deliberately, per that fn's own doc:
/// the postings cache is keyed by the live job id, which a finished run no
/// longer holds, so persisting it here would not fix the staleness it already
/// documents. `Text` DOES have the text this run was actually built from, so
/// it persists it — `merge_application`'s `pick` keeps the existing value
/// when this is empty, so the `Cache` arm's empty string is a no-op on an
/// existing aggregate, not an overwrite.
///
/// **The aggregate's OWN storage key is a separate concern this fn does not
/// touch, worth being explicit about.** `AiGenerationRecord.job_url` (what
/// `save_application` upserts BY) is, on the `Cache` path, a url `execute`
/// resolved SERVER-side from the postings cache; on the `Text` path it is
/// `job_meta_from_request`'s `meta.url`, which is just `clamped.jobUrl` — the
/// RENDERER's own claim, unverified against anything. A text-path run whose
/// `jobUrl` happens to match an existing posting therefore merges onto that
/// posting's aggregate rather than starting a fresh one. Not a security
/// boundary (single local user, and `is_persistable` still refuses to
/// overwrite a real work history with an empty one) — but a real
/// data-integrity surprise for a caller that sends a `jobUrl` it does not
/// actually own.
///
/// [`unlinked_run_key`] is a DIFFERENT, narrower key for a DIFFERENT table —
/// the run STORE's own retention partition, never this aggregate — and the
/// two must not be conflated: `execute` computes `unlinked_run_key` only for
/// the run row it writes, while this fn (and `meta.url` above) keep governing
/// the aggregate exactly as they did before PR-3.
pub(crate) fn job_ad_for_persist(source: JobSource<'_>) -> String {
    match source {
        JobSource::Cache(_) => String::new(),
        JobSource::Text(text) => text.to_string(),
    }
}

/// The run's `metrics_json.sourceResumeId` value — `Some` ONLY on the
/// [`ResumeSource::Store`] path (see `execute`'s doc: an id is content-free
/// per ADR-027, and it is the only thing `source_resume_for`'s later
/// provenance fallback can key on). Pure so "a text-path run never carries
/// an id here" is a test on the decision itself, not a substring match over
/// `execute`'s source.
pub(crate) fn source_resume_id_for_metrics<'a>(choice: ResumeSource<'a>) -> Option<&'a str> {
    match choice {
        ResumeSource::Store(id) => Some(id),
        ResumeSource::Text(_) => None,
    }
}

/// A stable, non-real "posting url" for an UNLINKED run's **run-row**
/// `job_url` — never handed to the `ai_generations` aggregate
/// (`persist_document`'s own `job_ad_for_persist`/`meta.url` plumbing is
/// untouched by this; the aggregate still gets exactly what it always did,
/// empty when there is truly no posting url).
///
/// **Why this exists.** `PipelineRunStore::prune` partitions retention on
/// `(job_url, kind)`, and `upsert_run` stores EVERY unlinked run under the
/// same `job_url = ""` today — harmless while an unlinked run was rare (a
/// cache entry with no `url` field), but the text path makes it routine (a
/// pasted job ad, or an Autopilot found job with no capturable link), so
/// every pasted posting's "resume" runs now pool into ONE shared bucket: a
/// 4th pasted job's first run evicts an unrelated pasted job's history.
/// Keying on a hash of the job-ad text scopes retention back to ONE posting
/// — repeats of the SAME pasted text land in the same bucket (so re-running
/// it still caps at [`RETENTION_RUNS_PER_JOB`](crate::pipeline::runs::RETENTION_RUNS_PER_JOB)),
/// while two DIFFERENT pasted postings get two different buckets.
///
/// **Why `https://…/.invalid`, not a bare token or a custom scheme.**
/// `PipelineRunStore::upsert_run` normalizes every `job_url` through
/// `applications::normalize_job_url` before it is stored — the SAME
/// chokepoint that neutralizes any non-`http(s)` scheme back to `""`
/// (`javascript:`, `data:`, a bare opaque id with no scheme at all falls
/// through as a bare "host" but a colon-bearing token like a hex hash
/// prefixed by anything scheme-shaped would trip the same guard). An
/// `http(s)` url is the only shape that survives that seam intact, so this
/// borrows the IANA-reserved `.invalid` TLD (RFC 2606 — guaranteed never to
/// resolve to a real host) rather than inventing a URL that could be
/// mistaken for a live posting if it ever reached a "view original
/// posting" affordance. It never does today: the renderer queries run
/// history by the APPLICATION's own `jobUrl` (a real, possibly-empty
/// value it already holds), never by anything this function returns, and
/// `runs_for_job("")`/`find_by_job_url("")` both short-circuit before
/// touching a table — so a caller holding the real empty url can never
/// accidentally fetch a row keyed by this synthetic one.
///
/// Deterministic and non-cryptographic (FNV-1a) on purpose: this is a
/// storage partition key, not a security boundary — the reserved TLD and
/// the fact it never reaches a click target are what keep it from being
/// mistaken for a real link, not the hash algorithm.
pub(crate) fn unlinked_run_key(job_ad_text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for byte in job_ad_text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
    }
    format!("https://unlinked.ajh.invalid/{hash:016x}")
}

/// `execute`'s `RunRow.job_url` — MAY differ from the aggregate's own
/// `job_url` (`persist_document`'s; entirely untouched by this fn). `job_url`
/// (the aggregate's key, computed the same way it always was) wins whenever
/// it is nonempty — the ordinary, linked case. Only when it is EMPTY AND the
/// run took the [`JobSource::Text`] path does this substitute
/// [`unlinked_run_key`] — see that fn's doc for the full "why" (retention
/// pooling). A `Cache` run whose cached posting itself carried no url (rare,
/// pre-existing, not part of PR-3's text-path regression) is left at `""`,
/// unchanged.
pub(crate) fn run_store_job_url(job_url: &str, choice: JobSource<'_>) -> String {
    if !job_url.trim().is_empty() {
        return job_url.to_string();
    }
    match choice {
        JobSource::Text(text) => unlinked_run_key(text),
        JobSource::Cache(_) => job_url.to_string(),
    }
}
