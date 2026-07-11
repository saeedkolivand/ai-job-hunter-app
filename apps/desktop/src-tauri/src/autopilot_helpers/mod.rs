use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use crate::agent::flows::AUTOPILOT_NOTE_SYSTEM;
use crate::agent::tools::{fenced, JOB_CAP, RESUME_CAP};
use crate::autopilot::{Autopilot, AutopilotTarget, FoundJob};
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, SCRAPE_ITEM, SCRAPE_PROGRESS};
use crate::limits::{Limiter, PROVIDER_DAILY_MAX};
use crate::pipeline::Completer;
use crate::scraping::{BoardScrapeSummary, BoardSearchInput, JobPosting, ScraperEngine};

/// Scrape job postings from one or more boards for an autopilot run. Returns the
/// postings **and** the per-board diagnostics, so the run can explain a zero
/// result (e.g. an aggregator error or a skipped board) instead of silently
/// showing "found 0".
pub async fn autopilot_scrape(
    engine: &ScraperEngine,
    target: &AutopilotTarget,
    job_id: &str,
    app: &AppHandle,
) -> AppResult<(Vec<JobPosting>, Vec<BoardScrapeSummary>)> {
    let input = BoardSearchInput {
        query: target.query.clone(),
        location: target.location.clone(),
        // Autopilot expresses its target in pages, so let the page budget bind and
        // set the central item cap to the maximum (never caps autopilot to 0).
        amount: 100,
        pages: target.pages,
        date_filter: target.date_filter.clone(),
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        // Location forwarding (trust PR F): the persisted `AutopilotTarget` carries
        // only `location` (free text) + `country_code` — verified: it has no
        // lat/lon/radius fields to forward (the wizard never captures them). Both
        // are forwarded here, so `input.location_spec()` yields a spec with a city
        // + country, which drives the engine's central location post-filter for
        // non-supporting boards AND the aggregator's market routing on autopilot
        // runs. Wiring lat/lon/radius needs the wizard + `AutopilotTargetSchema` to
        // capture + persist them first (stage-2 / follow-up).
        country_code: target.country_code.clone(),
        latitude: None,
        longitude: None,
        radius_km: None,
        // Autopilot has no per-company target, so it passes no explicit
        // companies here. Company-scoped ATS boards (greenhouse, lever, ashby,
        // smartrecruiters, recruitee, personio, workable) don't no-op on that —
        // the engine (`scraping/engine/mod.rs`) falls back to the curated
        // `ats_seed` directory for them when the list is empty. Non-company
        // boards are unaffected.
        companies: Vec::new(),
    };

    let app_progress = app.clone();
    let job_id_progress = job_id.to_string();
    let on_progress: std::sync::Arc<dyn Fn(f32) + Send + Sync> =
        std::sync::Arc::new(move |p: f32| {
            emit_event(
                &app_progress,
                SCRAPE_PROGRESS,
                serde_json::json!({ "jobId": job_id_progress, "progress": p }),
            );
        });

    let app_item = app.clone();
    let job_id_item = job_id.to_string();
    let on_item: std::sync::Arc<dyn Fn(JobPosting) + Send + Sync> =
        std::sync::Arc::new(move |item: JobPosting| {
            emit_event(
                &app_item,
                SCRAPE_ITEM,
                serde_json::json!({ "jobId": job_id_item, "item": item }),
            );
        });

    let result = engine
        .scrape_boards(
            &target.boards,
            input,
            job_id.to_string(),
            Some(on_progress),
            Some(on_item),
        )
        .await;

    // Log any skipped or errored boards so operators can diagnose unexpected empty
    // runs, AND return the summaries so the run can surface *why* it found zero.
    result
        .map(|(postings, summaries)| {
            for s in &summaries {
                if let Some(ref reason) = s.skipped {
                    log::warn!(
                        "[autopilot] board '{}' skipped (reason='{}')",
                        s.board,
                        reason
                    );
                }
                if let Some(ref err) = s.error {
                    log::warn!("[autopilot] board '{}' failed (error='{}')", s.board, err);
                }
            }
            (postings, summaries)
        })
        .map_err(AppError::from)
}

/// Max length of a single board's sanitized reason in the user-visible step log.
/// Diagnostics are a hint, not a full error dump — keep them short and bounded.
const MAX_REASON_LEN: usize = 200;

/// Redact a board's raw error/skip text before it reaches the user-visible step
/// log. The text can originate from an upstream `e.to_string()` and may carry
/// absolute filesystem paths, full URLs, or request internals — emitting those raw
/// violates the repo path-privacy rule. This collapses each such fragment to a
/// neutral placeholder, normalises whitespace, and caps the length.
///
/// Deliberately conservative: it errs toward over-redaction (a placeholder is
/// always safe) and keeps the high-level message (e.g. `"429 Too Many Requests"`,
/// `"needs-login"`, `"network timeout"`) intact. Pure + unit-testable.
fn sanitize_reason(reason: &str) -> String {
    let mut out = String::with_capacity(reason.len());

    for token in reason.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&redact_token(token));
    }

    if out.chars().count() > MAX_REASON_LEN {
        let truncated: String = out.chars().take(MAX_REASON_LEN).collect();
        out = format!("{truncated}…");
    }
    out
}

/// Classify a single whitespace-delimited token and replace it with a neutral
/// placeholder when it looks like a path / URL / request internal; otherwise keep
/// it verbatim. Whitespace-token granularity keeps the surrounding human message
/// (`"failed: <path>"` → `"failed: <path-redacted>"`) readable.
pub(crate) fn redact_token(token: &str) -> String {
    // Strip surrounding punctuation (quotes, parens, braces, backticks, trailing
    // `.,:;|`) so a token like `(C:\Users\x)`, `` `https://…` ``, or `{path}` still
    // matches, then re-attach it.
    let trimmed = token.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\''
                | '`'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '|'
                | ','
                | ';'
                | ':'
        )
    });

    let is_url = trimmed.contains("://");
    // Windows absolute path: drive letter + `:\` or `:/` (e.g. `C:\Users\…`).
    let mut chars = trimmed.chars();
    let is_windows_path = matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(c), Some(':'), Some('\\' | '/')) if c.is_ascii_alphabetic()
    );
    // Unix absolute path: starts with `/` and has a further separator (a lone `/`
    // or a fraction like `1/2` is not a path).
    let is_unix_path = trimmed.starts_with('/') && trimmed[1..].contains('/');
    // Drive-less home/user path: a fragment like `Users\alice\…` or `home/alice/…`
    // that lost its drive letter / leading `/` (common in unwound error chains).
    // Lowercased substring match catches both separators and any case.
    let lower = trimmed.to_ascii_lowercase();
    let is_homeish_path =
        lower.contains("users\\") || lower.contains("users/") || lower.contains("home/");

    // Standalone credential assignment (`app_key=…`, `token=…`, `secret=…`) emitted
    // OUTSIDE a full `://` URL. The `marker=` shape (the `=` makes it an assignment)
    // is what flags it — matching the bare word would over-redact `keyword` / a
    // prose "token". The `://` branch below runs first, so a full
    // `https://…?app_key=…` still collapses to `<url-redacted>`, not this.
    // The JSON-field shape (`"api_key":"value"` → brace/quote-trimmed to
    // `api_key":"value`) is also matched via the `key":` / `token":` sub-strings
    // so structured log lines (e.g. `{"api_key":"sk-…"}`) don't bypass redaction.
    let is_credential = [
        // `key=` (substring match) subsumes the `*key=` variants — `app_key=`,
        // `apikey=`, `api_key=` all CONTAIN it — so don't re-add those here.
        "key=",
        "app_id=",
        "secret=",
        "token=",
        "password=",
        "pwd=",
        "auth=",
        // JSON field shape: `"api_key":"value"` after brace/quote trimming becomes
        // `api_key":"value`; the `key":` sub-string flags it. `key":` subsumes
        // `apikey":`, `api_key":`, etc. The whole token is replaced, matching the
        // same behaviour as the `=` variants above.
        "key\":",
        "secret\":",
        "token\":",
        "password\":",
        "auth\":",
    ]
    .iter()
    .any(|marker| lower.contains(marker));

    // Bare IPv4 / host:port — leaks the user's network surroundings. Require an
    // embedded `.` AND either a trailing `:<digits>` port or an all-numeric dotted
    // IPv4, so `429:`, `12:34` timestamps, and plain integers stay untouched.
    let is_host_port = trimmed.contains('.') && {
        let segs: Vec<&str> = trimmed.split('.').filter(|s| !s.is_empty()).collect();
        let dotted_ipv4 = segs.len() == 4
            && segs
                .iter()
                .all(|seg| seg.chars().all(|c| c.is_ascii_digit()));
        let host_with_port = trimmed.rsplit_once(':').is_some_and(|(host, port)| {
            host.contains('.') && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())
        });
        dotted_ipv4 || host_with_port
    };

    // Email address: `local@domain.tld` — common in crash logs that include
    // contact profile data, apply-email generation output, or error context from
    // profile imports. Require a non-empty local part and a domain bearing a `.`
    // so bare `@` symbols and TLD-only fragments are left untouched.
    let is_email = trimmed
        .split_once('@')
        .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'));

    if is_url {
        token.replace(trimmed, "<url-redacted>")
    } else if is_credential {
        token.replace(trimmed, "<credential-redacted>")
    } else if is_windows_path || is_unix_path || is_homeish_path {
        token.replace(trimmed, "<path-redacted>")
    } else if is_host_port {
        token.replace(trimmed, "<host-redacted>")
    } else if is_email {
        token.replace(trimmed, "<email-redacted>")
    } else {
        token.to_string()
    }
}

/// Turn the per-board scrape summaries into a single human-readable reason string
/// explaining why a run may have come up short — `"<board>: <reason>"` for each
/// board that errored, was skipped, or kept only a partial (truncated) harvest,
/// joined with `"; "`. Precedence per board: `error` > `skipped` > `truncated` —
/// an outright error is the most actionable signal, a truncated harvest the
/// least (it DID return rows). The per-board reason is run through
/// [`sanitize_reason`] so absolute paths / URLs never leak into the user-visible
/// step log. Returns an empty string when no board reported a problem. Pure +
/// unit-testable.
pub(crate) fn scrape_diagnostics(summaries: &[BoardScrapeSummary]) -> String {
    summaries
        .iter()
        .filter_map(|s| {
            s.error
                .as_deref()
                .or(s.skipped.as_deref())
                .or(s.truncated.as_deref())
                .map(|reason| format!("{}: {}", s.board, sanitize_reason(reason)))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

// ── AI notes (Phase 4, opt-in, headless, read-only) ───────────────────────────
//
// SECURITY / SAFETY (co-reviewed by tauri-security-reviewer + performance-profiler):
// A scheduled run has no live user, so this path is READ-ONLY by construction — a
// plain single-shot `Completer::complete` per top match, NO tools / NO Write / NO
// agent loop / NO confirm gate. It can never mutate or apply, and its ONLY spend is
// bounded `complete()` calls. The résumé + job text are fenced as untrusted DATA in
// the user turn; `AUTOPILOT_NOTE_SYSTEM` is the sole trusted instruction (OWASP
// LLM01). Cost is triple-bounded — the top-N call ceiling, the shared per-provider
// daily charge, and the run's cancellation token — so the per-tick fan-out cannot
// blow up.

/// Hard ceiling on provider calls per AI-notes run. Bounds the scheduled per-tick
/// fan-out (cost + latency): even a run that finds hundreds of jobs makes at most
/// this many [`Completer::complete`] calls, each charged against the shared
/// per-provider daily ceiling. Intentionally small — this runs unattended.
pub(crate) const ASSISTANT_NOTES_MAX: usize = 3;

/// Char cap on a stored note — defense-in-depth on output size. The system prompt
/// already asks for 2–4 sentences and the provider layer exposes no max-tokens
/// knob, so this bounds a pathological over-long completion before it is persisted.
const NOTE_CAP: usize = 800;

/// Temperature for the note completion — low, for concise, grounded prose.
const NOTE_TEMPERATURE: f64 = 0.3;

/// MEDIUM-3: hard wall-clock ceiling on the WHOLE AI-notes step, independent of
/// `cancel` — the notes step runs BEFORE `record_run`/`on_new_jobs`, so a slow or
/// hung provider must never delay the user-facing "new jobs" notification by more
/// than this. Generous for ≤ [`ASSISTANT_NOTES_MAX`] sequential completions under
/// normal latency; a hard backstop against a hanging request, not a tuning knob.
const NOTES_STEP_TIMEOUT: Duration = Duration::from_secs(45);

/// Pure gate: should AI notes run for this record at all? Opt-in must be on AND a
/// résumé must exist to ground the "why it fits" reasoning. Provider availability is
/// resolved separately (it needs the app handle). Unit-tested.
fn notes_enabled(assistant: bool, resume: Option<&str>) -> bool {
    assistant && resume.map(str::trim).is_some_and(|r| !r.is_empty())
}

/// Build the grounded, fenced user turn for one note: the résumé — ALREADY fenced
/// by the caller (LOW-6: identical across every job in a run, so it is fenced ONCE
/// before the loop, not re-fenced per job) — plus the job (title + company +
/// description) as untrusted DATA, capped with the SAME fence helper + cap the
/// agent tools use (declared once in `agent::tools`). The system prompt is the only
/// trusted instruction source (OWASP LLM01). Pure + unit-tested.
fn note_user_msg(
    resume_fence: &str,
    title: &str,
    company: &str,
    description: Option<&str>,
) -> String {
    let (title, company) = (title.trim(), company.trim());
    // Only join with " at " when BOTH sides are present — otherwise a missing
    // title/company would render a bare " at " (or " at Acme"/"Acme at ") instead
    // of just the one part that exists, or nothing at all if neither does.
    let header = match (title.is_empty(), company.is_empty()) {
        (false, false) => format!("{title} at {company}"),
        (false, true) => title.to_string(),
        (true, false) => company.to_string(),
        (true, true) => String::new(),
    };
    let job_blob = format!("{header}\n\n{}", description.unwrap_or("").trim());
    format!(
        "{resume_fence}\n\n{}",
        fenced("job_posting", &job_blob, JOB_CAP)
    )
}

/// The notes loop's provider seam — mirrors `agent::controller::AgentEnv`: the ONE
/// external call (a provider completion + the shared daily-budget charge) sits
/// behind a trait so the loop's pure control flow (top-N cap / cancellation /
/// daily-ceiling short-circuit / prior-url skip) is unit-testable with a fake —
/// this crate has no way to fake a live `Box<dyn AiProvider>`. Prod wiring is
/// [`LiveNoteEnv`].
#[async_trait]
trait NoteEnv: Send + Sync {
    /// Run one note completion. Mirrors [`Completer::complete`]'s signature (a
    /// fixed system + a fenced user turn + temperature).
    async fn complete(&self, system: &str, user: &str, temperature: f64) -> AppResult<String>;
    /// Charge one call against the shared per-provider daily ceiling. MEDIUM-5:
    /// background notes intentionally draw from the SAME `PROVIDER_DAILY_MAX`
    /// counter as interactive AI — no parallel budget architecture — which is
    /// acceptable because a run charges at most [`ASSISTANT_NOTES_MAX`] (generous
    /// cap) against it.
    fn charge_daily(&self) -> AppResult<()>;
}

/// Production [`NoteEnv`]: the resolved [`Completer`] + the shared [`Limiter`]. Reads
/// the provider id straight off `completer.provider_id()` (same pattern as
/// `agent::tools`'s tool-call charge) so this module never needs to name the
/// `ProviderId` type itself.
struct LiveNoteEnv<'a> {
    completer: &'a Completer,
    limiter: Arc<Limiter>,
}

#[async_trait]
impl NoteEnv for LiveNoteEnv<'_> {
    async fn complete(&self, system: &str, user: &str, temperature: f64) -> AppResult<String> {
        self.completer
            .complete(system, user, Some(temperature))
            .await
    }
    fn charge_daily(&self) -> AppResult<()> {
        self.limiter
            .charge_provider_daily(self.completer.provider_id().as_str(), PROVIDER_DAILY_MAX)
    }
}

/// The pure(ish) notes loop: for each job whose `url` is genuinely new (∉
/// `prior_urls`), request one note through `env`, honoring — every iteration — the
/// top-N call ceiling, the run's cancellation token, and the shared daily-ceiling
/// charge. Split from [`generate_assistant_notes`] (mirroring
/// `agent::controller::run_agent`'s split from its `AgentEnv`) so a fake `env`
/// unit-tests the control flow directly, without a live provider.
///
/// HIGH-1: cancellation is raced against the in-flight completion itself via
/// `tokio::select!`, not just checked between iterations — Stop/cancel interrupts a
/// slow call immediately instead of waiting out the provider's own timeout
/// (120s cloud / 300s Ollama).
async fn run_notes_loop(
    env: &dyn NoteEnv,
    resume_fence: &str,
    found_jobs: &mut [FoundJob],
    prior_urls: &HashSet<&str>,
    cancel: &CancellationToken,
) -> usize {
    // `calls` bounds *provider calls* (the real cost) to ≤ ASSISTANT_NOTES_MAX,
    // independent of how many succeed; `generated` counts notes actually stored.
    let mut calls = 0usize;
    let mut generated = 0usize;
    for job in found_jobs.iter_mut() {
        if calls >= ASSISTANT_NOTES_MAX {
            break; // top-N call ceiling reached — hard cost bound
        }
        if prior_urls.contains(job.url.as_str()) {
            continue; // re-surfaced: keeps its prior note via merge, don't re-pay
        }
        if cancel.is_cancelled() {
            break; // run cancelled (tray/UI) between iterations — stop spending
        }
        // Charge the shared per-provider daily ceiling BEFORE the call; once it is
        // hit, stop generating notes (the discovery run still completes normally).
        if let Err(e) = env.charge_daily() {
            log::info!("[autopilot] AI notes stopped at daily ceiling: {e}");
            break;
        }
        calls += 1;
        let user = note_user_msg(
            resume_fence,
            &job.title,
            &job.company,
            job.description.as_deref(),
        );
        // HIGH-1: race the completion against cancellation so Stop interrupts an
        // IN-FLIGHT call too — the `is_cancelled()` check above only catches
        // cancellation BETWEEN iterations.
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            res = env.complete(AUTOPILOT_NOTE_SYSTEM, &user, NOTE_TEMPERATURE) => res,
        };
        match result {
            Ok(text) => {
                let note: String = text.trim().chars().take(NOTE_CAP).collect();
                if !note.is_empty() {
                    job.assistant_notes = Some(note);
                    generated += 1;
                }
            }
            // Best-effort: one job's failure never aborts the rest or the run.
            Err(e) => log::warn!("[autopilot] AI note generation failed: {e}"),
        }
    }
    log::info!(
        "[autopilot] AI notes: generated {generated} in {calls} call(s) (max {ASSISTANT_NOTES_MAX})"
    );
    generated
}

/// Generate a short AI-reasoned note for up to [`ASSISTANT_NOTES_MAX`] of the top
/// NEW matches, storing each on `FoundJob.assistant_notes`. Returns how many notes
/// were generated (drives the notification summary).
///
/// READ-ONLY: a plain single-shot [`Completer::complete`] per job — no tools, no
/// Write, no agent loop, no confirm gate. Cost is triple-bounded — the top-N *call*
/// ceiling, the shared per-provider daily charge (stop on exceed; the discovery run
/// still completes), and the run's cancellation token (stop immediately, even
/// mid-call — see [`run_notes_loop`]). Only genuinely-new matches (`url` ∉
/// `prior_urls`) are annotated: a re-surfaced job keeps its earlier note for free
/// via the store merge, so re-generating would just burn a call whose result the
/// merge discards — in steady state (nothing new) this makes ZERO provider calls.
/// `completer` is `None` when the caller's [`Completer::resolve`] found no usable
/// provider — swallowed here: the discovery run always succeeds; notes are
/// best-effort enrichment. Provider/limiter resolution (and the "no usable
/// provider (reason)" log, so a user can debug why notes never run) lives in the
/// L3 caller (`commands::autopilot::autopilot_run`, which already holds the
/// `AppHandle`) — this L2 module takes them already-resolved, never reaches up
/// into `crate::commands`, and does not re-log here (the caller already did,
/// whenever `assistant` is on).
pub(crate) async fn generate_assistant_notes(
    completer: Option<&Completer>,
    limiter: Arc<Limiter>,
    autopilot: &Autopilot,
    found_jobs: &mut [FoundJob],
    prior_urls: &HashSet<&str>,
    cancel: &CancellationToken,
) -> usize {
    if !notes_enabled(autopilot.assistant, autopilot.resume_text.as_deref()) {
        return 0;
    }
    let resume = autopilot.resume_text.as_deref().unwrap_or("");

    // No log here — the L3 caller already logged the resolve failure (with the
    // reason) before passing `None` down; logging again here would just duplicate
    // that line for every assistant-enabled run with a bad/missing provider.
    let Some(completer) = completer else {
        return 0;
    };
    let env = LiveNoteEnv { completer, limiter };

    // LOW-6: fence the résumé ONCE — it's identical for every job in this run,
    // unlike the per-job title/company/description `note_user_msg` re-fences.
    let resume_fence = fenced("candidate_resume", resume, RESUME_CAP);

    // MEDIUM-3: bound the WHOLE step by wall clock, independent of `cancel` — a
    // slow/hung provider must never delay the "new jobs" notification (fired right
    // after this returns in `autopilot_run`) unboundedly. Any notes already stored
    // on `found_jobs` before the timeout fires are KEPT (the loop mutates in place
    // synchronously before each next await, so a dropped future loses no already-
    // applied mutation); only this call's returned count may undercount in that
    // rare case, which under-reports "N with AI notes" in the notification — a
    // cosmetic tradeoff for a notification that is never blocked unboundedly.
    match tokio::time::timeout(
        NOTES_STEP_TIMEOUT,
        run_notes_loop(&env, &resume_fence, found_jobs, prior_urls, cancel),
    )
    .await
    {
        Ok(generated) => generated,
        Err(_) => {
            log::warn!("[autopilot] AI notes step exceeded {NOTES_STEP_TIMEOUT:?}; stopping");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use super::*;
    use crate::scraping::BoardScrapeSummary;

    fn summary(board: &str, error: Option<&str>, skipped: Option<&str>) -> BoardScrapeSummary {
        summary_full(board, error, skipped, None)
    }

    fn summary_full(
        board: &str,
        error: Option<&str>,
        skipped: Option<&str>,
        truncated: Option<&str>,
    ) -> BoardScrapeSummary {
        BoardScrapeSummary {
            board: board.into(),
            count: 0,
            error: error.map(String::from),
            skipped: skipped.map(String::from),
            truncated: truncated.map(String::from),
            note: None,
        }
    }

    #[test]
    fn empty_slice_returns_empty_string() {
        assert_eq!(scrape_diagnostics(&[]), "");
    }

    #[test]
    fn board_with_error_and_no_skip_shows_error() {
        let s = summary("linkedin", Some("429 Too Many Requests"), None);
        let diag = scrape_diagnostics(&[s]);
        assert!(
            diag.contains("linkedin"),
            "board name must appear; got: {diag}"
        );
        assert!(
            diag.contains("429 Too Many Requests"),
            "error text must appear; got: {diag}"
        );
    }

    #[test]
    fn exact_format_is_board_colon_space_reason() {
        // Pin the exact `"<board>: <reason>"` format the impl produces.
        // If the separator or spacing ever changes this test catches it immediately.
        assert_eq!(
            scrape_diagnostics(&[summary("aggregator", Some("network timeout"), None)]),
            "aggregator: network timeout"
        );
    }

    #[test]
    fn skipped_only_board_appears_in_output() {
        // A board that was skipped (no error) must still surface its reason.
        let diag = scrape_diagnostics(&[summary("glassdoor", None, Some("needs-login"))]);
        assert!(
            diag.contains("needs-login"),
            "skipped reason must appear; got: {diag}"
        );
    }

    #[test]
    fn truncated_only_board_appears_in_output() {
        // A paginated board (stage 1) that kept only a partial harvest — no
        // `error`, no `skipped` — must still surface its truncation reason, not
        // be silently treated as a clean run.
        let s = summary_full(
            "arbeitnow",
            None,
            None,
            Some("page 2 of 5 failed: HTTP 429"),
        );
        assert_eq!(
            scrape_diagnostics(&[s]),
            "arbeitnow: page 2 of 5 failed: HTTP 429"
        );
    }

    #[test]
    fn error_takes_precedence_over_skipped_when_both_are_set() {
        // `error` is checked first in the `or` chain; `skipped` must be shadowed.
        let s = summary("aggregator", Some("network timeout"), Some("needs-login"));
        let diag = scrape_diagnostics(&[s]);
        assert!(
            diag.contains("network timeout"),
            "error must win over skipped; got: {diag}"
        );
        assert!(
            !diag.contains("needs-login"),
            "skipped must be suppressed when error is set; got: {diag}"
        );
    }

    #[test]
    fn board_with_neither_error_nor_skipped_contributes_nothing() {
        let clean = summary("indeed", None, None);
        assert_eq!(scrape_diagnostics(&[clean]), "");
    }

    #[test]
    fn multiple_errored_boards_are_joined_with_semicolon() {
        let summaries = vec![
            summary("linkedin", Some("rate-limited"), None),
            summary("indeed", None, Some("needs-login")),
        ];
        let diag = scrape_diagnostics(&summaries);
        // Both boards must appear; they are joined with "; ".
        assert!(
            diag.contains("linkedin"),
            "first board missing; got: {diag}"
        );
        assert!(diag.contains("indeed"), "second board missing; got: {diag}");
        assert!(
            diag.contains("; "),
            "boards must be joined with \"; \"; got: {diag}"
        );
    }

    #[test]
    fn clean_board_mixed_with_errored_does_not_appear_in_output() {
        let summaries = vec![
            summary("linkedin", Some("timeout"), None),
            summary("remotive", None, None), // clean — must not appear
        ];
        let diag = scrape_diagnostics(&summaries);
        assert!(
            !diag.contains("remotive"),
            "clean board must not appear; got: {diag}"
        );
        assert!(
            !diag.contains("; "),
            "single problem board must not add trailing separator; got: {diag}"
        );
    }

    #[test]
    fn diagnostics_redact_absolute_paths_and_urls() {
        // A raw error from an upstream `e.to_string()` carrying an absolute
        // Windows path, a Unix path, and a full URL must NOT leak any of them into
        // the user-visible step log (repo path-privacy rule).
        let raw = "failed to open C:\\Users\\alice\\secret.json or /home/alice/cfg via https://api.example.com/v1/jobs?token=abc";
        let diag = scrape_diagnostics(&[summary("aggregator", Some(raw), None)]);

        assert!(
            !diag.contains("C:\\Users\\alice"),
            "windows path leaked; got: {diag}"
        );
        assert!(
            !diag.contains("/home/alice"),
            "unix path leaked; got: {diag}"
        );
        assert!(
            !diag.contains("https://api.example.com"),
            "url leaked; got: {diag}"
        );
        assert!(
            !diag.contains("token=abc"),
            "url query/secret leaked; got: {diag}"
        );
        // The high-level message + placeholders survive.
        assert!(
            diag.contains("aggregator:"),
            "board prefix missing; got: {diag}"
        );
        assert!(
            diag.contains("failed to open"),
            "message dropped; got: {diag}"
        );
        assert!(
            diag.contains("<path-redacted>"),
            "path placeholder missing; got: {diag}"
        );
        assert!(
            diag.contains("<url-redacted>"),
            "url placeholder missing; got: {diag}"
        );
    }

    #[test]
    fn sanitize_reason_caps_overlong_input() {
        let long = "x".repeat(500);
        let out = sanitize_reason(&long);
        assert!(
            out.chars().count() <= super::MAX_REASON_LEN + 1, // +1 for the ellipsis
            "sanitized reason must be length-capped; got {} chars",
            out.chars().count()
        );
        assert!(
            out.ends_with('…'),
            "overlong input must be truncated with …"
        );
    }

    #[test]
    fn sanitize_reason_keeps_benign_messages_verbatim() {
        // No paths/URLs → unchanged (modulo whitespace normalisation).
        assert_eq!(
            sanitize_reason("429 Too Many Requests"),
            "429 Too Many Requests"
        );
        assert_eq!(sanitize_reason("needs-login"), "needs-login");
    }

    #[test]
    fn redact_host_port_ipv4_and_driveless_home_path() {
        // host:port, dotted IPv4 (with and without port), and a drive-less
        // `Users\…` / `home/…` fragment must all be redacted — they leak the
        // user's network/home surroundings (hard path-privacy rule).
        for raw in [
            "connect to api.adzuna.com:443 failed",
            "connect to 93.184.216.34:443 failed",
            "peer 93.184.216.34 unreachable",
            "open Users\\alice\\cache denied",
            "open home/alice/cache denied",
        ] {
            let out = sanitize_reason(raw);
            assert!(
                !out.contains("api.adzuna.com")
                    && !out.contains("93.184.216.34")
                    && !out.contains("alice"),
                "sensitive fragment leaked from {raw:?}; got: {out}"
            );
            assert!(
                out.contains("<host-redacted>") || out.contains("<path-redacted>"),
                "a redaction placeholder must appear for {raw:?}; got: {out}"
            );
        }
    }

    #[test]
    fn over_redaction_control_keeps_codes_timestamps_and_numbers() {
        // Guard against the widened rules eating benign tokens: a trailing-colon
        // status (`429:`), a `HH:MM` timestamp (`12:34`), a plain integer, and a
        // plain word must ALL survive verbatim (no embedded `.` / no path markers).
        let out = sanitize_reason("429: 12:34 503 timeout reached");
        assert_eq!(
            out, "429: 12:34 503 timeout reached",
            "benign codes/timestamps/numbers must not be redacted; got: {out}"
        );
        // A dotted version with no port and a non-IPv4 shape (3 segments) is left
        // alone too — not a host:port and not a 4-octet IPv4.
        assert_eq!(sanitize_reason("v1.2.3"), "v1.2.3");
    }

    #[test]
    fn redact_standalone_credential_assignments() {
        // A bare `marker=value` credential token emitted OUTSIDE a `://` URL must
        // be redacted — the secret value must not survive (defense-in-depth).
        for raw in ["bad app_key=abc123 request", "auth token=deadbeef rejected"] {
            let out = sanitize_reason(raw);
            assert!(
                out.contains("<credential-redacted>"),
                "credential placeholder must appear for {raw:?}; got: {out}"
            );
            assert!(
                !out.contains("abc123") && !out.contains("deadbeef"),
                "secret value leaked from {raw:?}; got: {out}"
            );
        }
    }

    #[test]
    fn full_url_with_credential_query_still_wins_url_branch() {
        // A full `https://…?app_key=…` token contains `://`, so the URL branch must
        // win and collapse the WHOLE token to <url-redacted> — not <credential-redacted>.
        let out = sanitize_reason("GET https://api.adzuna.com/v1/jobs?app_key=secret failed");
        assert!(
            out.contains("<url-redacted>"),
            "URL branch must win for a full URL; got: {out}"
        );
        assert!(
            !out.contains("<credential-redacted>"),
            "URL token must not fall through to the credential branch; got: {out}"
        );
        assert!(
            !out.contains("secret") && !out.contains("api.adzuna.com"),
            "url + embedded secret leaked; got: {out}"
        );
    }

    #[test]
    fn credential_marker_does_not_over_redact_benign_words() {
        // The `marker=` shape (the `=`) is required: bare words like `keyword` or a
        // prose `token` (no `=`) must survive verbatim — no false positives.
        assert_eq!(
            sanitize_reason("keyword token apikey missing"),
            "keyword token apikey missing"
        );
    }

    // ── H1: email redaction ───────────────────────────────────────────────────

    #[test]
    fn email_address_in_log_line_is_redacted() {
        // A bare email token must be replaced and the address must not appear in
        // the sanitized output (H1 — emails are highly likely in crash/app logs
        // given the apply-email and contact-profile features).
        let out = sanitize_reason("contact alice@example.com for support");
        assert!(
            out.contains("<email-redacted>"),
            "email placeholder must appear; got: {out}"
        );
        assert!(
            !out.contains("alice@example.com"),
            "email address must not leak; got: {out}"
        );
        // Surrounding prose is preserved.
        assert!(
            out.contains("contact"),
            "surrounding word dropped; got: {out}"
        );
    }

    #[test]
    fn json_embedded_email_is_redacted() {
        // `"email":"alice@example.com"` is a single whitespace-delimited token;
        // after brace/quote trimming it becomes `email":"alice@example.com` which
        // still contains `@` with a dotted domain — must be caught.
        let out = redact_token("\"email\":\"alice@example.com\"");
        assert!(
            out.contains("<email-redacted>"),
            "JSON-embedded email must be redacted; got: {out}"
        );
        assert!(
            !out.contains("alice"),
            "email local-part must not leak; got: {out}"
        );
    }

    #[test]
    fn email_detection_does_not_fire_on_bare_at_or_tld_only() {
        // Lone `@` and `@nodot` must not be treated as emails (no false positives).
        assert_eq!(redact_token("@"), "@");
        assert_eq!(redact_token("@nodot"), "@nodot");
        assert_eq!(redact_token("user@"), "user@");
    }

    // ── H2: JSON-shaped credential redaction ──────────────────────────────────

    #[test]
    fn json_credential_field_is_redacted() {
        // A compact JSON object `{"api_key":"sk-abc123"}` is a single whitespace
        // token; after trimming → `api_key":"sk-abc123`; `key":` flags it.
        let out = sanitize_reason(r#"request {"api_key":"sk-abc123"} failed"#);
        assert!(
            out.contains("<credential-redacted>"),
            "JSON api_key must be redacted; got: {out}"
        );
        assert!(
            !out.contains("sk-abc123"),
            "secret value must not leak; got: {out}"
        );
    }

    #[test]
    fn json_token_field_is_redacted() {
        // `"token":"ghp_…"` shape (e.g. structured log output from an HTTP client).
        let out = sanitize_reason(r#"auth {"token":"ghp_deadbeef"} rejected"#);
        assert!(
            out.contains("<credential-redacted>"),
            "JSON token must be redacted; got: {out}"
        );
        assert!(
            !out.contains("ghp_deadbeef"),
            "token value must not leak; got: {out}"
        );
    }

    #[test]
    fn json_password_field_is_redacted() {
        let out = redact_token(r#"{"password":"hunter2"}"#);
        assert!(
            out.contains("<credential-redacted>"),
            "JSON password must be redacted; got: {out}"
        );
        assert!(
            !out.contains("hunter2"),
            "password value must not leak; got: {out}"
        );
    }

    #[test]
    fn url_branch_still_wins_over_json_credential_marker() {
        // A URL token containing `key=` in the query string must still collapse to
        // `<url-redacted>` (URL branch runs first — existing invariant).
        let out = sanitize_reason("GET https://api.example.com/?api_key=secret failed");
        assert!(
            out.contains("<url-redacted>"),
            "URL branch must win; got: {out}"
        );
        assert!(
            !out.contains("secret"),
            "secret must not leak through URL token; got: {out}"
        );
    }

    // ── AI notes (Phase 4) ────────────────────────────────────────────────────

    #[test]
    fn notes_gate_requires_optin_and_a_resume() {
        // Opt-in OFF → never runs, regardless of résumé.
        assert!(!notes_enabled(false, Some("full résumé text")));
        assert!(!notes_enabled(false, None));
        // Opt-in ON but no usable résumé (absent / empty / whitespace) → skip: the
        // note is grounded in the résumé, so there is nothing to reason about.
        assert!(!notes_enabled(true, None));
        assert!(!notes_enabled(true, Some("")));
        assert!(!notes_enabled(true, Some("   \n\t ")));
        // Opt-in ON with a real résumé → runs.
        assert!(notes_enabled(true, Some("Senior Rust engineer, 8y")));
    }

    #[test]
    fn note_user_msg_fences_resume_and_job_as_data() {
        // SECURITY (OWASP LLM01): the résumé and job posting must ride as fenced
        // DATA in the user turn — the system prompt is the only instruction source.
        // LOW-6: the caller fences the résumé ONCE, before the loop; `note_user_msg`
        // takes it already-fenced.
        let resume_fence = fenced("candidate_resume", "my résumé", RESUME_CAP);
        let msg = note_user_msg(
            &resume_fence,
            "Staff Engineer",
            "Acme",
            Some("Build distributed systems in Rust."),
        );
        assert!(msg.contains("<candidate_resume>\nmy résumé\n</candidate_resume>"));
        assert!(msg.contains("<job_posting>"));
        assert!(msg.contains("Staff Engineer at Acme"));
        assert!(msg.contains("Build distributed systems in Rust."));
        assert!(msg.contains("</job_posting>"));
        // A missing description still produces a valid, fenced job block.
        let no_desc = note_user_msg(&resume_fence, "Dev", "Beta", None);
        assert!(no_desc.contains("<job_posting>"));
        assert!(no_desc.contains("Dev at Beta"));
    }

    #[test]
    fn note_user_msg_never_renders_a_bare_at_separator() {
        // A blank title and/or company must not leave a dangling " at " (or
        // " at Acme" / "Acme at ") in the header — only join the two with " at "
        // when BOTH are present.
        let resume_fence = fenced("candidate_resume", "r", RESUME_CAP);

        let both_blank = note_user_msg(&resume_fence, "", "", Some("desc"));
        assert!(
            !both_blank.contains(" at "),
            "both blank must not render a bare separator; got: {both_blank}"
        );

        let title_only = note_user_msg(&resume_fence, "Dev", "", Some("desc"));
        assert!(title_only.contains("Dev"));
        assert!(
            !title_only.contains(" at "),
            "missing company must not render a trailing separator; got: {title_only}"
        );

        let company_only = note_user_msg(&resume_fence, "", "Acme", Some("desc"));
        assert!(company_only.contains("Acme"));
        assert!(
            !company_only.contains(" at "),
            "missing title must not render a leading separator; got: {company_only}"
        );
    }

    #[test]
    fn note_user_msg_caps_oversized_resume_and_job_as_data() {
        // A pathological résumé/description can't blow the context/cost budget of the
        // note call — each blob is capped at the shared agent-tools char cap. The
        // résumé cap is applied once, by the caller, when it builds the fence.
        let huge = "z".repeat(RESUME_CAP + 5_000);
        let resume_fence = fenced("candidate_resume", &huge, RESUME_CAP);
        let msg = note_user_msg(&resume_fence, "T", "C", Some(&huge));
        // The résumé fence carries at most RESUME_CAP chars of `z`.
        let resume_zs = msg
            .split("<candidate_resume>\n")
            .nth(1)
            .and_then(|s| s.split("\n</candidate_resume>").next())
            .unwrap_or("");
        assert_eq!(resume_zs.chars().filter(|&c| c == 'z').count(), RESUME_CAP);
        assert!(msg.chars().filter(|&c| c == 'z').count() <= RESUME_CAP + JOB_CAP);
    }

    // ── run_notes_loop (HIGH-2: the async loop's guarantees, fake-driven) ──────

    /// A scripted [`NoteEnv`] fake: records every `complete()` call, returns a
    /// canned response (or errors), and can fail `charge_daily` from a chosen call
    /// onward. Mirrors `agent::controller::tests::FakeEnv` — no `AppHandle` or live
    /// provider, which is the whole point of the seam.
    struct FakeNoteEnv {
        calls: Mutex<usize>,
        response: AppResult<String>,
        /// `charge_daily` fails starting from this 1-based call number (`None` =
        /// never fails).
        charge_fails_from: Option<usize>,
    }

    impl FakeNoteEnv {
        fn ok(response: &str) -> Self {
            Self {
                calls: Mutex::new(0),
                response: Ok(response.to_string()),
                charge_fails_from: None,
            }
        }
        fn charge_fails_from(call: usize) -> Self {
            Self {
                calls: Mutex::new(0),
                response: Ok("a note".to_string()),
                charge_fails_from: Some(call),
            }
        }
    }

    #[async_trait]
    impl NoteEnv for FakeNoteEnv {
        async fn complete(
            &self,
            _system: &str,
            _user: &str,
            _temperature: f64,
        ) -> AppResult<String> {
            *self.calls.lock() += 1;
            match &self.response {
                Ok(s) => Ok(s.clone()),
                Err(e) => Err(AppError::Provider(e.to_string())),
            }
        }
        fn charge_daily(&self) -> AppResult<()> {
            let attempted = *self.calls.lock() + 1; // the call this charge is guarding
            match self.charge_fails_from {
                Some(from) if attempted >= from => {
                    Err(AppError::RateLimited("daily cap reached".into()))
                }
                _ => Ok(()),
            }
        }
    }

    fn stub_job(url: &str) -> FoundJob {
        FoundJob {
            title: "Engineer".into(),
            company: "Acme".into(),
            url: url.into(),
            location: None,
            board: None,
            description: None,
            salary_min: None,
            salary_max: None,
            salary_currency: None,
            score: None,
            score_provisional: false,
            found_at: 0,
            is_new: true,
            applied: false,
            trust: None,
            assistant_notes: None,
        }
    }

    #[tokio::test]
    async fn more_than_max_new_jobs_makes_exactly_max_calls() {
        // >3 genuinely-new matches must stop at exactly ASSISTANT_NOTES_MAX calls,
        // not process every job — the hard cost bound.
        let env = FakeNoteEnv::ok("Strong fit; tailor the systems-design bullet.");
        let mut jobs: Vec<FoundJob> = (0..5)
            .map(|i| stub_job(&format!("https://acme.example/{i}")))
            .collect();
        let prior: HashSet<&str> = HashSet::new();
        let generated = run_notes_loop(
            &env,
            "<candidate_resume>\nr\n</candidate_resume>",
            &mut jobs,
            &prior,
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(*env.calls.lock(), ASSISTANT_NOTES_MAX);
        assert_eq!(generated, ASSISTANT_NOTES_MAX);
        assert_eq!(
            jobs.iter().filter(|j| j.assistant_notes.is_some()).count(),
            ASSISTANT_NOTES_MAX
        );
        // Only the FIRST ASSISTANT_NOTES_MAX jobs (in order) were annotated.
        assert!(jobs[ASSISTANT_NOTES_MAX].assistant_notes.is_none());
    }

    #[tokio::test]
    async fn jobs_all_in_prior_urls_make_zero_calls() {
        // Every match already surfaced in a prior run → the merge preserves its
        // earlier note for free; re-generating would just burn a call for nothing.
        let env = FakeNoteEnv::ok("note");
        let mut jobs = vec![
            stub_job("https://acme.example/1"),
            stub_job("https://acme.example/2"),
        ];
        let prior: HashSet<&str> = ["https://acme.example/1", "https://acme.example/2"]
            .into_iter()
            .collect();
        let generated = run_notes_loop(
            &env,
            "<candidate_resume>\nr\n</candidate_resume>",
            &mut jobs,
            &prior,
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(generated, 0);
        assert_eq!(
            *env.calls.lock(),
            0,
            "no provider call for a re-surfaced job"
        );
    }

    #[tokio::test]
    async fn daily_ceiling_error_stops_the_loop_early() {
        // MEDIUM-5: shared per-provider daily ceiling. Once `charge_daily` refuses
        // (2nd call onward here), the loop stops WITHOUT calling `complete()` for
        // that job or any after it — the run still completes, just with fewer notes.
        let env = FakeNoteEnv::charge_fails_from(2);
        let mut jobs = vec![
            stub_job("https://acme.example/1"),
            stub_job("https://acme.example/2"),
            stub_job("https://acme.example/3"),
        ];
        let prior: HashSet<&str> = HashSet::new();
        let generated = run_notes_loop(
            &env,
            "<candidate_resume>\nr\n</candidate_resume>",
            &mut jobs,
            &prior,
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(
            generated, 1,
            "only the job admitted before the ceiling got a note"
        );
        assert_eq!(
            *env.calls.lock(),
            1,
            "the loop must stop before calling complete() again"
        );
        assert!(jobs[0].assistant_notes.is_some());
        assert!(jobs[1].assistant_notes.is_none());
        assert!(jobs[2].assistant_notes.is_none());
    }

    /// HIGH-1: cancellation must interrupt an IN-FLIGHT completion, not just fire
    /// between iterations. `complete()` here never resolves on its own — the only
    /// way `run_notes_loop` can return is via the `tokio::select!` race against
    /// `cancel.cancelled()`. Deterministic under the current-thread test runtime,
    /// mirroring `agent::controller::tests::cancellation_during_an_inflight_turn_stops_immediately`.
    #[tokio::test]
    async fn cancellation_during_an_inflight_call_stops_immediately() {
        struct HangingNoteEnv;
        #[async_trait]
        impl NoteEnv for HangingNoteEnv {
            async fn complete(
                &self,
                _system: &str,
                _user: &str,
                _temperature: f64,
            ) -> AppResult<String> {
                std::future::pending::<AppResult<String>>().await
            }
            fn charge_daily(&self) -> AppResult<()> {
                Ok(())
            }
        }

        let mut jobs = vec![stub_job("https://acme.example/1")];
        let prior: HashSet<&str> = HashSet::new();
        let cancel = CancellationToken::new();
        let cancel_task = cancel.clone();
        tokio::spawn(async move {
            cancel_task.cancel();
        });

        let generated = run_notes_loop(
            &HangingNoteEnv,
            "<candidate_resume>\nr\n</candidate_resume>",
            &mut jobs,
            &prior,
            &cancel,
        )
        .await;
        assert_eq!(generated, 0);
        assert!(jobs[0].assistant_notes.is_none());
    }
}
