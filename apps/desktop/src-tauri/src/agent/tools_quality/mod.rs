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
//! transcript through [`fenced`] ([`fenced_summary`]) — the same
//! neutralize-then-fence pipeline guarding every other untrusted block in
//! `agent::tools`, so a forged fence tag smuggled inside a quoted span can't
//! masquerade as a new `<job_posting>`/`<candidate_resume>` boundary.
//! `lookup_salary` is the one exception: its result carries no free text (see
//! `salary_range_serializes_to_only_known_numeric_and_currency_fields`), so
//! it skips fencing and uses the plain [`envelope_result`] wrapper instead.

use std::future::Future;
use std::pin::Pin;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::commands::ai_salary::SalaryLookupReason;
use crate::commands::match_resume::{job_meta_for, job_text_for};
use crate::documents::evidence::{extract_evidence, rank_bullets, EvidenceBullet, EvidenceSet};
use crate::documents::keywords::detect_locale_tag;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::salary_research::SalaryRange;
use crate::validate::content::{
    validate_content, ContentInput, ContentIssue, ContentReport, DocKind,
};
use crate::validate::Severity;

use super::tools::{fenced, AgentTool, ToolContext, ToolKind, JOB_CAP, RESUME_CAP};

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
const EVIDENCE_CAP: usize = 80;

/// Cap on a validator issue's `message` in the compact summary — longer than
/// [`EVIDENCE_CAP`] since guidance prose reads longer than a quoted span, but
/// still bounded so a crafted draft that trips many long-message issues can't
/// blow the tool-result budget.
const MESSAGE_CAP: usize = 400;

/// Cap on a validator issue's `section` in the compact summary — section
/// names are short labels ("Experience", "Skills"), never free-flowing text;
/// a defensive backstop, not an expected truncation point.
const SECTION_CAP: usize = 80;

/// Cap on a bullet's quoted `text` in a compact tool summary — same
/// rationale as [`EVIDENCE_CAP`]: it quotes untrusted résumé text directly.
const BULLET_TEXT_CAP: usize = 200;

/// Max issues surfaced in `validate_resume`'s compact summary. A crafted
/// draft that trips dozens of checks must not blow the tool-result budget —
/// dropped issues are counted in the summary's `truncated` field, never a
/// mid-string cut of the issue list.
const MAX_ISSUES: usize = 20;

/// Max entries kept in `skillsPresent`/`skillsAbsent` — a résumé with an
/// unusually long skills section must not blow the tool-result budget either.
const MAX_SKILLS: usize = 15;

/// Max entries kept in one bullet's `hits` (job-derived keyword matches) —
/// analogous to [`MAX_SKILLS`], scoped to a single bullet instead of the
/// résumé's whole skills section: a keyword-dense job posting must not blow
/// the tool-result budget either (MEDIUM fix, PR #963 round 4).
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
/// `consistency.skill_not_demonstrated`, is 34), plus 60 chars for the
/// object's own JSON syntax (keys/quotes/colons/commas/braces — measured at
/// ~52: `{"code":"…","section":"…","message":"…","evidence":"…"},`).
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
/// the body fits [`SUMMARY_CAP`], rather than relying on `fenced()`'s hard
/// `body.chars().take(cap)` as the enforcement point.
const PER_ISSUE_WORST_CASE: usize = SECTION_CAP + MESSAGE_CAP + EVIDENCE_CAP + 40 + 60;

/// Target ceiling on a fenced tool-result summary's SERIALIZED body —
/// `MAX_ISSUES` issues at [`PER_ISSUE_WORST_CASE`] each (a plain-ASCII,
/// non-escaping estimate), plus 500 chars of headroom for the summary's own
/// envelope (`ok`/`criticals`/`warnings`/`truncated` fields, the `issues`
/// array's brackets — measured at ~70).
///
/// [`compact_content_report`] ENFORCES this by measurement, not by trusting
/// the estimate above: it serializes the candidate summary and, if it's over
/// `SUMMARY_CAP`, drops the weakest (last-sorted) whole issue and re-checks,
/// repeating until the body fits or every issue is gone. Dropped issues are
/// counted in `truncated`, never a mid-string cut.
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

/// Trim + clamp an optional `draft` arg to [`RESUME_CAP`] chars. An
/// absent/empty draft is a valid "use the saved résumé instead" case for
/// BOTH `validate_resume` and `get_trim_suggestions` — never an error; each
/// handler falls back to the candidate's saved résumé (M-5: this also means
/// the model is never FORCED to echo the whole draft as tool-call arguments
/// just to run a sanity check on the résumé it already has).
fn optional_draft_arg(args: &Value) -> String {
    let draft = args
        .get("draft")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    draft.chars().take(RESUME_CAP).collect()
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

// ── Compact, clamped summaries (unit-testable without an AppHandle) ─────────

/// Cap `s` to `cap` chars, char-boundary safe — the one clamp primitive
/// every per-field cap in this module reuses ([`clamp_evidence`], and the
/// `message`/`section`/bullet-`text` clamps in [`compact_content_report`]/
/// [`bullet_to_value`]), so every clamp behaves identically.
fn clamp_chars(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

/// Cap `s` to [`EVIDENCE_CAP`] chars, char-boundary safe.
fn clamp_evidence(s: &str) -> String {
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
fn clamped_resume_text(text: &str) -> String {
    clamp_chars(text, RESUME_CAP)
}

/// Clamp a cached job posting's text to [`JOB_CAP`] for the same perf reason
/// as [`clamped_resume_text`] — scraped posting text is unbounded too.
/// Mirrors `super::tools::research_company_handler`'s own
/// `.chars().take(JOB_CAP)` clamp.
fn clamped_job_text(text: &str) -> String {
    clamp_chars(text, JOB_CAP)
}

/// Compact a [`ContentReport`] into what `validate_resume` actually returns
/// to the model: counts, plus up to [`MAX_ISSUES`] issues (each
/// code/section/message/evidence field individually clamped — see the module
/// SECURITY note), plus a `truncated` count for anything dropped past that
/// cap OR past [`SUMMARY_CAP`]'s serialized-length budget (MEDIUM fix, PR
/// #963 round 4 — see [`SUMMARY_CAP`]'s doc: per-field char clamps alone
/// don't bound the SERIALIZED size once JSON escaping is in play). Never a
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
fn compact_content_report(report: &ContentReport) -> Value {
    let criticals = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .count();
    let warnings = report.issues.len() - criticals;
    let mut ordered: Vec<&ContentIssue> = report.issues.iter().collect();
    // `false < true`: Criticals sort ahead of everything else.
    ordered.sort_by_key(|i| i.severity != Severity::Critical);
    let mut kept = MAX_ISSUES.min(ordered.len());
    loop {
        let issues: Vec<Value> = ordered
            .iter()
            .take(kept)
            .map(|i| {
                json!({
                    "code": i.code,
                    "section": i.section.as_deref().map(|s| clamp_chars(s, SECTION_CAP)),
                    "message": clamp_chars(&i.message, MESSAGE_CAP),
                    "evidence": i.evidence.as_deref().map(clamp_evidence),
                })
            })
            .collect();
        let candidate = json!({
            "ok": report.ok,
            "criticals": criticals,
            "warnings": warnings,
            "truncated": report.issues.len() - kept,
            "issues": issues,
        });
        // Measure the ACTUAL serialized length — per-field char clamps alone
        // don't bound this once JSON escaping inflates a field (see
        // SUMMARY_CAP's doc). Drop a whole issue and retry rather than let
        // `fenced()`'s hard char cap cut the JSON body mid-string.
        let fits = serde_json::to_string(&candidate)
            .map(|s| s.chars().count() <= SUMMARY_CAP)
            .unwrap_or(false);
        if fits || kept == 0 {
            return candidate;
        }
        kept -= 1;
    }
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
fn compact_evidence_set(set: &EvidenceSet, limit: usize) -> Value {
    let mut bullets: Vec<&EvidenceBullet> = set
        .roles
        .iter()
        .flat_map(|r| r.bullets.iter())
        .chain(set.projects.iter())
        .collect();
    bullets.sort_by(|a, b| b.score.total_cmp(&a.score));
    let top: Vec<Value> = bullets
        .into_iter()
        .take(limit)
        .map(bullet_to_value)
        .collect();
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
    json!({
        "bullets": top,
        "skillsPresent": skills_present,
        "skillsAbsent": skills_absent,
    })
}

/// `get_trim_suggestions`' payload: the weakest `limit` bullets from an
/// already weakest-first [`rank_bullets`] ranking — never re-sorted here.
fn compact_trim_suggestions(ranked: &[EvidenceBullet], limit: usize) -> Value {
    let top: Vec<Value> = ranked.iter().take(limit).map(bullet_to_value).collect();
    json!({ "weakestBullets": top })
}

/// `lookup_salary`'s payload: the validated range, or an explicit
/// unavailable-with-`reason` (L-2: `rate_limited`/`provider_unavailable`/
/// `no_data`, mapped from the actual [`SalaryLookupReason`] the lookup
/// failed with) — never a bare `null`, so the model doesn't have to guess
/// whether an absent range means "no data" or "the tool failed".
fn compact_salary_range(outcome: Result<SalaryRange, SalaryLookupReason>) -> Value {
    match outcome {
        Ok(r) => {
            json!({ "available": true, "min": r.min, "max": r.max, "currency": r.currency })
        }
        Err(reason) => {
            let reason = match reason {
                SalaryLookupReason::RateLimited => "rate_limited",
                SalaryLookupReason::ProviderUnavailable => "provider_unavailable",
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

/// Wrap a tool's compact JSON `summary` under `"result"`, fenced as `tag`. See
/// the module SECURITY note: the summary embeds untrusted-résumé/job-derived
/// text, so it goes through the same neutralize-then-fence pipeline every
/// other untrusted block in `agent::tools` uses.
fn fenced_summary(tag: &'static str, summary: &Value) -> Value {
    let body = serde_json::to_string(summary).unwrap_or_default();
    json!({ "result": fenced(tag, &body, SUMMARY_CAP) })
}

/// Wrap a tool's JSON `summary` under `"result"` — the same top-level
/// envelope shape [`fenced_summary`] uses for its sibling tools, but WITHOUT
/// fencing: `SalaryRange` (unlike every other quality-tool payload) carries
/// no untrusted free-text field to neutralize (pinned by
/// `salary_range_serializes_to_only_known_numeric_and_currency_fields`
/// below), so fence-then-stringify would just relabel already-safe data with
/// no security benefit.
fn envelope_result(value: Value) -> Value {
    json!({ "result": value })
}

// ── Handlers ──────────────────────────────────────────────────────────────

fn validate_resume_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move {
        let draft_arg = optional_draft_arg(&args);
        let source = app
            .state::<DocumentStore>()
            .get(&ctx.resume_id)
            .ok_or_else(|| AppError::Validation(format!("resume not found: {}", ctx.resume_id)))?;
        // HIGH + MEDIUM fix — see `clamped_resume_text`'s doc.
        let source_text = clamped_resume_text(&source.text);
        // M-5 fix: an absent/empty draft validates the candidate's CURRENT
        // saved résumé against the job posting — the same
        // "check-the-baseline" fallback `get_trim_suggestions` already has
        // (see `optional_draft_arg`'s doc). Falls back to the SAME clamped
        // view as `source_text`, not the raw unclamped text — otherwise this
        // fallback would compare the full résumé against a truncated copy of
        // itself and reintroduce the exact false-Critical class the clamp
        // above exists to prevent.
        let draft = if draft_arg.is_empty() {
            source_text.clone()
        } else {
            draft_arg
        };
        let job_ad = job_text_for(&app, &ctx.job_id).ok_or_else(|| {
            AppError::Validation(format!("job not found in cache: {}", ctx.job_id))
        })?;
        let job_ad = clamped_job_text(&job_ad);
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
            doc_kind: DocKind::Resume,
        };
        let report = validate_content(&input);
        Ok(fenced_summary(
            "validate_resume_result",
            &compact_content_report(&report),
        ))
    })
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
        let source = app
            .state::<DocumentStore>()
            .get(&ctx.resume_id)
            .ok_or_else(|| AppError::Validation(format!("resume not found: {}", ctx.resume_id)))?;
        // MEDIUM perf fix — see `clamped_resume_text`'s doc.
        let source_text = clamped_resume_text(&source.text);
        // `query` is already bounded to `QUERY_CAP` (200 chars, far under
        // `JOB_CAP`) by `optional_query_arg` — one clamp per input, not two.
        let scoring_text = if query.is_empty() {
            clamped_job_text(&job_text_for(&app, &ctx.job_id).ok_or_else(|| {
                AppError::Validation(format!("job not found in cache: {}", ctx.job_id))
            })?)
        } else {
            query
        };
        let set = extract_evidence(&source_text, &scoring_text);
        Ok(fenced_summary(
            "search_candidate_evidence_result",
            &compact_evidence_set(&set, EVIDENCE_SEARCH_LIMIT),
        ))
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
        let meta = job_meta_for(&app, &ctx.job_id).ok_or_else(|| {
            AppError::Validation(format!("job not found in cache: {}", ctx.job_id))
        })?;
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
        let outcome = crate::commands::ai_salary::ai_lookup_salary_reasoned(
            &app, meta.title, company, location, None, currency, None,
        )
        .await;
        Ok(envelope_result(compact_salary_range(outcome)))
    })
}

fn get_trim_suggestions_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move {
        let draft_arg = optional_draft_arg(&args);
        let job_ad = job_text_for(&app, &ctx.job_id).ok_or_else(|| {
            AppError::Validation(format!("job not found in cache: {}", ctx.job_id))
        })?;
        // MEDIUM perf fix — see `clamped_resume_text`'s/`clamped_job_text`'s doc.
        let job_ad = clamped_job_text(&job_ad);
        let text = if draft_arg.is_empty() {
            clamped_resume_text(
                &app.state::<DocumentStore>()
                    .get(&ctx.resume_id)
                    .ok_or_else(|| {
                        AppError::Validation(format!("resume not found: {}", ctx.resume_id))
                    })?
                    .text,
            )
        } else {
            draft_arg
        };
        let ranked = rank_bullets(&text, &job_ad);
        Ok(fenced_summary(
            "get_trim_suggestions_result",
            &compact_trim_suggestions(&ranked, TRIM_SUGGESTIONS_LIMIT),
        ))
    })
}

// ── Schemas ───────────────────────────────────────────────────────────────

fn validate_resume_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "draft": {
                "type": "string",
                "description": "The generated résumé draft to check for factual, alignment, \
                    consistency, ATS-structure, and voice issues against the candidate's own \
                    résumé and this run's job posting. Leave empty to check the candidate's \
                    saved résumé instead."
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
                 this run's job posting, and return a compact summary of what's wrong. \
                 Read-only."
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
                 Read-only. Takes no arguments — it always targets this run's own role/company."
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
