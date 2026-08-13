//! Phase-3 résumé-pipeline Read tools for the agent registry: `analyze_job`,
//! `get_quality_report`, `run_quality_pipeline`.
//!
//! A third file in the SAME registry, split from [`super::tools`] /
//! [`super::tools_quality`] for the one reason those two were already split
//! from each other: R8's module-size cap (`tools_quality/mod.rs` is ~1 200 LOC
//! against a 1 400 hard cap). Every handler here is a thin adapter over
//! `pipeline::resume` — the stage, the pipeline and the persisted report are
//! all existing Rust; nothing about how a résumé is analysed, generated or
//! scored is re-decided in this file.
//!
//! SECURITY (same trust story as its two siblings): the source résumé and the
//! job posting are loaded SERVER-SIDE from the trusted [`ToolContext`], never
//! from model-supplied args — none of these three tools takes an argument at
//! all. Routing is [`Completer::from_active`] (task #25). Every summary they
//! return quotes untrusted text (a requirement lifted from a scraped ad, an
//! issue's evidence span, the generated draft), so each one is clamped
//! field-by-field, measured against `SUMMARY_CAP` by
//! [`shrink_to_summary_cap`], and made inert as a transcript boundary by
//! [`neutralized_summary`] — the SAME primitives `tools_quality` uses, widened
//! rather than copied (see that module's doc).
//!
//! ## Nothing here writes
//!
//! `run_quality_pipeline` returns the draft and its report as DATA. It does not
//! touch `ai_generations`, unlike `commands::resume_pipeline::execute`, which
//! persists. Saving stays behind the existing gated `save_resume` Write tool,
//! so a document only ever reaches the store through a confirmation the user
//! saw and could edit.
//!
//! ## What bounds a tool call
//!
//! [`crate::agent::controller`] races EVERY tool call against
//! `Budget::step_timeout` and ends the WHOLE run at
//! [`StoppedReason::Timeout`] if one overruns — so a tool that makes provider
//! calls has to fit inside that, and the two facts below are why the three
//! tools are whitelisted differently:
//!
//! * `analyze_job` is ONE stage. Its single `complete_json` is allowed one
//!   re-ask, i.e. two `OLLAMA_COMPLETION`-bounded calls, which does NOT fit —
//!   so it runs under [`AGENT_STAGE_DEADLINE`], derived so the re-ask is
//!   refused rather than overrunning.
//! * `run_quality_pipeline` is FIFTEEN provider calls with a 75-minute floor
//!   ([`Budget::RESUME_QUALITY`]`.run_timeout`). No shrinking makes that fit a
//!   360 s step; the tool is therefore registered but kept OUT of the prep
//!   whitelist — see [`super::tools::improve_resume_tools`].

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};

use crate::ai_generations::{AiGenerationRecord, AiGenerationStore};
use crate::commands::ai_provider::timeouts;
use crate::commands::match_resume::{job_meta_for, job_text_for};
use crate::commands::resume_pipeline::report::hash_text;
use crate::db::new_job_id;
use crate::documents::keywords::detect_locale_tag;
use crate::documents::DocumentStore;
use crate::error::AppResult;
use crate::limits::{Limiter, PROVIDER_DAILY_MAX};
use crate::pipeline::budget::{Budget, StoppedReason};
use crate::pipeline::cache::KvCache;
use crate::pipeline::resume::types::JobAnalysis;
use crate::pipeline::resume::{
    guard_deadline, quality_pipeline, run_deadline, stages, QualityCtx, QualityInput, RunDeadline,
    RunLedger,
};
use crate::pipeline::{Completer, Stage, StageHooks, StageInfo, StageOutcome};
use crate::validate::content::ContentReport;

use super::tools::{AgentTool, ToolContext, ToolKind, RESUME_CAP};
use super::tools_quality::{
    clamp_chars, clamp_evidence, clamped_job_text, clamped_resume_text, job_not_found,
    neutralized_summary, resume_not_found, shrink_to_summary_cap, spawn_blocking_core, MAX_ISSUES,
    MESSAGE_CAP, SECTION_CAP,
};

// ── Bounds ─────────────────────────────────────────────────────────────────

/// Slack between [`AGENT_STAGE_DEADLINE`] + one full provider call and the
/// agent's own per-step wall clock, so the arithmetic below is strictly under
/// the bound it is derived from rather than exactly equal to it (an equality
/// is a race between two clocks started at different instants).
const AGENT_STAGE_MARGIN_SECS: u64 = 10;

/// The wall clock ONE stage gets when the agent runs it.
///
/// Derived, not picked. [`Completer::complete_json`] is allowed one re-ask,
/// and the re-ask is a second full provider call at up to
/// [`timeouts::OLLAMA_COMPLETION`] (300 s) — so an `analyze_job` whose first
/// call answers slowly and unparseably would spend 600 s against
/// [`Budget::AGENT_PREP`]'s 360 s `step_timeout`, and the controller's race
/// would end the WHOLE agent run at [`StoppedReason::Timeout`] (the user loses
/// the prep run, its drafts and its saves — not just this tool's answer).
///
/// `step_timeout − OLLAMA_COMPLETION − margin` is the largest deadline for
/// which "a re-ask that STARTS inside the deadline still finishes inside the
/// step" holds: the guard runs before every round-trip, so once this much wall
/// time is gone the re-ask is refused by `pipeline::resume::guard_deadline`
/// with the run-timeout message instead of being paid for. The first call is
/// never refused (nothing has elapsed yet) and is bounded by its own HTTP
/// timeout.
const AGENT_STAGE_DEADLINE: Duration = Duration::from_secs(
    Budget::AGENT_PREP.step_timeout.as_secs()
        - timeouts::OLLAMA_COMPLETION.as_secs()
        - AGENT_STAGE_MARGIN_SECS,
);

// The derivation above, asserted at COMPILE time rather than in a test — the
// same discipline `pipeline::budget`'s budget relations use, and for the same
// reason: shrinking `step_timeout` (or raising `OLLAMA_COMPLETION`) past this
// relation must fail `cargo build`, not merely `cargo test`.
const _: () = assert!(
    AGENT_STAGE_DEADLINE.as_secs() + timeouts::OLLAMA_COMPLETION.as_secs()
        < Budget::AGENT_PREP.step_timeout.as_secs(),
    "a stage the agent runs plus one full provider call must fit inside one agent step"
);

// The same relation for the OTHER direction of the same race, and the reason
// `Budget::AGENT_IMPROVE.step_timeout` is 90 minutes: `run_quality_pipeline` is
// reachable only from the improve flow, and ONE call of it is a whole quality
// run bounded by [`quality_tool_deadline`] — `run_deadline(RESUME_QUALITY,
// quality_run_deadline(None))`, i.e. `RESUME_QUALITY.run_timeout` (4 500 s) at
// the effort-blind bottom tier, which
// `quality_run_deadline_agrees_with_the_budget_floor_at_the_bottom_tier` and
// `the_tool_run_deadline_is_the_commands_own_floor` pin — the second one against
// the FUNCTION, which is what makes it legitimate for this assert to read the
// constant instead (a `const` cannot call `quality_tool_deadline`).
//
// Unlike `AGENT_STAGE_DEADLINE` above, the deadline is FIXED (it is the same
// floor the IPC command uses, deliberately) and the step timeout is what has to
// clear it: `guard_deadline` refuses the NEXT call, so a call admitted one
// second inside the deadline still runs its own full `OLLAMA_COMPLETION`. A
// `step_timeout` shrunk under that sum makes the controller's race, not the
// pipeline's own deadline, the thing that ends the run — at
// `StoppedReason::Timeout`, mid-tool, with nothing saved — so it fails
// `cargo build` rather than a test.
const _: () = assert!(
    Budget::RESUME_QUALITY.run_timeout.as_secs()
        + timeouts::OLLAMA_COMPLETION.as_secs()
        + AGENT_STAGE_MARGIN_SECS
        < Budget::AGENT_IMPROVE.step_timeout.as_secs(),
    "one whole quality run plus the last call it may admit must fit inside one improve-flow step"
);

/// Max entries kept per list in [`compact_job_analysis`]. A posting is free to
/// state forty "must haves"; the model reading this summary needs the top of
/// each list, not all of it, and the drops are counted in `truncated`.
const MAX_ANALYSIS_ITEMS: usize = 15;

/// Max entries kept in a slot's `fabrications` list. Bounded for the same
/// reason [`MAX_ISSUES`] is; the drops are counted in `fabricationsTruncated`
/// because this list is one the model is meant to act on exhaustively.
const MAX_FABRICATIONS: usize = 20;

// ── Compact, clamped summaries (unit-testable without an AppHandle) ─────────

/// Whether an ALREADY-SERIALIZED issue object is a Critical.
///
/// Reads the wire word rather than a typed `Severity` on purpose: this same
/// helper runs over a freshly-serialized [`ContentReport`] AND over a report
/// read back out of the persisted `quality_report` blob, which is only ever
/// `serde_json::Value` (`ContentIssue` is `Serialize`-only by design). An
/// entry with no recognizable severity counts as a Warning — it is not
/// evidence of a Critical.
fn is_critical(issue: &Value) -> bool {
    issue.get("severity").and_then(Value::as_str) == Some("critical")
}

/// One issue, every untrusted field clamped. Same caps, same order of fields
/// as `tools_quality::compact_content_report` — a model that has learned to
/// read one of these summaries can read the other.
fn compact_issue(issue: &Value) -> Value {
    json!({
        "code": issue.get("code").and_then(Value::as_str).map(clamp_evidence),
        "severity": issue.get("severity").and_then(Value::as_str).map(clamp_evidence),
        "section": issue.get("section").and_then(Value::as_str).map(|s| clamp_chars(s, SECTION_CAP)),
        "message": issue.get("message").and_then(Value::as_str).map(|m| clamp_chars(m, MESSAGE_CAP)),
        "evidence": issue.get("evidence").and_then(Value::as_str).map(clamp_evidence),
    })
}

/// Counts plus the first `kept` issues, **Criticals first**.
///
/// The ordering is the same one `tools_quality::compact_content_report`
/// applies and exists for the same reason: the validator emits in CHECK order,
/// so on a draft that trips twenty Warnings a late Critical fell off the list
/// and the model saw `criticals: 1` with no Critical it could act on. The sort
/// is stable, so within a severity the emission order survives, and dropping
/// from the END means a Critical is the last thing this ever drops.
fn issue_summary(issues: &[Value], kept: usize) -> Value {
    let mut ordered: Vec<&Value> = issues.iter().collect();
    // `false < true`: Criticals sort ahead of everything else.
    ordered.sort_by_key(|issue| !is_critical(issue));
    let criticals = ordered.iter().filter(|issue| is_critical(issue)).count();
    let kept = kept.min(ordered.len());
    json!({
        "criticals": criticals,
        "warnings": ordered.len() - criticals,
        "issues": ordered.iter().take(kept).map(|issue| compact_issue(issue)).collect::<Vec<_>>(),
        "truncated": ordered.len() - kept,
    })
}

/// `analyze_job`'s payload: the posting's own requirements, clamped.
///
/// `kept` shrinks EVERY list uniformly rather than one designated list, because
/// unlike the single-list summaries in `tools_quality` there is no ranking here
/// to drop the tail of — the five lists are peers, and starving one to keep
/// another whole would silently bias what the model plans against.
fn compact_job_analysis(analysis: &JobAnalysis) -> Value {
    let lists: [(&str, &Vec<String>); 5] = [
        ("mustHave", &analysis.must_have),
        ("niceToHave", &analysis.nice_to_have),
        ("responsibilities", &analysis.responsibilities),
        ("domainKeywords", &analysis.domain_keywords),
        ("redFlags", &analysis.red_flags),
    ];
    shrink_to_summary_cap(MAX_ANALYSIS_ITEMS, |kept| {
        let mut out = Map::new();
        out.insert(
            "roleTitle".to_string(),
            json!(clamp_evidence(&analysis.role_title)),
        );
        out.insert(
            "seniority".to_string(),
            json!(clamp_evidence(&analysis.seniority)),
        );
        out.insert(
            "language".to_string(),
            json!(clamp_evidence(&analysis.language)),
        );
        let mut dropped = 0usize;
        for (key, values) in lists {
            let kept = kept.min(values.len());
            dropped += values.len() - kept;
            out.insert(
                (*key).to_string(),
                json!(values
                    .iter()
                    .take(kept)
                    .map(|value| clamp_evidence(value))
                    .collect::<Vec<_>>()),
            );
        }
        out.insert("truncated".to_string(), json!(dropped));
        Value::Object(out)
    })
}

/// Whether a persisted slot's report still describes the text it sits beside.
///
/// `null`, not `false`, when the slot carries no `sourceTextHash`: that is the
/// JOIN between a report and a document (see
/// `commands::resume_pipeline`'s module doc), and a report written without one
/// cannot be checked at all. Answering `false` there would tell the model the
/// report is current when nothing established that — the divergence is meant
/// to be surfaced honestly, and "unknown" is one of its three states.
fn stale_flag(slot: &Value, current_text: &str) -> Value {
    match slot.get("sourceTextHash").and_then(Value::as_u64) {
        Some(hash) => json!(hash != u64::from(hash_text(current_text))),
        None => Value::Null,
    }
}

/// One slot's fabrication review list, clamped, plus how many entries nobody
/// has decided about yet. `decision` is `null` for an undecided entry.
/// (`report::unresolved_count` applies a STRICTER run-level rule — a recorded
/// Remove keeps counting until its span leaves the document — but `undecided`
/// here is about the VERDICT, which is the part the model can act on.)
///
/// `kept` is [`shrink_to_summary_cap`]'s, so this list shrinks WITH the issue
/// list rather than sitting outside the budget: two slots' worth of
/// [`MAX_FABRICATIONS`] entries is ~15k raw chars on its own declared worst
/// case, over `SUMMARY_CAP` BEFORE any JSON escaping applies — exactly the
/// round-5 hole `tools_quality`'s `compact_evidence_set` was fixed for. The
/// `undecided` COUNT is deliberately NOT shrunk: it is the number the model
/// acts on, and counting only the entries that survived the budget would tell
/// it the review is finished when it is not.
///
/// **A field WHITELIST, not a copy of the entry** — which is what keeps the
/// budget arithmetic above valid as the persisted shape grows. A stored entry
/// also carries `line`, the whole document line the finding sits on (up to
/// 1 000 chars — see `commands::resume_pipeline::report::containing_line`);
/// listing it here would multiply the per-entry worst case by ~6 and collapse
/// the review list to a handful of entries on an ordinary report, for a field
/// the model cannot act on: the anchor exists so the RENDERER can apply a
/// Remove, and no tool in this registry can record a verdict. Widening this
/// list means re-doing that arithmetic AND re-running the fence battery —
/// `every_pipeline_payload_neutralizes_a_forged_boundary_in_untrusted_text`
/// plants a forgery in `line` for exactly that reason.
fn compact_fabrications(slot: &Value, kept: usize) -> (Vec<Value>, usize, usize) {
    let Some(entries) = slot.get("fabrications").and_then(Value::as_array) else {
        return (Vec::new(), 0, 0);
    };
    let undecided = entries
        .iter()
        .filter(|entry| entry.get("decision").is_none())
        .count();
    let listed: Vec<Value> = entries
        .iter()
        .take(kept.min(MAX_FABRICATIONS))
        .map(|entry| {
            json!({
                "issueKey": entry.get("issueKey").and_then(Value::as_str).map(clamp_evidence),
                "code": entry.get("code").and_then(Value::as_str).map(clamp_evidence),
                "evidence": entry.get("evidence").and_then(Value::as_str).map(clamp_evidence),
                "decision": entry.get("decision").and_then(Value::as_str).map(clamp_evidence),
            })
        })
        .collect();
    let truncated = entries.len() - listed.len();
    (listed, undecided, truncated)
}

/// One document's slot of the persisted wrapper: its issue counts, the top
/// `kept` issues, its review list, and whether the whole thing is stale
/// against the document that is actually stored now.
fn compact_slot(slot: &Value, current_text: &str, kept: usize) -> Value {
    let issues: Vec<Value> = slot
        .get("report")
        .and_then(|report| report.get("issues"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out = issue_summary(&issues, kept);
    if let Some(object) = out.as_object_mut() {
        let (fabrications, undecided, truncated) = compact_fabrications(slot, kept);
        object.insert("stale".to_string(), stale_flag(slot, current_text));
        object.insert("undecided".to_string(), json!(undecided));
        object.insert("fabricationsTruncated".to_string(), json!(truncated));
        object.insert("fabrications".to_string(), Value::Array(fabrications));
    }
    out
}

/// `get_quality_report`'s payload.
///
/// An explicit unavailable-with-`reason` rather than a bare `null`, mirroring
/// `tools_quality::compact_salary_range`: the model must not have to guess
/// whether a missing report means "this posting was never generated for",
/// "it was generated on the fast path, which writes no report", or "the tool
/// failed".
fn compact_quality_report(record: Option<&AiGenerationRecord>) -> Value {
    let Some(record) = record else {
        return json!({ "available": false, "reason": "no_generation" });
    };
    let Ok(wrapper) = serde_json::from_str::<Value>(&record.quality_report) else {
        return json!({ "available": false, "reason": "no_report" });
    };
    let slots: [(&str, &str); 2] = [
        ("resume", record.resume_text.as_str()),
        ("coverLetter", record.cover_letter_text.as_str()),
    ];
    let present: Vec<(&str, &Value, &str)> = slots
        .iter()
        .filter_map(|(key, text)| wrapper.get(*key).map(|slot| (*key, slot, *text)))
        .collect();
    if present.is_empty() {
        return json!({ "available": false, "reason": "no_report" });
    }
    shrink_to_summary_cap(MAX_ISSUES, |kept| {
        let mut out = Map::new();
        out.insert("available".to_string(), json!(true));
        out.insert(
            "pipeline".to_string(),
            json!(wrapper
                .get("pipeline")
                .and_then(Value::as_str)
                .map(clamp_evidence)),
        );
        // COERCED to a number, never copied through: this is the one field of
        // the payload no `kept` step can shrink, so a wrapper whose
        // `generatedAt` is a megabyte-long string (a corrupt or hand-edited
        // blob — the column is opaque JSON the store never validates) exhausts
        // the shrink loop at `kept == 0` and hands the still-oversized body to
        // `neutralized_summary`'s hard `take(cap)`, which cuts it mid-JSON and
        // costs the model the whole answer. Anything non-numeric is `null`: an
        // unreadable timestamp is unknown, and the alternative is a clamped
        // string that reads like a date.
        out.insert(
            "generatedAt".to_string(),
            wrapper
                .get("generatedAt")
                .and_then(Value::as_u64)
                .map_or(Value::Null, |at| json!(at)),
        );
        for (key, slot, text) in &present {
            out.insert((*key).to_string(), compact_slot(slot, text, kept));
        }
        Value::Object(out)
    })
}

/// How many steps the draft itself shrinks in, once dropping issues is not
/// enough to fit [`shrink_to_summary_cap`]'s budget.
///
/// The draft is the one field of this payload that cannot simply be dropped —
/// it IS the answer, and the model has to be able to hand it to `save_resume`.
/// But it also cannot sit OUTSIDE the budget: [`RESUME_CAP`] is 8 000 chars,
/// and a draft whose characters escape (a raw control char serializes as
/// `\u00XX`) blows `SUMMARY_CAP` on its own, at which point
/// [`neutralized_summary`]'s hard `chars().take(cap)` cuts the JSON mid-string
/// and the model gets an unparseable body — i.e. loses the draft ENTIRELY
/// rather than losing its tail. Eighths are fine-grained enough that the
/// common case never reaches the second step and coarse enough that the loop
/// stays short.
const DRAFT_SHRINK_STEPS: usize = 8;

/// `run_quality_pipeline`'s payload: the draft as DATA plus a summary of the
/// report that was produced over it.
///
/// The draft is clamped to [`RESUME_CAP`] — the same slice every other agent
/// path shows the model (`tools::grounded_user_msg` fences the source résumé
/// at exactly this cap) — with `draftTruncated` saying so, because the text
/// the model would hand to the gated `save_resume` must not silently be a
/// prefix of what the pipeline actually validated. The full artifacts (every
/// metric, every uncapped span, the stage trail) belong to the quality-report
/// panel; a transcript gets the summary.
///
/// The single `kept` dial spends the budget in the order the model can most
/// afford to lose it: ISSUES first (a finding is advice; the report's COUNTS
/// survive whatever happens, and `truncated` says how many went), and only
/// once they are gone does the draft start shrinking — never the other way
/// round, and never a mid-string cut.
fn compact_pipeline_run(
    draft: &str,
    report: Option<&ContentReport>,
    stopped: Option<StoppedReason>,
    error: Option<&str>,
) -> Value {
    let issues: Vec<Value> = report
        .map(|report| {
            report
                .issues
                .iter()
                .map(|issue| serde_json::to_value(issue).unwrap_or(Value::Null))
                .collect()
        })
        .unwrap_or_default();
    shrink_to_summary_cap(DRAFT_SHRINK_STEPS + MAX_ISSUES, |kept| {
        let draft_steps = kept.min(DRAFT_SHRINK_STEPS);
        let draft_cap = RESUME_CAP * draft_steps / DRAFT_SHRINK_STEPS;
        let text = clamp_chars(draft, draft_cap);
        let draft_truncated = draft.chars().nth(draft_cap).is_some();
        let mut out = issue_summary(&issues, kept.saturating_sub(DRAFT_SHRINK_STEPS));
        if let Some(object) = out.as_object_mut() {
            object.insert("draft".to_string(), json!(text));
            object.insert("draftTruncated".to_string(), json!(draft_truncated));
            object.insert("validated".to_string(), json!(report.is_some()));
            object.insert(
                "stoppedReason".to_string(),
                stopped
                    .map(|reason| serde_json::to_value(reason).unwrap_or(Value::Null))
                    .unwrap_or(Value::Null),
            );
            // Clamped like every other untrusted span: a provider error can
            // echo prompt fragments back, and this one lands in the transcript.
            object.insert(
                "error".to_string(),
                json!(error.map(|message| clamp_chars(message, MESSAGE_CAP))),
            );
        }
        out
    })
}

// ── Handlers ───────────────────────────────────────────────────────────────

/// The trusted inputs all three tools resolve server-side: this run's own
/// résumé and its own posting, both already clamped to the caps the rest of
/// the agent layer uses.
struct RunInputs {
    resume: String,
    job_ad: String,
}

fn load_inputs(app: &AppHandle, ctx: &ToolContext) -> AppResult<RunInputs> {
    let resume = app
        .state::<DocumentStore>()
        .get(&ctx.resume_id)
        .ok_or_else(|| resume_not_found(&ctx.resume_id))?;
    let job_ad = job_text_for(app, &ctx.job_id).ok_or_else(|| job_not_found(&ctx.job_id))?;
    Ok(RunInputs {
        resume: clamped_resume_text(&resume.text),
        job_ad: clamped_job_text(&job_ad),
    })
}

/// Run the pipeline's OWN `analyze_job` stage — the stage struct, not a second
/// copy of its prompt — so the agent's analysis is the same artifact the
/// quality run produces, shares the same `resume_stage:analyze_job` cache
/// entry, and inherits the same "a structurally-valid but empty analysis is a
/// failed stage" check.
///
/// The cache key is seeded from the résumé + posting + target language
/// (`QualityCtx::new`), so an agent call and a real run collide only when all
/// three match — which is exactly when they would produce the same answer.
/// The language is detected from the posting because the agent, unlike
/// `resume_pipeline_run`, has no requested target language to pass.
async fn analyze_job_core(app: &AppHandle, ctx: &ToolContext) -> AppResult<Value> {
    let inputs = load_inputs(app, ctx)?;
    let completer = Completer::from_active(app)?;
    let cache = app.try_state::<KvCache>();
    let mut quality = QualityCtx::new(
        QualityInput {
            source_resume: &inputs.resume,
            job_ad: &inputs.job_ad,
            target_language: detect_locale_tag(&inputs.job_ad),
            top_requirements: &[],
            cover_letter: "",
            // The agent loop has no per-request reasoning effort of its own,
            // so this runs on the unscaled baseline — same choice
            // `research_company_handler` makes.
            effort: None,
            // INERT here: `job_id` is only read by the `draft` stage, which
            // streams, and this runs `analyze_job` alone. The agent's own id
            // is therefore safe to pass — `run_quality_pipeline_core`, which
            // DOES reach the draft, deliberately passes a throwaway one
            // instead, and says why.
            job_id: &ctx.job_id,
        },
        &completer,
        cache.as_deref(),
        RunDeadline::starting_now(AGENT_STAGE_DEADLINE),
        Arc::new(RunLedger::new()),
    );
    stages::AnalyzeJob.run(&mut quality).await?;
    Ok(neutralized_summary(&compact_job_analysis(
        &quality.analysis,
    )))
}

fn analyze_job_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    _args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move { analyze_job_core(&app, &ctx).await })
}

/// What resolving a run's posting to its stored generation found.
///
/// Three outcomes, not an `Option`, because the two empty cases are different
/// facts with different remedies and every caller so far has wanted to tell
/// them apart: a posting with no url can never be linked to a generation at
/// all, while a posting that simply has not been generated for yet names the
/// one action that would change that.
pub(crate) enum GenerationLookup {
    /// The cached posting carries no url, so nothing in `ai_generations` — an
    /// aggregate keyed by posting url — can be linked to it.
    UnlinkedJob,
    /// The posting has a url, but no generation is stored under it.
    NotGenerated,
    /// The stored generation aggregate for this posting. Boxed: the record is
    /// large and the other two variants are unit, so an unboxed enum would be
    /// record-sized everywhere it is returned.
    Found(Box<AiGenerationRecord>),
}

/// Resolve THIS run's job id to the generation stored for its posting.
///
/// The one place the rule lives: posting id → cached posting url (trimmed,
/// non-empty) → [`AiGenerationStore::find_for_job`], which tries the
/// normalized url before the raw one. It was written twice — here and in
/// `commands::agent`'s seed loader — and two copies of a resolution rule is
/// how the report tool and the flow that acts on it would come to disagree
/// about WHICH document is under review.
///
/// **Synchronous on purpose.** Both callers already own a `spawn_blocking`
/// closure (the `find_for_job` hit is SQLite), and returning a future here
/// would either add a second hop or drag each caller's own blocking work onto
/// the async worker. `job_meta_for` is an in-memory `Mutex<PostingsCache>`
/// lock, cheap on either pool (see this module's sibling note in
/// `tools_quality`).
///
/// POLICY BELONGS TO THE CALLER. This says what is stored, never whether it is
/// usable: `get_quality_report` reports on a generation of any size, while the
/// review flow refuses one it cannot seed whole
/// (`commands::agent::readable_generation_text`). A length rule pushed down
/// here would break the report tool for exactly the documents it is most
/// useful on.
pub(crate) fn generation_for_job(app: &AppHandle, job_id: &str) -> AppResult<GenerationLookup> {
    let meta = job_meta_for(app, job_id).ok_or_else(|| job_not_found(job_id))?;
    let job_url = meta.url.trim().to_string();
    if job_url.is_empty() {
        return Ok(GenerationLookup::UnlinkedJob);
    }
    Ok(app
        .try_state::<AiGenerationStore>()
        .and_then(|store| store.find_for_job(&job_url))
        .map_or(GenerationLookup::NotGenerated, |record| {
            GenerationLookup::Found(Box::new(record))
        }))
}

fn get_quality_report_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    _args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move {
        spawn_blocking_core(move || {
            match generation_for_job(&app, &ctx.job_id)? {
                // The aggregate is keyed by posting url; a posting the cache has
                // no url for has no report to find, and saying so beats "no
                // report".
                GenerationLookup::UnlinkedJob => Ok(neutralized_summary(
                    &json!({ "available": false, "reason": "unlinked_job" }),
                )),
                GenerationLookup::NotGenerated => {
                    Ok(neutralized_summary(&compact_quality_report(None)))
                }
                GenerationLookup::Found(record) => Ok(neutralized_summary(
                    &compact_quality_report(Some(record.as_ref())),
                )),
            }
        })
        .await
    })
}

/// The run deadline a tool-driven quality run gets: the SAME floor the IPC
/// command uses (`Budget::RESUME_QUALITY.run_timeout`, raised to the
/// effort-scaled deadline when that is larger), at baseline effort. The point
/// of matching is that a run started from the agent must not be killed by a
/// bound the identical run started from the UI would never hit — the deadline
/// exists to catch a run that never converges, not to make the agent's copy of
/// it cheaper.
fn quality_tool_deadline() -> Duration {
    run_deadline(Budget::RESUME_QUALITY, timeouts::quality_run_deadline(None))
}

/// The deadline half of `commands::resume_pipeline::hooks::RunHooks`, with no
/// emits and no run row: a tool-driven run has no `runId` to attach a
/// `pipeline:stage` trail to, and the agent's own `agent:step` narration is the
/// progress channel it does have.
///
/// The boundary check is not optional even so — `guard_deadline` inside the
/// JSON stages and the `repair` loop's own check cover the two multi-call
/// stages, but nothing else would stop a run that has already blown its clock
/// from paying for `draft`.
struct DeadlineHooks {
    ledger: Arc<RunLedger>,
    deadline: RunDeadline,
}

#[async_trait]
impl StageHooks for DeadlineHooks {
    async fn before(&self, stage: &StageInfo) -> AppResult<()> {
        match guard_deadline(&self.ledger, self.deadline) {
            // A stage that makes no provider call still runs — the same
            // decision, and the same reasoning, as
            // `commands::resume_pipeline::hooks::apply_stop`: stopping at a
            // boundary is about not paying for the NEXT call, and `validate` is
            // what turns an already-paid-for draft into something this tool may
            // report on. The reason is recorded either way (`guard_deadline`
            // writes it before returning), so `ledger.stopped()` still says
            // `run_timeout`.
            Err(_) if !stage.costs_a_call => Ok(()),
            other => other,
        }
    }

    async fn after(&self, _stage: &StageInfo, _outcome: StageOutcome) {}
}

/// Run the whole quality pipeline and hand back the draft + report as DATA.
///
/// **Nothing is persisted.** `commands::resume_pipeline::execute` writes a run
/// row and merges the document into `ai_generations`; this deliberately does
/// neither, so the only way a tool-produced résumé reaches the store is the
/// gated `save_resume` Write tool the user has to confirm.
///
/// Two consequences of that, both deliberate:
///
/// * **The draft streams under a FRESH job id.** `chat_stream`'s `finish()`
///   emits `job_complete` for whatever id it is handed, and the agent run's own
///   job id is the renderer's handle on the run — passing it here would mark
///   the agent run complete in the middle of a tool call. A throwaway id makes
///   those deltas addressed to a job nobody is filtering for, which is the
///   correct outcome for a stream nothing displays.
/// * **A run that produced a document is not a failure.** If the pipeline
///   errors but a draft exists, the draft comes back with the stop reason and
///   the (clamped) error alongside it — the same call
///   `hooks::terminal_state` makes for the command path. With no draft there is
///   nothing to report on, so the error is the answer.
async fn run_quality_pipeline_core(app: &AppHandle, ctx: &ToolContext) -> AppResult<Value> {
    let inputs = load_inputs(app, ctx)?;
    let completer = Completer::from_active(app)?;
    // A tool-side provider call spends money too — the same coarse daily
    // backstop `tools::complete_trusted` charges before its own call. The three
    // JSON stages charge again inside `complete_json`; over-charging a coarse
    // ceiling is the safe direction, and the alternative (a run of fifteen
    // calls admitted by nothing) is not.
    //
    // The `agent_run` limiter bucket is deliberately NOT re-acquired: the
    // enclosing `agent_run` command already holds one of its two slots for the
    // whole run, so taking a second from inside would let ONE user action
    // occupy the entire bucket, and `acquire_queued` could park this call
    // behind another 75-minute run while still holding that slot.
    app.state::<Arc<Limiter>>()
        .inner()
        .charge_provider_daily(completer.provider_id().as_str(), PROVIDER_DAILY_MAX)?;

    let cache = app.try_state::<KvCache>();
    let ledger = Arc::new(RunLedger::new());
    let deadline = RunDeadline::starting_now(quality_tool_deadline());
    let hooks = DeadlineHooks {
        ledger: Arc::clone(&ledger),
        deadline,
    };
    let stream_job = new_job_id();
    let mut quality = QualityCtx::new(
        QualityInput {
            source_resume: &inputs.resume,
            job_ad: &inputs.job_ad,
            target_language: detect_locale_tag(&inputs.job_ad),
            top_requirements: &[],
            cover_letter: "",
            effort: None,
            job_id: &stream_job,
        },
        &completer,
        cache.as_deref(),
        deadline,
        Arc::clone(&ledger),
    );

    let outcome = quality_pipeline().run_hooked(&mut quality, &hooks).await;
    let error = match outcome {
        Ok(()) => None,
        Err(e) if quality.draft.trim().is_empty() => return Err(e),
        Err(e) => Some(e.to_string()),
    };
    Ok(neutralized_summary(&compact_pipeline_run(
        &quality.draft,
        quality.report.as_ref(),
        ledger.stopped(),
        error.as_deref(),
    )))
}

fn run_quality_pipeline_handler(
    app: &AppHandle,
    ctx: &ToolContext,
    _args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    let ctx = ctx.clone();
    Box::pin(async move { run_quality_pipeline_core(&app, &ctx).await })
}

// ── Registration ───────────────────────────────────────────────────────────

/// The empty schema all three tools share: every input is resolved server-side
/// from the trusted [`ToolContext`], so there is nothing for a prompt-injected
/// posting to steer — the same shape `research_company` and `lookup_salary`
/// already declare.
fn no_arguments_schema() -> Value {
    json!({ "type": "object", "properties": {} })
}

/// One-shot job analysis. Cheap enough for a general whitelist: ONE stage, one
/// provider round-trip, bounded by [`AGENT_STAGE_DEADLINE`].
pub(crate) fn analyze_job_tool() -> AgentTool {
    AgentTool {
        name: "analyze_job",
        description: "Analyse this run's own job posting and return what it asks for: role title, \
                      seniority, must-have and nice-to-have requirements, responsibilities, domain \
                      keywords and red flags. Read-only. Takes no arguments — it always targets \
                      this run's own posting."
            .to_string(),
        schema: no_arguments_schema(),
        kind: ToolKind::Read,
        handler: analyze_job_handler,
    }
}

/// The persisted quality report for this run's posting. No provider call at
/// all — a store read.
pub(crate) fn get_quality_report_tool() -> AgentTool {
    AgentTool {
        name: "get_quality_report",
        description: "Read the saved quality report for this run's own job posting: how many \
                      Critical and Warning findings the last check produced, the findings \
                      themselves, and any flagged claims still awaiting a Keep or Remove \
                      decision. Read-only, no generation. A `stale` field of true means the \
                      report describes an EARLIER version of the saved document, so treat its \
                      findings as historical; `available: false` reports why there is nothing to \
                      read. Takes no arguments."
            .to_string(),
        schema: no_arguments_schema(),
        kind: ToolKind::Read,
        handler: get_quality_report_handler,
    }
}

/// The whole quality pipeline, returning the draft + report as DATA.
///
/// **Expensive, and reachable from exactly ONE flow.** Fifteen provider calls
/// against a 75-minute floor cannot fit [`Budget::AGENT_PREP`]'s 360 s
/// `step_timeout`, which the controller races every tool call against — a prep
/// run that called this would die at [`StoppedReason::Timeout`] with its drafts
/// unsaved. Its caller is the `improve_resume` flow
/// ([`crate::agent::flows::FLOWS`]), whose `AgentFlow` carries a `Budget`
/// (`AGENT_IMPROVE`) with a 90-minute step clock sized on the arithmetic
/// asserted at the top of this module; the whitelist it reaches through is
/// [`super::tools::improve_resume_tools`]. `agent::tools::test`'s
/// `the_quality_pipeline_tool_is_absent_from_a_flow_whose_step_cannot_cover_it`
/// and its `…_is_present_in_the_flow_whose_step_can_cover_it` twin pin both
/// directions off that arithmetic rather than off a name list.
pub(crate) fn run_quality_pipeline_tool() -> AgentTool {
    AgentTool {
        name: "run_quality_pipeline",
        description: "Run the full staged résumé pipeline (analyse the posting, map the \
                      candidate's evidence, plan a strategy, draft, check, repair) for this run's \
                      own posting and return the finished draft plus a summary of what the check \
                      found. Read-only: it saves NOTHING — pass the returned draft to save_resume \
                      if the user should be offered it. Expensive and slow; call it at most once. \
                      Takes no arguments."
            .to_string(),
        schema: no_arguments_schema(),
        kind: ToolKind::Read,
        handler: run_quality_pipeline_handler,
    }
}

#[cfg(test)]
mod test;
