//! "Check fit" (`match.live` → `match.result`) — the user-gestured live ATS match against the
//! open job posting, plus the shared ad-hoc scoring path `handle_import` reuses to populate
//! `import.result.matchScore`. Split out of `mod.rs` (R8 module-size cap); the pure scoring
//! primitives (posting-parse, résumé-resolution) now live in the sibling `match_live_score`
//! (shared with `match_live_import_score`'s import-time fill), the timeout wrapper in
//! `match_live_timeout`, and the per-pairing throttle in `match_live_throttle` — see each
//! module's own doc.
//!
//! Scan-mode ONLY: the popup always sends the SAME authenticated-DOM capture
//! the import button uses (`content.ts`'s `capture()`, honoring its
//! `data-ajh-job-root` hint via [`crate::scraping::scrape_url::parse_from_html`]).
//! There is no URL-mode network-fetch fallback here — unlike `handle_import`,
//! a "Check fit" click never adds a network fetch on the desktop side, so the
//! scoring path itself stays zero-egress.
//!
//! No [`crate::postings::PostingsCache`] involvement anywhere in this module:
//! an ad-hoc "Check fit"/import-time score is neither a pursuit nor a
//! discovery (ADR-015) — it only ever reads/writes the `match_scores`
//! self-invalidating result cache, keyed by [`adhoc_job_id`] over the SAME
//! canonicalized + normalized url `handle_import` derives (see
//! [`canonicalized_normalized_url`]) so a "Check fit" click and an import on
//! the same page hit the SAME row instead of scoring twice.
//!
//! ## Keyword-only ALWAYS, structurally — not "off by default"
//! `match_resume`'s `semanticScoringEnabled` toggle lives only in the renderer's
//! `preferences-store` (webview `localStorage`) — no Rust-owned store carries it, so this bridge
//! has no bit to read. [`score_keyword_only`] instead calls
//! [`crate::commands::match_resume::score_adhoc_keyword_only`], which hardcodes semantic scoring
//! OFF **internally** (no `semantic_enabled` parameter exists to flip) — a structural guarantee,
//! not a default this module could accidentally override.
//!
//! That entry point also hardcodes `translate: false`, skipping `score_one`'s
//! `translate_if_needed` call entirely rather than merely no-opping it. This matters because a
//! "local" CLI-agent provider (Ollama et al.) can still perform **cloud egress** despite
//! `ProviderId::is_local()` returning `true` — so only never calling the translate path at all
//! guarantees zero egress. Accepted trade-off: a foreign-language posting is scored keyword-only
//! against its RAW (untranslated) text here; the in-app `match_resume` path
//! (`translate: true`) is unaffected.
//!
//! `scoreSource` is therefore always `"keyword"`; the wire's `semantic`/`"combined"` shapes are
//! reserved for a future PR that gives the bridge a Rust-readable semantic-scoring setting.
//!
//! ## Consent gate — rides the assisted-autofill opt-in
//! `match.live` (unlike `applied.check`/`status.update`) is gated on the SAME
//! opt-in as `profile.get`/`answers.save`/`answers.suggest` (see
//! [`super::AUTOFILL_OFF_MESSAGE`]): `gaps` is effectively a résumé-keyword
//! membership oracle (which of the user's résumé keywords are ABSENT), which
//! is the same consent class as the PII/résumé-derived data those other verbs
//! gate — see [`resolve_match_live`]. The import-time `matchScore` fill
//! ([`score_import_posting`]) stays ungated: it rides the already-consented
//! import gesture and reveals only a single number, never `gaps`.
//!
//! **Threat-model note (the ungated import score):** a single `matchScore` number is technically
//! a coarse résumé-membership signal a scripted client could harvest without opting into
//! autofill — accepted not because the number is too coarse to matter, but because every probe
//! that produces it must go through `handle_import`, which always persists a visible
//! `Application` row and fires a toast/OS notification — probing this signal is loud by
//! construction, never silent. Visibility of the act is the safeguard, not the score's precision.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::match_live_score::{
    adhoc_job_id, build_match_ok, canonicalized_normalized_url, parse_job_text, score_keyword_only,
};
use super::match_live_timeout::score_or_timeout;
use super::msg;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};

/// Fixed sentinel — no résumé exists to score against. One constant (not
/// copies) so `resolve_match_live` and any future caller can't drift, mirrors
/// `AUTOFILL_OFF_MESSAGE`'s discipline.
const NO_RESUME_MESSAGE: &str = "Add a resume in AI Job Hunter first, then try Check fit again.";

/// Fixed sentinel — the captured page couldn't be parsed into job text.
const NO_JOB_TEXT_MESSAGE: &str = "Could not read this job posting. Reload the page and try again.";

/// The `match.live` success outcome — see [`msg::MATCH_RESULT`] docs.
#[derive(Debug)]
pub(super) struct MatchLiveOk {
    pub(super) combined: f64,
    pub(super) ats: f64,
    pub(super) gaps: Vec<String>,
    pub(super) resume_name: String,
    /// A salary range found in the posting text (PR3, design decision 5) — `None` when no range
    /// was found, in which case the WHOLE `salary` wire field is omitted (never a lone
    /// `expectation` with no posting fact — "two facts side by side, never a judgement").
    pub(super) salary_posting: Option<String>,
    /// `JobPreferences.salary_expectation` verbatim, non-empty only — attached to the reply ONLY
    /// alongside [`Self::salary_posting`]. See [`msg::MATCH_LIVE`]'s doc.
    pub(super) salary_expectation: Option<String>,
}

/// The `match.live` consent gate in isolation: refuse with the shared
/// `AUTOFILL_OFF_MESSAGE` when the opt-in is off. Pure (no `AppHandle`, no
/// I/O) so the gate itself is directly unit-testable even though the rest of
/// [`resolve_match_live`] (which calls into `score_one` and therefore needs a
/// real `AppHandle`) is not — mirrors `resolve_answers_suggest`'s early-return
/// gate shape.
fn check_autofill_gate(autofill_enabled: bool) -> AppResult<()> {
    if autofill_enabled {
        Ok(())
    } else {
        Err(AppError::Validation(
            super::AUTOFILL_OFF_MESSAGE.to_string(),
        ))
    }
}

/// Validate `match.live`'s two structural preconditions IN ORDER: the
/// assisted-autofill opt-in gate, THEN url/html emptiness — the "gate-first
/// ordering" fix. An opted-out client must always see
/// [`super::AUTOFILL_OFF_MESSAGE`], even when the request is ALSO malformed,
/// mirroring `resolve_answers_save`/`resolve_answers_suggest` (both gate
/// before parsing their own payload fields). Pure (no `AppHandle`, no I/O) so
/// the ORDERING itself is directly unit-testable even though the rest of
/// [`resolve_match_live`] is not.
fn validate_match_live_request(autofill_enabled: bool, url: &str, html: &str) -> AppResult<()> {
    check_autofill_gate(autofill_enabled)?;
    if url.is_empty() || html.is_empty() {
        return Err(AppError::Validation(
            "url and html are required".to_string(),
        ));
    }
    Ok(())
}

/// Core `match.live`: validate the request in gate-first order
/// ([`validate_match_live_request`] — the assisted-autofill opt-in, same
/// fixed sentinel as `profile.get`/`answers.save`/`answers.suggest`, THEN
/// url/html emptiness), parse the captured DOM, resolve the résumé to score
/// (a fixed sentinel when none exists), and score keyword-only via
/// [`score_keyword_only`] bounded by [`SCORE_TIMEOUT`] ([`score_or_timeout`])
/// so a hung/slow scorer can never block this connection's serial frame loop
/// indefinitely — shaping the reply via [`build_match_ok`].
pub(super) async fn resolve_match_live(
    app: &AppHandle,
    store: &DocumentStore,
    autofill_enabled: bool,
    url: &str,
    html: &str,
    salary_expectation: Option<String>,
) -> AppResult<MatchLiveOk> {
    validate_match_live_request(autofill_enabled, url, html)?;

    let job_text = parse_job_text(url, html)
        .ok_or_else(|| AppError::Validation(NO_JOB_TEXT_MESSAGE.to_string()))?;

    // Extracted BEFORE `job_text` is moved into `score_keyword_only` below — pure, no scoring
    // dependency (see `extraction::salary`'s module doc: two facts, never a verdict).
    let salary_posting = crate::extraction::salary::extract_salary_range(&job_text);

    let docs = store.list();
    let resume =
        resolve_resume(&docs).ok_or_else(|| AppError::Validation(NO_RESUME_MESSAGE.to_string()))?;

    // Cache-key parity with `handle_import`/`score_import_posting`: canonicalize
    // + normalize the raw url before hashing so a "Check fit" click and an
    // import on the SAME page share one `match_scores` row (see
    // `canonicalized_normalized_url`'s doc). `parse_job_text` above still
    // parses the DOM against the RAW url — only the cache key changes here.
    let job_id = adhoc_job_id(&canonicalized_normalized_url(url));
    let result =
        score_or_timeout(score_keyword_only(app, store, resume, &job_id, job_text)).await?;

    let ok = build_match_ok(&result, resume.title.clone());
    Ok(attach_salary(ok, salary_posting, salary_expectation))
}

/// Attach the salary wire pair to `ok`, enforcing the "expectation rides ONLY alongside a found
/// `posting` range" invariant (see [`MatchLiveOk`]'s doc): when `posting` is `None`, `expectation`
/// is dropped too, never left attached alone — "two facts side by side, never a judgement" (design
/// decision 5). Pure — no `AppHandle`, no I/O — so the gate itself is directly unit-testable
/// without a scoring round-trip, unlike [`resolve_match_live`] (which calls it after scoring).
fn attach_salary(
    mut ok: MatchLiveOk,
    posting: Option<String>,
    expectation: Option<String>,
) -> MatchLiveOk {
    ok.salary_expectation = if posting.is_some() { expectation } else { None };
    ok.salary_posting = posting;
    ok
}

/// Build the `match.live` reply. Discriminated union: `ok:true` mirrors a
/// subset of `match_resume`'s `MatchScore` shape (`combined`/`ats`/`gaps`,
/// clamped to [`MAX_GAPS`]) plus `resumeName` + the fixed
/// `scoreSource: "keyword"` (see the module doc); `ok:false` carries a
/// fixed-sentinel `error`. This verb answers a deliberate click, so errors
/// ARE user-facing (like `status.update`/`answers.suggest`), never folded
/// into a silent no-op.
pub(super) fn match_result_reply(req_id: &str, outcome: AppResult<MatchLiveOk>) -> String {
    let payload = match outcome {
        Ok(ok) => {
            let mut payload = json!({
                "ok": true,
                "combined": ok.combined,
                "ats": ok.ats,
                "gaps": ok.gaps,
                "resumeName": ok.resume_name,
                "scoreSource": "keyword",
            });
            // Additive, OPTIONAL — an older extension ignores an unrecognized field harmlessly.
            // Only ever present when a range was actually found (see `MatchLiveOk`'s doc); two
            // facts side by side, never a verdict (design decision 5).
            if let Some(posting) = ok.salary_posting {
                let mut salary = json!({ "posting": posting });
                if let Some(expectation) = ok.salary_expectation {
                    salary["expectation"] = json!(expectation);
                }
                payload["salary"] = salary;
            }
            payload
        }
        // Wire-error discipline: fixed sentinel text only (no dynamic/path/PII
        // content) — detailed context belongs in the desktop log, not on the wire.
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    };
    json!({
        "type": msg::MATCH_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Answer an authenticated `match.live`: read the autofill opt-in off
/// [`super::BridgeState`] + resolve the score against the local
/// `DocumentStore`, then reply `match.result`. Rides the SAME
/// assisted-autofill opt-in gate as `profile.get`/`answers.save`/
/// `answers.suggest` — see the module doc's "Consent gate" section and
/// [`resolve_match_live`].
pub(super) async fn handle_match_live(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let autofill_enabled = app
        .try_state::<super::BridgeState>()
        .map(|s| s.autofill_enabled())
        .unwrap_or(false);

    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let html = payload.get("html").and_then(|v| v.as_str()).unwrap_or("");

    // The backend-readable salary expectation (Task #30) — same managed-state fetch pattern
    // `answers_suggest::handle_answers_suggest` uses; an absent `JobPreferencesStore` (a start-up
    // failure) just means no `expectation` fact, never an error. Rides the SAME autofill gate as
    // the rest of this reply (checked inside `resolve_match_live`).
    let salary_expectation = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .and_then(|s| s.get().salary_expectation)
        .filter(|s| !s.trim().is_empty());

    // Validation order (gate before url/html emptiness) lives inside
    // `resolve_match_live` — see `validate_match_live_request`'s doc.
    let outcome = match app.try_state::<DocumentStore>() {
        Some(store) => {
            resolve_match_live(
                app,
                store.inner(),
                autofill_enabled,
                &url,
                html,
                salary_expectation,
            )
            .await
        }
        None => Err(AppError::Config("document store unavailable".to_string())),
    };

    match_result_reply(req_id, outcome)
}

// ── Split siblings (R8 relief), declared in `mod.rs` — re-exported so every existing
// external caller (`answer_assist::resolve`, `import_flow`, `mod.rs`'s BridgeState) keeps
// resolving these through `match_live::…` unchanged. ─────────────────────────────────────
pub(super) use super::match_live_import_score::score_import_posting_bounded;
pub(super) use super::match_live_score::resolve_resume;
pub(super) use super::match_live_throttle::{throttled_reply, MatchLiveThrottle};

#[cfg(test)]
mod tests;
