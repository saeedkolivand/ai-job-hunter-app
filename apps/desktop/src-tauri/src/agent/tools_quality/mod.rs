//! Résumé-quality Read tools for the agent registry: `validate_resume`,
//! `search_candidate_evidence`, `lookup_salary`, `get_trim_suggestions`.
//!
//! Split out of [`super::tools`] purely to stay under the R8 module-size cap
//! (`docs/architecture-rules.md`) — this is NOT a second registry. Every
//! handler here is a thin adapter over an existing pure module
//! (`validate::content`, `documents::evidence`, `salary_research`) or Tauri
//! command (`commands::ai_salary::ai_lookup_salary_reasoned` — the core
//! `commands::ai::ai_lookup_salary` itself delegates to); no business logic
//! is duplicated
//! (`docs/knowledge/automation-domain.md`'s zero-change-abstraction rule).
//! [`quality_tools`] is appended to [`super::tools::read_tools`]'s `Vec`, so
//! every per-flow whitelist still comes from ONE call.
//!
//! SECURITY (same trust story as `super::tools`): the SOURCE résumé is always
//! loaded server-side via the trusted [`ToolContext::resume_id`], never a
//! model-supplied `resumeId` arg — a prompt-injected job posting can't
//! substitute a different candidate's document into a factual check. Every
//! summary returned here quotes text drawn from the untrusted résumé/job
//! posting (evidence spans, bullet text, issue messages/sections), so EVERY
//! per-field cap below ([`clamp_chars`] and the [`EVIDENCE_CAP`]/
//! [`MESSAGE_CAP`]/[`SECTION_CAP`]/[`BULLET_TEXT_CAP`] caps it backs, plus the
//! [`MAX_ISSUES`]/[`MAX_SKILLS`] COUNT caps) bounds it before it re-enters the
//! transcript through [`neutralized_summary`], which makes every quoted span
//! inert as a transcript boundary via the SAME
//! [`crate::prompt_fence::neutralize_transcript_boundaries`] pass
//! [`crate::prompt_fence::fenced`] runs on untrusted input — so a forged fence tag or
//! `[tool_result:…]` marker smuggled inside a quoted span can't masquerade as
//! a `<job_posting>`/`<candidate_resume>` boundary or a second tool verdict.
//! `lookup_salary` is the one exception: its result carries no free text (see
//! `salary_range_serializes_to_only_known_numeric_and_currency_fields`), so
//! it skips neutralization and uses the plain [`envelope_result`] wrapper.
//!
//! **The clamp/fence primitives here are `pub(super)` and SHARED with
//! [`super::tools_pipeline`]** (Phase 3's `analyze_job` / `get_quality_report`
//! / `run_quality_pipeline`), which quotes the same untrusted résumé/posting/
//! report text into the same transcript. `clamp_chars`/[`clamp_evidence`]/
//! [`clamped_resume_text`]/[`clamped_job_text`]/[`shrink_to_summary_cap`]/
//! [`neutralized_summary`] and the caps behind them are widened rather than
//! copied for the same ADR-010 reason [`fenced`] itself is: a second copy of a
//! bound is where the two drift, and a summary that skipped this module's
//! measure-then-drop loop would hand the hard `chars().take(cap)` a JSON body
//! to cut mid-string.
//!
//! LOW fix, PR #963 round 9 — these summaries used to be `<validate_resume_
//! result>`-style tag-WRAPPED as well ([`fenced`]). That wrap was provably
//! dead work: those three tags are registered in
//! `crate::prompt_fence::FENCE_TAG_PATTERNS`, and
//! [`crate::agent::controller::tool_result_fence`] runs the same
//! neutralization over EVERY tool result body on its way into the transcript
//! — so the wrapper this module added was broken open again one layer up
//! (`< validate_resume_result>`) before the model ever saw it. The model got
//! a mangled tag and the transcript got no boundary; only the INTERIOR
//! neutralization ever did any work, and that is what survives here. The tags
//! stay registered on purpose: with no legitimate producer left, ANY
//! occurrence of one is a forgery, and breaking it is now unambiguously
//! correct (pinned by `prompt_fence::test`'s
//! `fenced_neutralizes_a_forged_validate_resume_result_tag_inside_a_job_posting_body`).
//!
//! [`fenced`]: crate::prompt_fence::fenced

use std::future::Future;
use std::pin::Pin;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::commands::ai_salary::SalaryLookupReason;
use crate::commands::match_resume::{job_meta_for, job_text_for, JobPostingMeta};
use crate::documents::evidence::{extract_evidence, rank_bullets, EvidenceBullet, EvidenceSet};
use crate::documents::keywords::detect_locale_tag;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::salary_research::SalaryRange;
use crate::validate::content::{
    validate_content, ContentInput, ContentIssue, ContentReport, DocKind,
};
use crate::validate::Severity;

use super::tools::{clamped_echo, AgentTool, ToolContext, ToolKind};
use crate::prompt_fence::{neutralize_transcript_boundaries, JOB_CAP, RESUME_CAP};

// ── Shared caps ────────────────────────────────────────────────────────────

/// Cap on the `query` arg to `search_candidate_evidence` — sized for a short
/// skill/requirement phrase, not a pasted paragraph. Mirrors
/// `salary_research::MAX_INPUT_CHARS`'s "bound what a caller can inflate a
/// downstream operation with" rationale.
const QUERY_CAP: usize = 200;

/// Cap on each evidence/bullet-text span embedded in a compact tool summary —
/// these quote the résumé/job/generated text directly (not the model's own
/// words), so a validator that found many issues can't blow the tool-result
/// budget the way returning the FULL [`ContentReport`] would. The full report
/// (and its longer spans) is the quality-report panel's job, not this tool's.
/// Also reused ([`clamp_evidence`]) for a bullet's `hits` entries and
/// `compact_evidence_set`'s `skillsPresent`/`skillsAbsent` entries — same
/// "quoted, untrusted, job/résumé-derived token" shape (MEDIUM fix, PR #963
/// round 4: `documents::keywords::keywords_normalized` only filters on
/// `len() > 3`, so nothing upstream bounds how long a single keyword can be).
pub(super) const EVIDENCE_CAP: usize = 80;

/// Cap on a validator issue's `message` in the compact summary — longer than
/// [`EVIDENCE_CAP`] since guidance prose reads longer than a quoted span, but
/// still bounded so a crafted draft that trips many long-message issues can't
/// blow the tool-result budget.
pub(super) const MESSAGE_CAP: usize = 400;

/// Cap on a validator issue's `section` in the compact summary — section
/// names are short labels ("Experience", "Skills"), never free-flowing text;
/// a defensive backstop, not an expected truncation point.
pub(super) const SECTION_CAP: usize = 80;

/// Cap on a bullet's quoted `text` in a compact tool summary — same
/// rationale as [`EVIDENCE_CAP`]: it quotes untrusted résumé text directly.
const BULLET_TEXT_CAP: usize = 200;

/// Max issues surfaced in `validate_resume`'s compact summary. A crafted
/// draft that trips dozens of checks must not blow the tool-result budget —
/// dropped issues are counted in the summary's `truncated` field, never a
/// mid-string cut of the issue list.
pub(super) const MAX_ISSUES: usize = 20;

/// Max entries kept in `skillsPresent`/`skillsAbsent` — a résumé with an
/// unusually long skills section must not blow the tool-result budget
/// either. Drops past this cap are reported via `compact_evidence_set`'s
/// `skillsTruncated` field (LOW fix, PR #963 round 7) — unlike
/// [`MAX_HITS`] below, this cap gates `skillsAbsent`, the GAP LIST the
/// agent works from, so a silent drop actively misleads it.
///
/// **CROSS-MODULE INVARIANT — this cap is only honest because the producer
/// orders by relevance.** `documents::evidence::extract_evidence` sorts
/// `skills_present`/`skills_absent` by how often the POSTING states each term
/// (alphabetical only as a deterministic tiebreak), so `.take(MAX_SKILLS)`
/// below is the top-N by relevance BY CONSTRUCTION and needs no re-ranking
/// here. That ordering is load-bearing, not incidental: it used to be purely
/// alphabetical (fixed in `595fa055`, round-8 follow-up), which handed the
/// agent the a–… PREFIX of the gap list — `ansible` kept, `terraform` cut — a
/// bias `skillsTruncated` (a count) cannot reveal. Reordering the producer
/// would silently re-bias this cap with nothing failing here: [`EvidenceSet`]
/// exposes only `Vec<String>` display forms, and for `skills_absent` no
/// bullet hit exists by definition (that is what "absent" means), so this
/// module has no signal of its own to sort on and could not detect the
/// regression. Re-ranking here is also forbidden outright by the module doc
/// (it would be a second, drifting keyword heuristic) — the fix always
/// belongs at the producer.
const MAX_SKILLS: usize = 15;

/// Max entries kept in one bullet's `hits` (job-derived keyword matches) —
/// analogous to [`MAX_SKILLS`], scoped to a single bullet instead of the
/// résumé's whole skills section: a keyword-dense job posting must not blow
/// the tool-result budget either (MEDIUM fix, PR #963 round 4). Deliberately
/// NOT paired with a `hitsTruncated`-style field (PR #963 round 7 review):
/// `hits` is corroborating evidence for a bullet the model already sees in
/// full, not a gap list it's meant to act on exhaustively like
/// `skillsAbsent` — a dropped hit here doesn't mislead the way a silently
/// dropped skill does, so [`MAX_SKILLS`]'s new `skillsTruncated` signal
/// isn't warranted here too.
const MAX_HITS: usize = MAX_SKILLS;

/// How many bullets `search_candidate_evidence` returns — the strongest
/// dozen is plenty for the model to ground a claim in; a résumé with many
/// roles could otherwise return dozens of lines.
const EVIDENCE_SEARCH_LIMIT: usize = 12;

/// How many bullets `get_trim_suggestions` returns — the weakest ~10 is what
/// a trim conversation actually needs; the full ranking is the trim panel's
/// job.
const TRIM_SUGGESTIONS_LIMIT: usize = 10;

/// Worst-case RAW (pre-serialization) chars ONE compact issue's clamped
/// fields can contribute — [`SECTION_CAP`] + [`MESSAGE_CAP`] + [`EVIDENCE_CAP`],
/// plus 40 chars of headroom for `code` (the longest registered
/// [`crate::validate::content::CONTENT_ISSUE_CODES`] entry today,
/// `consistency.skill_not_demonstrated`, is 34), plus 8 chars for `severity`
/// (round-12 addition — its value is never clamped because it can only ever
/// be one of [`Severity`]'s two lowercase wire words, and `"critical"`, the
/// longer of the two, is 8 raw chars), plus 70 chars for the object's own
/// JSON syntax (keys/quotes/colons/commas/braces — measured at ~66 with
/// `severity` added: `{"code":"","section":"","message":"","evidence":"",
/// "severity":""},`; was ~52 before round 12).
///
/// A sizing dial for [`SUMMARY_CAP`] — NOT a guarantee the real SERIALIZED
/// body stays under it (MEDIUM fix, PR #963 round 4): JSON escaping (a `"`
/// becomes `\"`, a raw control char becomes `\u00XX`) can inflate a clamped
/// field's serialized size well past its raw-char cap, and `duplicates.rs`
/// quotes untrusted bullet text verbatim into `message`, so a quote-heavy
/// draft can push a real issue past this "worst case" even though every
/// field is within its char cap. That's exactly why [`compact_content_report`]
/// no longer trusts this arithmetic to hold on its own — it measures the
/// ACTUAL serialized length and drops whole issues (into `truncated`) until
/// the body fits [`SUMMARY_CAP`], rather than relying on
/// [`neutralized_summary`]'s hard `body.chars().take(cap)` as the enforcement
/// point.
const PER_ISSUE_WORST_CASE: usize = SECTION_CAP + MESSAGE_CAP + EVIDENCE_CAP + 40 + 8 + 70;

/// Target ceiling on a tool-result summary's SERIALIZED body —
/// `MAX_ISSUES` issues at [`PER_ISSUE_WORST_CASE`] each (a plain-ASCII,
/// non-escaping estimate), plus 500 chars of headroom for the summary's own
/// envelope (`ok`/`criticals`/`warnings`/`truncated` fields, the `issues`
/// array's brackets — measured at ~70).
///
/// [`compact_content_report`], [`compact_evidence_set`], and
/// [`compact_trim_suggestions`] all ENFORCE this by measurement, not by
/// trusting a raw-char estimate, via the shared [`shrink_to_summary_cap`]
/// helper: each serializes its candidate summary and, if it's over
/// `SUMMARY_CAP`, drops the weakest (last-ordered) whole item and
/// re-checks, repeating until the body fits or every item is gone. Dropped
/// items are counted in `truncated`, never a mid-string cut.
///
/// MEDIUM fix, PR #963 round 5: the round-4 fix below only wired this
/// measure-then-drop loop into [`compact_content_report`] —
/// [`compact_evidence_set`] and [`compact_trim_suggestions`] still built
/// their bullet lists unconditionally and handed the result straight to the
/// hard char cap. Both overflow `SUMMARY_CAP` on their own
/// declared per-field worst case BEFORE any JSON escaping even applies:
/// `EVIDENCE_SEARCH_LIMIT` (12) bullets at up to `BULLET_TEXT_CAP +
/// MAX_HITS * EVIDENCE_CAP` (~1,495) raw chars each, plus two
/// `MAX_SKILLS`-entry skill lists, is ~20k chars; `TRIM_SUGGESTIONS_LIMIT`
/// (10) bullets alone is ~15k — both well past the ~13.7k this constant
/// works out to.
///
/// This used to just reuse [`RESUME_CAP`]'s magnitude (8,000) on the theory
/// that a "handful of items" summary could never approach it (PR #963 round
/// 3 fixed that by deriving the bigger number above) — but even that fix
/// still assumed a clamped field's SERIALIZED size never exceeds its
/// raw-char cap, which [`PER_ISSUE_WORST_CASE`]'s doc explains is false
/// under JSON escaping. `summary_cap_holds_the_real_worst_case_without_truncating_the_json`
/// pins the plain-ASCII worst case (zero issues dropped);
/// `compact_content_report_drops_whole_issues_instead_of_cutting_escaped_json_mid_string`
/// pins the quote-heavy case that broke the old char-truncation approach.
const SUMMARY_CAP: usize = MAX_ISSUES * PER_ISSUE_WORST_CASE + 500;

// ── Pure arg parsing (unit-testable without an AppHandle) ───────────────────

/// Trim + clamp an optional `draft` arg to [`RESUME_CAP`] chars, returning
/// `(clamped, was_truncated)`. An absent/empty draft is a valid "use the
/// saved résumé instead" case for BOTH `validate_resume` and
/// `get_trim_suggestions` — never an error; each handler falls back to the
/// candidate's saved résumé (M-5: this also means the model is never FORCED
/// to echo the whole draft as tool-call arguments just to run a sanity check
/// on the résumé it already has).
///
/// MEDIUM fix, PR #963 round 8: the clamp itself was silent. `save_resume`
/// accepts [`super::tools::SAVED_RESUME_CAP`] (40,000) chars, so a draft up
/// to 5× this cap can reach the save path while `validate_resume` inspected
/// only its first `RESUME_CAP` chars — and then reported `ok: true`,
/// `criticals: 0` for a document it had mostly never read. The second
/// return value is what [`compact_content_report`] turns into a
/// `draftTruncated` flag forcing `ok: false`; see
/// [`validate_resume_core`]'s doc for why the cap is NOT simply raised to
/// `SAVED_RESUME_CAP` instead.
fn optional_draft_arg(args: &Value) -> (String, bool) {
    let draft = args
        .get("draft")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    // `nth` short-circuits at the cap — never a full O(len) count of a
    // model-supplied blob just to answer "is it longer than 8,000?".
    let truncated = draft.chars().nth(RESUME_CAP).is_some();
    (draft.chars().take(RESUME_CAP).collect(), truncated)
}

/// Trim + clamp an optional `query` arg to [`QUERY_CAP`] chars. An empty
/// query is valid — `search_candidate_evidence` then searches against this
/// run's own job posting instead.
fn optional_query_arg(args: &Value) -> String {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    query.chars().take(QUERY_CAP).collect()
}

/// Resolve `validate_resume`'s optional `docKind` arg (MEDIUM fix, PR #963
/// round 9). The tool used to hardcode [`DocKind::Resume`], so an agent
/// checking the drafted COVER LETTER — which the prep flow drafts BEFORE the
/// résumé, and can now spend an optional call on (see
/// `agent::flows::PREP_APPLICATION_SYSTEM`) — had it scored against the
/// résumé ruleset: `factual`/`alignment`/`consistency`/`ats`/`duplicates`
/// checks that assume sections, roles and bullets, none of which a letter
/// has.
///
/// Absent/null/empty defaults to `resume` (the overwhelmingly common case,
/// and the pre-existing behavior). Anything else must be exactly one of the
/// two `DocKind` wire values — `resume` | `coverLetter`, its `camelCase`
/// serde rename — and an unrecognized value is REFUSED, never silently
/// degraded to the more common kind: the same rule, for the same reason, as
/// `commands::resume::resume_validate_content`'s wire-form match (a caller
/// bug must not have its document validated against the wrong ruleset
/// unnoticed). The offending value is echoed back clamped to
/// [`EVIDENCE_CAP`] — it is model-supplied, so it is bounded like every other
/// untrusted span this module quotes.
fn doc_kind_arg(args: &Value) -> AppResult<DocKind> {
    let raw = match args.get("docKind") {
        None | Some(Value::Null) => return Ok(DocKind::Resume),
        Some(Value::String(s)) => s.trim().to_string(),
        // A non-string (number, object, array) is a caller bug, not a kind.
        Some(other) => other.to_string(),
    };
    match raw.as_str() {
        "" | "resume" => Ok(DocKind::Resume),
        "coverLetter" => Ok(DocKind::CoverLetter),
        other => Err(AppError::Validation(format!(
            "validate_resume: unknown docKind {:?}, expected \"resume\" or \"coverLetter\"",
            clamp_evidence(other)
        ))),
    }
}

// ── Compact, clamped summaries (unit-testable without an AppHandle) ─────────

/// Cap `s` to `cap` chars, char-boundary safe — the one clamp primitive
/// every per-field cap in this module reuses ([`clamp_evidence`], and the
/// `message`/`section`/bullet-`text` clamps in [`compact_content_report`]/
/// [`bullet_to_value`]), so every clamp behaves identically.
pub(super) fn clamp_chars(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

/// Cap `s` to [`EVIDENCE_CAP`] chars, char-boundary safe.
pub(super) fn clamp_evidence(s: &str) -> String {
    clamp_chars(s, EVIDENCE_CAP)
}

/// Clamp the candidate's server-loaded résumé text to [`RESUME_CAP`] — the
/// SAME cap `super::tools::grounded_user_msg` clamps the model's OWN view of
/// the résumé to before a drafting tool ever runs. Every handler in this
/// module that reads the résumé (directly, or as the fallback when no
/// `draft`/query text was supplied) routes through here, for two reasons:
///
/// 1. **Correctness (HIGH)** — `validate_resume` used to compare a
///    `RESUME_CAP`-clamped model draft against the FULL, unclamped stored
///    résumé; a role starting past the cap fired a false `factual.dropped_role`
///    Critical for a role the drafting tool was never shown. Clamping BOTH
///    sides to the same cap keeps them looking at the same universe.
/// 2. **Perf (MEDIUM)** — a server-loaded résumé is otherwise unbounded, and
///    feeds a CPU-bound analysis pass (`validate_content`/`extract_evidence`/
///    `rank_bullets`) inline on the tokio runtime.
pub(super) fn clamped_resume_text(text: &str) -> String {
    clamp_chars(text, RESUME_CAP)
}

/// Clamp a cached job posting's text to [`JOB_CAP`] for the same perf reason
/// as [`clamped_resume_text`] — scraped posting text is unbounded too.
/// Mirrors `super::tools::research_company_handler`'s own
/// `.chars().take(JOB_CAP)` clamp.
pub(super) fn clamped_job_text(text: &str) -> String {
    clamp_chars(text, JOB_CAP)
}

/// Shrink an already-ordered set of `len` items to fit [`SUMMARY_CAP`] once
/// JSON-serialized, dropping the LAST item at a time and re-measuring — the
/// measure-the-real-serialized-length-then-drop-a-whole-item loop PR #963
/// round 4 introduced for [`compact_content_report`], generalized (round 5)
/// so [`compact_evidence_set`] and [`compact_trim_suggestions`] enforce the
/// SAME budget the same way: per-field char clamps alone don't bound a JSON
/// candidate's SERIALIZED size (JSON escaping) OR its size once multiplied
/// by item count (see [`SUMMARY_CAP`]'s doc) — either way, leaving
/// [`neutralized_summary`]'s hard `body.chars().take(cap)` as the enforcement
/// point cuts the JSON body mid-string.
///
/// `build(kept)` renders the FULL candidate `Value` (envelope + the first
/// `kept` of the caller's already-ordered items, including whatever
/// `truncated` bookkeeping the envelope carries) for a given `kept`. The
/// caller's own ordering decides what dropping the tail means:
/// [`compact_content_report`] sorts Criticals first — dropping the tail
/// drops the weakest Warnings, never a Critical; [`compact_evidence_set`]
/// sorts strongest-first — dropping the tail drops the weakest bullets;
/// [`compact_trim_suggestions`]'s bullets are already weakest-first — this
/// drops from the end of that order, i.e. the least-weak of the selected
/// set, keeping the weakest (most actionable) suggestions.
pub(super) fn shrink_to_summary_cap(len: usize, mut build: impl FnMut(usize) -> Value) -> Value {
    let mut kept = len;
    loop {
        let candidate = build(kept);
        // Measure the ACTUAL serialized length — see this fn's doc for why
        // per-field char clamps and item-count arithmetic alone don't bound it.
        let fits = serde_json::to_string(&candidate)
            .map(|s| s.chars().count() <= SUMMARY_CAP)
            .unwrap_or(false);
        if fits || kept == 0 {
            return candidate;
        }
        kept -= 1;
    }
}

/// Compact a [`ContentReport`] into what `validate_resume` actually returns
/// to the model: counts, plus up to [`MAX_ISSUES`] issues (each
/// severity/code/section/message/evidence field individually clamped or, for
/// `severity`, inherently bounded to one of two words — see the module
/// SECURITY note), plus a `truncated` count for anything dropped past that
/// cap OR past [`SUMMARY_CAP`]'s serialized-length budget (enforced by
/// [`shrink_to_summary_cap`] — see its doc and [`SUMMARY_CAP`]'s). Never a
/// mid-string cut of the issue list — a whole issue is dropped instead, one
/// at a time, until the summary actually fits. The full report — every
/// [`crate::validate::content::ContentMetrics`] field, every issue, uncapped
/// spans — is the quality-report panel's job, not this tool's.
///
/// Issues are ordered **Criticals first** before either cap is applied. The
/// validator emits in check order, not severity order, so `ats.header_in_body`
/// (emitted near the end) fell off the list on a draft that tripped 20+ earlier
/// Warnings — the model then saw `criticals: 1` with no Critical it could act on.
/// The sort is stable, so within each severity the emission order is
/// preserved; dropping from the END of the sorted, already-capped list means
/// a Critical is the last thing this function ever drops.
///
/// MEDIUM fix, PR #963 round 12: each issue now also carries its own
/// `severity` — the `criticals`/`warnings` COUNTS above already told the
/// model how many Criticals existed, but nothing in the per-issue objects
/// said WHICH of the listed issues those were. `agent::flows::
/// PREP_APPLICATION_SYSTEM` step 6 tells the model to fix a résumé draft
/// "if ok is false or criticals is above 0" — with only a count and no
/// per-issue marker, a model reading a Warnings-and-Criticals-mixed list had
/// to guess which entries were the ones the count referred to. Sourced
/// straight from [`ContentIssue::severity`] via `Severity`'s own `Serialize`
/// impl, so the wire word can never drift from the same "critical"/"warning"
/// spelling every other consumer of `Severity` sees.
///
/// MEDIUM fix, PR #963 round 8: `draft_truncated` (from
/// [`optional_draft_arg`]) says the model's `draft` argument was LONGER than
/// the [`RESUME_CAP`] slice actually validated. A clean bill of health on
/// 8,000 of a 40,000-char draft is not a clean bill of health, so it is
/// surfaced as its own `draftTruncated` field AND forces `ok: false` — the
/// counts stay honest about what was inspected (`criticals` still counts
/// only real findings, never a fabricated one), while `ok` stops asserting
/// something this report cannot know. Built INSIDE the candidate so
/// [`shrink_to_summary_cap`] measures the field like every other.
fn compact_content_report(report: &ContentReport, draft_truncated: bool) -> Value {
    let criticals = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .count();
    let warnings = report.issues.len() - criticals;
    let mut ordered: Vec<&ContentIssue> = report.issues.iter().collect();
    // `false < true`: Criticals sort ahead of everything else.
    ordered.sort_by_key(|i| i.severity != Severity::Critical);
    let cap_count = MAX_ISSUES.min(ordered.len());
    shrink_to_summary_cap(cap_count, |kept| {
        let issues: Vec<Value> = ordered
            .iter()
            .take(kept)
            .map(|i| {
                json!({
                    "code": i.code,
                    // MEDIUM fix, PR #963 round 12: the flow prompt (step 6,
                    // `agent::flows::PREP_APPLICATION_SYSTEM`) tells the model to
                    // "fix Criticals" but this summary gave it a `criticals` COUNT
                    // with no way to tell which of the listed issues those were —
                    // `i.severity` reuses `Severity`'s own `Serialize` impl, so the
                    // wire word (`"critical"`/`"warning"`) is always the SAME one
                    // `Severity`'s `#[serde(rename_all = "lowercase")]` produces
                    // everywhere else it crosses the wire (`ExportIssue`, the
                    // quality-report panel) — no second, driftable string mapping.
                    "severity": i.severity,
                    "section": i.section.as_deref().map(|s| clamp_chars(s, SECTION_CAP)),
                    "message": clamp_chars(&i.message, MESSAGE_CAP),
                    "evidence": i.evidence.as_deref().map(clamp_evidence),
                })
            })
            .collect();
        json!({
            "ok": report.ok && !draft_truncated,
            "criticals": criticals,
            "warnings": warnings,
            "draftTruncated": draft_truncated,
            "truncated": report.issues.len() - kept,
            "issues": issues,
        })
    })
}

/// Quotes untrusted bullet `text` — clamped to [`BULLET_TEXT_CAP`] for the
/// same reason as [`EVIDENCE_CAP`]. `hits` (job-derived keyword matches the
/// bullet scored against) is untrusted too and was serialized unclamped in
/// both per-entry length AND count (MEDIUM fix, PR #963 round 4) — clamped
/// here the same way, entry-for-entry, as [`compact_evidence_set`]'s skills
/// lists.
fn bullet_to_value(b: &EvidenceBullet) -> Value {
    let hits: Vec<String> = b
        .hits
        .iter()
        .take(MAX_HITS)
        .map(|h| clamp_evidence(h))
        .collect();
    json!({
        "id": b.id,
        "text": clamp_chars(&b.text, BULLET_TEXT_CAP),
        "hits": hits,
        "score": b.score,
    })
}

/// Flatten every scored bullet in `set` (experience roles + projects) into
/// one list, strongest-first, capped to `limit`. Skills lists are capped to
/// [`MAX_SKILLS`] entries each, and — MEDIUM fix, PR #963 round 4 — each
/// entry is itself clamped to [`EVIDENCE_CAP`] chars (nothing upstream
/// bounds an individual skill/keyword's length, only the count). The
/// résumé's own STRUCTURE (roles, education) is the quality-report panel's
/// job, not a tool result.
///
/// MEDIUM fix, PR #963 round 5: `limit` (up to [`EVIDENCE_SEARCH_LIMIT`])
/// strongest bullets, each up to `BULLET_TEXT_CAP + MAX_HITS * EVIDENCE_CAP`
/// raw chars, is already over [`SUMMARY_CAP`] on its own declared worst case
/// before JSON escaping is even in play (see [`SUMMARY_CAP`]'s doc) — routed
/// through [`shrink_to_summary_cap`] the same way [`compact_content_report`]
/// is, so a bullet-heavy résumé drops whole bullets (weakest-last — the
/// list is already sorted strongest-first, so dropping the tail drops the
/// weakest) into `truncated` instead of getting cut mid-string by the
/// hard char cap. Skills lists are NOT part of this drop loop:
/// their own `MAX_SKILLS`/`EVIDENCE_CAP` caps alone already bound them well
/// under `SUMMARY_CAP`, so dropping bullets is always enough.
///
/// MEDIUM fix, PR #963 round 6: `truncated` used to be `capped.len() - kept`
/// — `capped` is already POST-`.take(limit)`, so that arithmetic could only
/// ever count drops from the [`shrink_to_summary_cap`] loop, never the
/// initial `.take(limit)` cap itself. A résumé with more than `limit`
/// scored bullets silently withheld the rest while reporting `truncated: 0`,
/// defeating the whole "tell the model the summary is partial" point of the
/// field. Counted against `bullets_total` — the FULL scored count, captured
/// before `.take(limit)` consumes `bullets` — instead.
///
/// LOW fix, PR #963 round 7: round 6 only wired that "count against the
/// pre-take total" fix into `bullets`/`truncated` — `skillsPresent`/
/// `skillsAbsent` were still silently `.take(MAX_SKILLS)`-capped with no
/// signal at all next to them. `skillsAbsent` is the GAP LIST the agent
/// works from, so a résumé/job posting with an unusually long skills
/// section silently withheld skills the agent never saw, while the
/// adjacent `truncated` field (scoped to `bullets` only) read as
/// whole-payload completeness. `skillsTruncated` now counts drops from
/// BOTH skills lists against their own pre-`.take` totals, the same shape
/// `truncated` uses for `bullets`. WHICH skills go is decided upstream: the
/// `.take(MAX_SKILLS)` below cuts a list `documents::evidence` already
/// ordered by posting relevance, so the drops are the least-relevant skills
/// rather than the tail of the alphabet — a CROSS-MODULE invariant this
/// module depends on and cannot re-derive (see [`MAX_SKILLS`]'s doc).
fn compact_evidence_set(set: &EvidenceSet, limit: usize) -> Value {
    let mut bullets: Vec<&EvidenceBullet> = set
        .roles
        .iter()
        .flat_map(|r| r.bullets.iter())
        .chain(set.projects.iter())
        .collect();
    bullets.sort_by(|a, b| b.score.total_cmp(&a.score));
    let bullets_total = bullets.len();
    let capped: Vec<&EvidenceBullet> = bullets.into_iter().take(limit).collect();
    let skills_present: Vec<String> = set
        .skills_present
        .iter()
        .take(MAX_SKILLS)
        .map(|s| clamp_evidence(s))
        .collect();
    let skills_absent: Vec<String> = set
        .skills_absent
        .iter()
        .take(MAX_SKILLS)
        .map(|s| clamp_evidence(s))
        .collect();
    // LOW fix, PR #963 round 7: `skillsPresent`/`skillsAbsent` were silently
    // `.take(MAX_SKILLS)`-capped with no signal alongside them — unlike
    // `bullets`, which reports its own drops in `truncated`. `skillsAbsent`
    // is the GAP LIST the agent is meant to work from (missing skills to
    // add/address), so a résumé/job posting with more than `MAX_SKILLS`
    // present-or-absent skills silently withheld the rest while the
    // adjacent `truncated` field (scoped to `bullets` only) read as
    // whole-payload completeness. Counted against the PRE-`.take` totals of
    // BOTH lists, mirroring `bullets_total`/`ranked.len()`'s "count against
    // the full set, not the post-cap one" shape from round 6.
    let skills_present_total = set.skills_present.len();
    let skills_absent_total = set.skills_absent.len();
    let skills_truncated =
        (skills_present_total - skills_present.len()) + (skills_absent_total - skills_absent.len());
    shrink_to_summary_cap(capped.len(), |kept| {
        let top: Vec<Value> = capped
            .iter()
            .take(kept)
            .map(|b| bullet_to_value(b))
            .collect();
        json!({
            "bullets": top,
            "skillsPresent": skills_present.clone(),
            "skillsAbsent": skills_absent.clone(),
            "skillsTruncated": skills_truncated,
            "truncated": bullets_total - kept,
        })
    })
}

/// `get_trim_suggestions`' payload: the weakest `limit` bullets from an
/// already weakest-first [`rank_bullets`] ranking — never re-sorted here.
///
/// MEDIUM fix, PR #963 round 5: `limit` (up to [`TRIM_SUGGESTIONS_LIMIT`])
/// bullets at up to `BULLET_TEXT_CAP + MAX_HITS * EVIDENCE_CAP` raw chars
/// each is already over [`SUMMARY_CAP`] on its own (see [`SUMMARY_CAP`]'s
/// doc) — routed through [`shrink_to_summary_cap`] the same way
/// [`compact_evidence_set`] is, so a crafted résumé drops whole suggestions
/// from the end of the already-weakest-first list into `truncated` instead
/// of getting cut mid-string.
///
/// MEDIUM fix, PR #963 round 6: same `truncated` bug as
/// [`compact_evidence_set`]'s doc — `capped.len() - kept` only ever counted
/// drops from the shrink loop, never `ranked`'s bullets withheld by the
/// initial `.take(limit)` cap. Counted against `ranked.len()` (the FULL
/// ranking) instead.
fn compact_trim_suggestions(ranked: &[EvidenceBullet], limit: usize) -> Value {
    let capped: Vec<&EvidenceBullet> = ranked.iter().take(limit).collect();
    shrink_to_summary_cap(capped.len(), |kept| {
        let top: Vec<Value> = capped
            .iter()
            .take(kept)
            .map(|b| bullet_to_value(b))
            .collect();
        json!({ "weakestBullets": top, "truncated": ranked.len() - kept })
    })
}

/// `lookup_salary`'s payload: the validated range, or an explicit
/// unavailable-with-`reason` (L-2: `rate_limited`/`provider_unavailable`/
/// `daily_budget_exhausted`/`no_data`, mapped from the actual
/// [`SalaryLookupReason`] the lookup failed with) — never a bare `null`, so
/// the model doesn't have to guess whether an absent range means "no data"
/// or "the tool failed". `daily_budget_exhausted` (round-11 fix, PR #963)
/// used to collapse into `rate_limited`, which reads as "retry shortly" —
/// misleading for a ceiling that only resets at UTC midnight; the
/// `lookup_salary` tool description tells the model not to retry on it.
fn compact_salary_range(outcome: Result<SalaryRange, SalaryLookupReason>) -> Value {
    match outcome {
        Ok(r) => {
            json!({ "available": true, "min": r.min, "max": r.max, "currency": r.currency })
        }
        Err(reason) => {
            let reason = match reason {
                SalaryLookupReason::RateLimited => "rate_limited",
                SalaryLookupReason::ProviderUnavailable => "provider_unavailable",
                SalaryLookupReason::DailyBudgetExhausted => "daily_budget_exhausted",
                SalaryLookupReason::NoData => "no_data",
            };
            json!({ "available": false, "reason": reason })
        }
    }
}

/// Tiny curated country-name → ISO-4217 currency map for the free-text
/// `location` field (`"City, Country"` is the common scraped-posting shape,
/// so the country is usually the last comma-separated segment). NOT the
/// full `packages/prompts` `COUNTRY_TO_CURRENCY` table — that's keyed on an
/// ISO-2 code no cached posting carries (see `JobPostingMeta`), and porting
/// the whole gazetteer here would be its own project. Just enough of this
/// app's common markets (DACH/EU/UK/US/CA/CH) that
/// `salary_research::reconcile_expected_currency` re-engages instead of
/// being a permanent no-op (M-2); an unmatched location still degrades to
/// the existing "unknown currency" behavior (a broader-market estimate,
/// never a hard failure).
fn currency_for_location(location: &str) -> Option<&'static str> {
    let country = location
        .rsplit(',')
        .next()
        .unwrap_or(location)
        .trim()
        .to_lowercase();
    Some(match country.as_str() {
        "germany" | "deutschland" | "austria" | "österreich" | "france" | "spain" | "italy"
        | "netherlands" | "ireland" | "portugal" | "belgium" | "finland" | "greece" => "EUR",
        "united states" | "united states of america" | "usa" | "us" => "USD",
        "united kingdom" | "uk" | "great britain" | "england" => "GBP",
        "switzerland" => "CHF",
        "canada" => "CAD",
        _ => return None,
    })
}

/// Serialize a tool's compact JSON `summary` and wrap it under `"result"`,
/// clamped to [`SUMMARY_CAP`] and made inert as a transcript boundary. See
/// the module SECURITY note: the summary embeds untrusted-résumé/job-derived
/// text, so it goes through exactly the neutralization
/// [`crate::prompt_fence::fenced`] applies to untrusted INPUT — minus the `<tag>`
/// wrap, which `crate::agent::controller::tool_result_fence` breaks open
/// again one layer up (round-9 LOW; see the module doc).
///
/// Clamp-then-neutralize, in that order, mirroring `fenced` exactly: the
/// clamp is the backstop for the one case [`shrink_to_summary_cap`] cannot
/// solve (its `kept == 0` arm returns the envelope whatever its size), and
/// neutralization runs last because it can only ever LENGTHEN the body (one
/// inserted space per forged token), so applying it before the clamp could
/// re-cut a boundary it had just broken. In the normal case the shrink loop
/// has already measured this exact serialization against `SUMMARY_CAP`, so
/// the clamp is a no-op and the budget mechanism is unchanged.
pub(super) fn neutralized_summary(summary: &Value) -> Value {
    let body = serde_json::to_string(summary).unwrap_or_default();
    let body: String = body.chars().take(SUMMARY_CAP).collect();
    json!({ "result": neutralize_transcript_boundaries(&body) })
}

/// Wrap a tool's JSON `summary` under `"result"` — the same top-level
/// envelope shape [`neutralized_summary`] uses for its sibling tools, but
/// WITHOUT neutralization: `SalaryRange` (unlike every other quality-tool
/// payload) carries no untrusted free-text field to neutralize (pinned by
/// `salary_range_serializes_to_only_known_numeric_and_currency_fields`
/// below), so stringify-then-scrub would just relabel already-safe data with
/// no security benefit.
fn envelope_result(value: Value) -> Value {
    json!({ "result": value })
}

// ── Handlers ──────────────────────────────────────────────────────────────
//
// MEDIUM fix, PR #963 round 5: none of the four handlers below used to be
// exercised by any test — only the pure helpers above them were, so the
// not-found error paths and fallback branches were unpinned. Each handler is
// now a thin `AppHandle`-touching wrapper around an `AppHandle`-FREE "core"
// function that carries the actual not-found/fallback logic and is fully
// unit-testable with a plain `Option<&str>` in place of a live
// `DocumentStore`/postings-cache lookup — the same AppHandle-free-core split
// `agent::controller`'s `AgentEnv` trait uses for its own seam (this crate
// has no `tauri::test` mock-app harness, so that split — not a heavier
// AppHandle mock — is the pattern to reuse here too).
//
// MEDIUM perf fix, PR #963 round 10: `validate_resume`/`search_candidate_
// evidence`/`get_trim_suggestions` each ran their `*_core` call — a CPU-bound
// analysis pass (`validate_content`/`extract_evidence`/`rank_bullets`; see
// `clamped_resume_text`'s perf note) — AND the `DocumentStore::get` SQLite
// read feeding it directly inline on the async handler, parking a tokio
// worker for the whole pass on every one of these agent-tool calls. Both now
// run together inside [`spawn_blocking_core`], off the tokio worker.
// `job_text_for`/`job_meta_for` stay inline (unchanged): they lock an
// in-memory `Mutex<PostingsCache>`, not a SQLite connection, the same "cheap
// lock, no spawn_blocking" call `commands::match_resume::match_resume`
// already makes for the identical read.

/// Run one of this module's `*_core` calls — together with the
/// [`DocumentStore`] SQLite read that feeds it — on the `spawn_blocking`
/// pool, so neither parks the calling handler's tokio worker (see this
/// section's round-10 perf note). Same wrapper + `JoinError` →
/// [`AppError::Storage`] mapping `documents::spawn_blocking_db` already uses
/// for this store's blocking writes (`tauri::async_runtime::spawn_blocking`,
/// never a bare `tokio::spawn`); generalized to `Value` here since every core
/// in this module returns `AppResult<Value>`, not `documents`' `AppResult<()>`.
pub(super) async fn spawn_blocking_core<F>(f: F) -> AppResult<Value>
where
    F: FnOnce() -> AppResult<Value> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Storage(format!("quality-tool task failed: {e}")))?
}

/// The trusted "resume not found" error every handler below raises when the
/// run's `ToolContext::resume_id` isn't in the [`DocumentStore`] — factored
/// out so [`validate_resume_core`]/[`search_candidate_evidence_core`]/
/// [`get_trim_suggestions_core`] can construct and test it without an
/// `AppHandle`.
///
/// **The id is CLAMPED into the message** ([`clamped_echo`]). "Trusted" here
/// means trusted-as-ROUTING (it comes from `ToolContext`, never from tool
/// args) — it does NOT mean bounded: `ToolContext::resume_id` is the request's
/// own `resumeId`, an unvalidated wire string, and this message is fenced into
/// the model's transcript AND surfaced verbatim as a run's failure message by
/// `commands::agent`'s generation lookup. Same rule, same cap, one owner as
/// `agent_run`'s own echoes.
pub(super) fn resume_not_found(resume_id: &str) -> AppError {
    AppError::Validation(format!("resume not found: {}", clamped_echo(resume_id)))
}

/// The trusted "job not found" error, mirroring [`resume_not_found`] for the
/// run's `ToolContext::job_id` against the live postings cache — clamped for
/// the reason that one states.
pub(super) fn job_not_found(job_id: &str) -> AppError {
    AppError::Validation(format!("job not found in cache: {}", clamped_echo(job_id)))
}

/// Core, `AppHandle`-free logic for `validate_resume`: the not-found paths
/// and the M-5 empty-draft fallback (validates the candidate's own saved
/// résumé instead of erroring). `validate_resume_handler` resolves
/// `source_text`/`job_text` from the `DocumentStore`/postings cache and
/// delegates here.
///
/// **Why `draft_truncated` (MEDIUM fix, PR #963 round 8) and not simply a
/// bigger cap.** `save_resume` accepts up to
/// [`super::tools::SAVED_RESUME_CAP`] (40,000) chars, so the obvious repair
/// looks like "validate what will actually be saved". It is the wrong one:
///
/// - **Same-universe invariant (the round-3 HIGH).** Both sides of this
///   comparison are clamped to [`RESUME_CAP`] because that is the slice the
///   DRAFTING tool itself was shown (`super::tools::grounded_user_msg`
///   fences the source résumé at `RESUME_CAP` before `draft_resume` ever
///   runs). Raise only the draft side and its tail has no source to match
///   against — every role past 8,000 chars reads as invented. Raise the
///   source side too and `factual.dropped_role` fires for roles the drafting
///   tool was never shown. Either way the fix manufactures false Criticals,
///   which is exactly the class [`clamped_resume_text`] exists to prevent.
/// - **Cost is real but secondary.** Measured on a synthetic worst-case
///   résumé (release build): `validate_content` 17ms at 8k vs 31ms at 40k,
///   `extract_evidence` 1.9ms vs 5.6ms — ~2×, not the order-of-magnitude the
///   O(n×m) shape suggests, since the per-check entry caps bite first. Worth
///   recording honestly: cost alone would NOT have decided this, the
///   invariant above did.
///
/// So the cap stays, and the summary stops claiming a verdict it cannot
/// support: `draftTruncated: true` with `ok: false` (see
/// [`compact_content_report`]). `get_trim_suggestions` deliberately gets no
/// such flag — it returns a weakest-first RANKING, advisory by construction,
/// with no `ok`/`criticals` verdict that a partial read could falsify (the
/// same "does a silent drop actively mislead?" test [`MAX_HITS`] is
/// documented against).
fn validate_resume_core(
    draft_arg: &str,
    draft_truncated: bool,
    doc_kind: DocKind,
    resume_id: &str,
    source_text: Option<&str>,
    job_id: &str,
    job_text: Option<&str>,
) -> AppResult<Value> {
    let source_text = source_text.ok_or_else(|| resume_not_found(resume_id))?;
    // HIGH + MEDIUM fix — see `clamped_resume_text`'s doc. The candidate's own
    // résumé is the factual SOURCE for both doc kinds: a cover letter is
    // checked against it too (`validate::content`'s letter arm), it is just
    // never the thing being checked.
    let source_text = clamped_resume_text(source_text);
    // M-5 fix: an absent/empty draft validates the candidate's CURRENT
    // saved résumé against the job posting — the same
    // "check-the-baseline" fallback `get_trim_suggestions_core` already has
    // (see `optional_draft_arg`'s doc). Falls back to the SAME clamped
    // view as `source_text`, not the raw unclamped text — otherwise this
    // fallback would compare the full résumé against a truncated copy of
    // itself and reintroduce the exact false-Critical class the clamp
    // above exists to prevent.
    //
    // MEDIUM fix, PR #963 round 9: that fallback is résumé-only. There is no
    // saved cover letter to fall back ON (the store holds the candidate's
    // documents, and a letter only exists once this run drafts one), and
    // running the letter ruleset over a résumé would report a document the
    // model never wrote — so an empty draft with `docKind: coverLetter` is a
    // caller error, refused rather than silently answered about the wrong
    // document.
    if draft_arg.is_empty() && doc_kind == DocKind::CoverLetter {
        return Err(AppError::Validation(
            "validate_resume: checking a cover letter needs the drafted letter in `draft` — \
             there is no saved cover letter to fall back on"
                .into(),
        ));
    }
    let draft = if draft_arg.is_empty() {
        source_text.clone()
    } else {
        draft_arg.to_string()
    };
    let job_ad = job_text.ok_or_else(|| job_not_found(job_id))?;
    let job_ad = clamped_job_text(job_ad);
    let lang = detect_locale_tag(&job_ad);
    let input = ContentInput {
        generated: &draft,
        source_resume: &source_text,
        job_ad: &job_ad,
        // No per-job "top requirements" are resolved server-side today
        // (that extraction is client-side, fed only through the save-time
        // IPC payload) — `alignment.missing_top_requirement` simply has
        // nothing to check against here. Every other check is unaffected.
        top_requirements: &[],
        target_language: lang,
        doc_kind,
    };
    let report = validate_content(&input);
    Ok(neutralized_summary(&compact_content_report(
        &report,
        // An EMPTY draft validated the saved résumé instead, which
        // `clamped_resume_text` clamps on its own terms — the flag only ever
        // describes a model-supplied draft that was actually cut.
        draft_truncated && !draft_arg.is_empty(),
    )))
}

fn validate_resume_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move {
        let (draft_arg, draft_truncated) = optional_draft_arg(&args);
        let doc_kind = doc_kind_arg(&args)?;
        // In-memory cache lock, not SQLite — stays inline (see this module's
        // round-10 perf note above `spawn_blocking_core`).
        let job_text = job_text_for(&app, &ctx.job_id);
        let resume_id = ctx.resume_id.clone();
        let job_id = ctx.job_id.clone();
        spawn_blocking_core(move || {
            let source_text = app.state::<DocumentStore>().get(&resume_id).map(|d| d.text);
            validate_resume_core(
                &draft_arg,
                draft_truncated,
                doc_kind,
                &resume_id,
                source_text.as_deref(),
                &job_id,
                job_text.as_deref(),
            )
        })
        .await
    })
}

/// Core, `AppHandle`-free logic for `search_candidate_evidence`: the
/// not-found path and the empty-query fallback (scores against this run's
/// own job posting instead of erroring). See [`validate_resume_core`]'s doc
/// for the AppHandle-free-core pattern.
fn search_candidate_evidence_core(
    query: &str,
    resume_id: &str,
    source_text: Option<&str>,
    job_id: &str,
    job_text: Option<&str>,
) -> AppResult<Value> {
    let source_text = source_text.ok_or_else(|| resume_not_found(resume_id))?;
    // MEDIUM perf fix — see `clamped_resume_text`'s doc.
    let source_text = clamped_resume_text(source_text);
    // `query` is already bounded to `QUERY_CAP` (200 chars, far under
    // `JOB_CAP`) by `optional_query_arg` — one clamp per input, not two.
    let scoring_text = if query.is_empty() {
        let job_text = job_text.ok_or_else(|| job_not_found(job_id))?;
        clamped_job_text(job_text)
    } else {
        query.to_string()
    };
    let set = extract_evidence(&source_text, &scoring_text);
    Ok(neutralized_summary(&compact_evidence_set(
        &set,
        EVIDENCE_SEARCH_LIMIT,
    )))
}

fn search_candidate_evidence_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move {
        let query = optional_query_arg(&args);
        // Only look up the job posting when the fallback actually needs it —
        // mirrors the pre-refactor handler's lazy `job_text_for` call. Stays
        // inline like `validate_resume_handler`'s (in-memory cache, not SQLite).
        let job_text = if query.is_empty() {
            job_text_for(&app, &ctx.job_id)
        } else {
            None
        };
        let resume_id = ctx.resume_id.clone();
        let job_id = ctx.job_id.clone();
        spawn_blocking_core(move || {
            let source_text = app.state::<DocumentStore>().get(&resume_id).map(|d| d.text);
            search_candidate_evidence_core(
                &query,
                &resume_id,
                source_text.as_deref(),
                &job_id,
                job_text.as_deref(),
            )
        })
        .await
    })
}

/// Resolved provider args for `lookup_salary`'s `ai_lookup_salary_reasoned`
/// call — a named struct rather than a 4-tuple (clippy's `type_complexity`
/// lint, and cheaper to read at every call/test site than positional
/// `Option<String>`s).
#[derive(Debug)]
struct SalaryLookupArgs {
    title: String,
    company: Option<String>,
    location: Option<String>,
    currency: Option<String>,
}

/// Core, `AppHandle`-free arg-shaping for `lookup_salary`: the not-found
/// path and the M-2 currency-resolution logic, both testable without an
/// `AppHandle` — only the provider call itself (`ai_lookup_salary_reasoned`)
/// needs one, so it stays in the handler.
fn lookup_salary_args(job_id: &str, meta: Option<&JobPostingMeta>) -> AppResult<SalaryLookupArgs> {
    let meta = meta.ok_or_else(|| job_not_found(job_id))?;
    let company = (!meta.company.trim().is_empty()).then(|| meta.company.clone());
    // M-2 fix: `JobPostingMeta` now carries the posting's free-text
    // location; no ISO-3166 country code is resolved server-side for a
    // cached posting though, so `currency_for_location` is a small
    // curated fallback (mirrors the spirit of
    // `commands::geocoding::geonames::COUNTRY_ALIASES`'s tiny,
    // deliberately-not-exhaustive list) that lets
    // `reconcile_expected_currency` re-engage for the common markets it
    // recognizes. An unmatched location still degrades to the same
    // "unknown currency" case `SalaryResearch::enrich` already handles
    // gracefully — a broader market estimate, never a hard failure.
    let location = (!meta.location.trim().is_empty()).then(|| meta.location.clone());
    let currency = location
        .as_deref()
        .and_then(currency_for_location)
        .map(str::to_string);
    Ok(SalaryLookupArgs {
        title: meta.title.clone(),
        company,
        location,
        currency,
    })
}

fn lookup_salary_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    _args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move {
        let meta = job_meta_for(&app, &ctx.job_id);
        let args = lookup_salary_args(&ctx.job_id, meta.as_ref())?;
        let outcome = crate::commands::ai_salary::ai_lookup_salary_reasoned(
            &app,
            args.title,
            args.company,
            args.location,
            None,
            args.currency,
            None,
        )
        .await;
        Ok(envelope_result(compact_salary_range(outcome)))
    })
}

/// Core, `AppHandle`-free logic for `get_trim_suggestions`: the not-found
/// paths and the empty-draft fallback (ranks the candidate's own saved
/// résumé instead of erroring). See [`validate_resume_core`]'s doc for the
/// AppHandle-free-core pattern.
fn get_trim_suggestions_core(
    draft_arg: &str,
    resume_id: &str,
    source_text: Option<&str>,
    job_id: &str,
    job_text: Option<&str>,
) -> AppResult<Value> {
    let job_ad = job_text.ok_or_else(|| job_not_found(job_id))?;
    // MEDIUM perf fix — see `clamped_resume_text`'s/`clamped_job_text`'s doc.
    let job_ad = clamped_job_text(job_ad);
    let text = if draft_arg.is_empty() {
        let source_text = source_text.ok_or_else(|| resume_not_found(resume_id))?;
        clamped_resume_text(source_text)
    } else {
        draft_arg.to_string()
    };
    let ranked = rank_bullets(&text, &job_ad);
    Ok(neutralized_summary(&compact_trim_suggestions(
        &ranked,
        TRIM_SUGGESTIONS_LIMIT,
    )))
}

fn get_trim_suggestions_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move {
        // The truncation flag is deliberately unused here — see
        // `validate_resume_core`'s doc for why a ranking needs no such
        // signal while an `ok`/`criticals` verdict does.
        let (draft_arg, _truncated) = optional_draft_arg(&args);
        // In-memory cache lock, not SQLite — stays inline (see this module's
        // round-10 perf note above `spawn_blocking_core`).
        let job_text = job_text_for(&app, &ctx.job_id);
        let resume_id = ctx.resume_id.clone();
        let job_id = ctx.job_id.clone();
        spawn_blocking_core(move || {
            // Only load the saved résumé when the fallback actually needs it —
            // mirrors the pre-refactor handler's lazy `DocumentStore` lookup.
            let source_text = if draft_arg.is_empty() {
                app.state::<DocumentStore>().get(&resume_id).map(|d| d.text)
            } else {
                None
            };
            get_trim_suggestions_core(
                &draft_arg,
                &resume_id,
                source_text.as_deref(),
                &job_id,
                job_text.as_deref(),
            )
        })
        .await
    })
}

// ── Schemas ───────────────────────────────────────────────────────────────

fn validate_resume_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "draft": {
                "type": "string",
                // The cap is stated to the model rather than left implicit: a
                // longer draft still validates (its first `RESUME_CAP` chars),
                // but the result then carries `draftTruncated: true` and
                // `ok: false` — see `validate_resume_core`'s doc.
                "description": format!(
                    "The generated draft to check for factual, alignment, consistency, \
                     ATS-structure, and voice issues against the candidate's own résumé and \
                     this run's job posting. Leave empty to check the candidate's saved résumé \
                     instead (résumé only — a cover letter check must pass the drafted letter \
                     here). Only the first {RESUME_CAP} characters are checked; a longer \
                     draft comes back with draftTruncated: true and ok: false, never a clean \
                     verdict for the part that was not read."
                )
            },
            // MEDIUM fix, PR #963 round 9 — see `doc_kind_arg`'s doc. Declared
            // as an `enum` so a tool-calling model with constrained decoding
            // can't emit a third value; `doc_kind_arg` re-validates anyway,
            // because a schema guarantees shape, never values.
            "docKind": {
                "type": "string",
                "enum": ["resume", "coverLetter"],
                "description": "Which ruleset to check the draft against. Defaults to \
                    \"resume\". Pass \"coverLetter\" to check a drafted cover letter instead — \
                    a letter has no sections, roles or bullets, so the résumé-structure checks \
                    would report problems it cannot have."
            }
        }
    })
}

fn search_candidate_evidence_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "A skill, achievement, or requirement to search the candidate's \
                    own résumé for. Leave empty to search against this run's own job posting."
            }
        }
    })
}

fn get_trim_suggestions_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "draft": {
                "type": "string",
                "description": "The résumé text to rank weakest-first for trimming against \
                    this run's job posting. Leave empty to use the candidate's saved résumé."
            }
        }
    })
}

// ── Registration ─────────────────────────────────────────────────────────

/// The four résumé-quality Read tools, appended to
/// [`super::tools::read_tools`]'s whitelist. See the module doc for the
/// zero-business-logic-here contract.
pub(crate) fn quality_tools() -> Vec<AgentTool> {
    vec![
        AgentTool {
            name: "validate_resume",
            description: "Run deterministic content checks (facts, alignment, consistency, ATS \
                 structure, voice) on a résumé draft against the candidate's own résumé and \
                 this run's job posting, and return a compact summary of what's wrong. Pass \
                 docKind \"coverLetter\" to check a drafted cover letter against the letter \
                 ruleset instead. Read-only."
                .to_string(),
            schema: validate_resume_schema(),
            kind: ToolKind::Read,
            handler: validate_resume_handler,
        },
        AgentTool {
            name: "search_candidate_evidence",
            description:
                "Search the candidate's own résumé for evidence (bullets, skills) that backs a \
                 claim — never invents anything the résumé doesn't already say. Read-only."
                    .to_string(),
            schema: search_candidate_evidence_schema(),
            kind: ToolKind::Read,
            handler: search_candidate_evidence_handler,
        },
        AgentTool {
            name: "lookup_salary",
            description:
                "Look up a web-grounded market salary range for this run's own job posting. \
                 Read-only. Takes no arguments — it always targets this run's own role/company. \
                 An unavailable result reports why via `reason`: `rate_limited` may succeed on \
                 a retry later this run, but `daily_budget_exhausted` will not — that provider's \
                 daily request ceiling only resets at UTC midnight, so do not retry the call \
                 this run."
                    .to_string(),
            schema: json!({ "type": "object", "properties": {} }),
            kind: ToolKind::Read,
            handler: lookup_salary_handler,
        },
        AgentTool {
            name: "get_trim_suggestions",
            description:
                "Rank a résumé's bullets weakest-first against this run's job posting, to help \
                 decide what to cut if the document runs long. Read-only."
                    .to_string(),
            schema: get_trim_suggestions_schema(),
            kind: ToolKind::Read,
            handler: get_trim_suggestions_handler,
        },
    ]
}

#[cfg(test)]
mod test;
