//! "Check fit" (`match.live` → `match.result`) — the user-gestured live ATS
//! match against the open job posting, plus the shared ad-hoc scoring path
//! `handle_import` reuses to populate `import.result.matchScore`. Split out of
//! `mod.rs` (own file, per the R8 module-size cap) mirroring
//! `status_update`/`answers_suggest`'s pure/impure split: the posting-parse and
//! résumé-resolution helpers below take no `AppHandle` and are directly
//! unit-testable; only [`resolve_match_live`]/[`score_import_posting`] (which
//! call into [`crate::commands::match_resume::score_adhoc_keyword_only`]) are
//! async.
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
//! `match_resume` (the in-app Jobs-page scorer) reads a caller-supplied
//! `semanticScoringEnabled` request flag; the ONE place a user toggles that
//! flag is the renderer's `preferences-store`
//! (`apps/desktop/src/renderer/store/preferences-store` — a Zustand store
//! persisted to the WEBVIEW's `localStorage`, not any Rust-owned store — see
//! `commands::privacy::privacy_reset_app`'s doc: "The frontend is responsible
//! for resetting persisted preferences (localStorage)"). There is NO Rust-side
//! settings store this module could read that bit from — the bridge is a
//! plain Rust module with no channel into the webview's `localStorage`. So
//! [`score_keyword_only`] calls
//! [`crate::commands::match_resume::score_adhoc_keyword_only`], which
//! hardcodes semantic scoring OFF **internally** (no `semantic_enabled`
//! parameter exists to flip) — a structural guarantee, not a default this
//! module could accidentally override.
//!
//! That same entry point also NEVER translates the job text: it hardcodes
//! `translate: false` all the way through to `score_one`'s
//! `translate_if_needed` call site, so the call is skipped entirely rather
//! than merely no-opping. This matters because a CLI-agent provider
//! configured as "local" (Ollama et al.) still performs **cloud egress**
//! despite `ProviderId::is_local()` returning `true` — so "translate only
//! when a local provider is configured" is not by itself a zero-egress
//! guarantee; only never calling the translation path at all is. The accepted
//! accuracy trade-off: a foreign-language job posting is scored keyword-only
//! against its RAW (untranslated) text from this entry point — the in-app
//! `match_resume` path (`score_one` called with `translate: true`) is
//! unaffected and keeps translating exactly as before.
//!
//! `scoreSource` is therefore always `"keyword"` today; the wire's
//! `semantic`/`"combined"` shapes are reserved (see the shared TS payload doc)
//! for a future PR that gives the bridge a Rust-readable version of the
//! semantic-scoring setting — not wired here.
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
//! **Threat-model note (the ungated import score):** a single `matchScore`
//! number IS technically a coarse résumé-membership signal — one bit per
//! import of "did this posting score well against my résumé" — that a
//! scripted client could in principle harvest without ever opting into
//! autofill. This is accepted NOT because the number is too coarse to
//! matter, but because every probe that produces it must first go through
//! `handle_import`, which always persists a visible `Application` row and
//! fires a toast/OS notification (see `import_flow.rs`'s `matchScore` fill
//! site) — probing this signal is loud by construction, never silent.
//! Visibility of the act is the safeguard here, not the score's precision.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::msg;
use crate::documents::{DocumentRecord, DocumentStore};
use crate::error::{AppError, AppResult};
use crate::scraping::types::JobPosting;

/// Fixed sentinel — no résumé exists to score against. One constant (not
/// copies) so `resolve_match_live` and any future caller can't drift, mirrors
/// `AUTOFILL_OFF_MESSAGE`'s discipline.
const NO_RESUME_MESSAGE: &str = "Add a resume in AI Job Hunter first, then try Check fit again.";

/// Fixed sentinel — the captured page couldn't be parsed into job text.
const NO_JOB_TEXT_MESSAGE: &str = "Could not read this job posting. Reload the page and try again.";

/// Fixed sentinel — scoring failed with an unexpected internal shape OR
/// exceeded [`SCORE_TIMEOUT`]. One constant (not two call-site copies) so a
/// genuine hang and an internal failure are indistinguishable on the wire —
/// see [`score_or_timeout`].
const SCORE_FAILED_MESSAGE: &str = "Could not score this posting. Please retry.";

/// Cap on the gap keywords sent over the wire — a JD can carry far more misses
/// than are useful in a popup chip list.
const MAX_GAPS: usize = 8;

/// The `match.live` success outcome — see [`msg::MATCH_RESULT`] docs.
#[derive(Debug)]
pub(super) struct MatchLiveOk {
    pub(super) combined: f64,
    pub(super) ats: f64,
    pub(super) gaps: Vec<String>,
    pub(super) resume_name: String,
}

/// Resolve which résumé to score: the `is_default` row, else the
/// most-recently-created one, else `None` when the user has no résumé at all.
/// `docs` is expected to be [`DocumentStore::list`]'s output, which is already
/// ordered `created_at DESC` — so the first entry IS the most recent; this
/// function does not re-sort. Pure — no `AppHandle`, no I/O — directly
/// unit-testable against a synthetic `Vec<DocumentRecord>`.
pub(super) fn resolve_resume(docs: &[DocumentRecord]) -> Option<&DocumentRecord> {
    docs.iter().find(|d| d.is_default).or_else(|| docs.first())
}

/// Parse a captured Scan-mode DOM into a searchable job-text blob — the SAME
/// extraction `handle_import`'s Scan-mode branch uses
/// ([`crate::scraping::scrape_url::parse_from_html`], which honors the
/// `data-ajh-job-root` hint `content.ts` marks) plus
/// [`crate::documents::keywords::posting_text_blob`] for the title/
/// description/requirements join. `None` when nothing usable parsed (a
/// blocked page / unrecognized markup) — the caller surfaces a fixed refusal,
/// never a panic.
pub(super) fn parse_job_text(url: &str, html: &str) -> Option<String> {
    let posting = crate::scraping::scrape_url::parse_from_html(url, html)?;
    posting_job_text(&posting)
}

/// Build the same ATS text blob [`parse_job_text`] does, directly from an
/// already-parsed [`JobPosting`] — shared by [`score_import_posting`], which
/// has a `JobPosting` in hand from `handle_import` and never re-parses HTML.
fn posting_job_text(posting: &JobPosting) -> Option<String> {
    crate::documents::keywords::posting_text_blob(
        &posting.title,
        posting.description.as_deref(),
        posting.requirements.as_deref(),
    )
}

/// Score `resume` against `job_text`, keyword-only (ALWAYS — see the module
/// doc), caching under the ad-hoc `job_id`. Shared by [`resolve_match_live`]
/// (the popup's "Check fit" click) AND [`score_import_posting`] (the
/// `import.result.matchScore` fill), so the keyword-only decision and the
/// underlying [`crate::commands::match_resume::score_adhoc_keyword_only`] call
/// live in exactly one place. That callee hardcodes keyword-only AND
/// never-translate internally (no flags to pass here) — see its doc.
pub(super) async fn score_keyword_only(
    app: &AppHandle,
    store: &DocumentStore,
    resume: &DocumentRecord,
    job_id: &str,
    job_text: String,
) -> Value {
    let resume_raw_keywords = crate::commands::match_resume::parse_resume_keywords(resume);
    let active = store.embedding_config();
    crate::commands::match_resume::score_adhoc_keyword_only(
        app,
        store,
        resume,
        resume_raw_keywords.as_deref(),
        &active,
        job_id,
        job_text,
    )
    .await
}

/// Build the ad-hoc result-cache key for a normalized job url. One scheme
/// shared by [`resolve_match_live`] and [`score_import_posting`] so a
/// "Check fit" click and an import on the SAME page hit the SAME
/// self-invalidating `match_scores` row instead of scoring twice. Prefixed so
/// it can never collide with a real `PostingsCache` posting id (which never
/// carries this prefix).
fn adhoc_job_id(normalized_url: &str) -> String {
    format!("adhoc:{}", crate::documents::sha256_hex(normalized_url))
}

/// Canonicalize + normalize a raw url the SAME way `handle_import` derives its
/// `normalized` variable (`canonical_job_url` then
/// [`crate::applications::normalize_job_url`]) — so a "Check fit" click and an
/// import on the SAME page compute the IDENTICAL [`adhoc_job_id`] and hit the
/// SAME `match_scores` row, regardless of which raw url variant (www /
/// trailing slash / tracking query params) each side happened to observe. Used
/// ONLY to derive the cache key — [`parse_job_text`] still parses the DOM
/// against the raw `url` untouched, mirroring `handle_import`'s Scan-mode
/// branch (which never rewrites the DOM-parse url either). Pure — no I/O — so
/// the cache-key parity is directly unit-testable against representative url
/// variants without a scoring round-trip.
fn canonicalized_normalized_url(url: &str) -> String {
    let canonical = crate::scraping::scrape_url::canonical_job_url(url);
    let effective = canonical.as_deref().unwrap_or(url);
    crate::applications::normalize_job_url(effective)
}

/// Extract the wire-ready [`MatchLiveOk`] fields out of `score_one`'s raw
/// `Value` result (`combined`/`ats`/`gaps`), clamping `gaps` to [`MAX_GAPS`].
/// Pure — no `AppHandle`, no I/O — so the clamp is directly unit-testable
/// against a synthetic `Value` without a scoring round-trip.
fn build_match_ok(result: &Value, resume_name: String) -> MatchLiveOk {
    let combined = result
        .get("combined")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let ats = result.get("ats").and_then(Value::as_f64).unwrap_or(0.0);
    let gaps: Vec<String> = result
        .get("gaps")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|g| g.as_str().map(str::to_string))
                .take(MAX_GAPS)
                .collect()
        })
        .unwrap_or_default();

    MatchLiveOk {
        combined,
        ats,
        gaps,
        resume_name,
    }
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
) -> AppResult<MatchLiveOk> {
    validate_match_live_request(autofill_enabled, url, html)?;

    let job_text = parse_job_text(url, html)
        .ok_or_else(|| AppError::Validation(NO_JOB_TEXT_MESSAGE.to_string()))?;

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

    Ok(build_match_ok(&result, resume.title.clone()))
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
        Ok(ok) => json!({
            "ok": true,
            "combined": ok.combined,
            "ats": ok.ats,
            "gaps": ok.gaps,
            "resumeName": ok.resume_name,
            "scoreSource": "keyword",
        }),
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

    // Validation order (gate before url/html emptiness) lives inside
    // `resolve_match_live` — see `validate_match_live_request`'s doc.
    let outcome = match app.try_state::<DocumentStore>() {
        Some(store) => resolve_match_live(app, store.inner(), autofill_enabled, &url, html).await,
        None => Err(AppError::Config("document store unavailable".to_string())),
    };

    match_result_reply(req_id, outcome)
}

/// Best-effort keyword-only match score for `handle_import`'s
/// `import.result.matchScore` (the field has existed on the wire since the
/// import feature shipped — see `ExtensionImportResult`'s doc; this is the
/// first PR that ever populates it). Reuses the SAME résumé-resolution + ad-
/// hoc scoring path as [`resolve_match_live`]'s "Check fit" button, keyed by
/// the import's OWN already-normalized url so a later "Check fit" click on
/// the identical page hits the SAME self-invalidating result-cache row.
/// Returns `None` on ANY failure (no résumé yet, no usable posting text, the
/// document store unavailable) — `handle_import` has ALREADY persisted the
/// Application by the time this runs, so a scoring failure only omits the
/// field; it can never fail or block the import itself.
pub(super) async fn score_import_posting(
    app: &AppHandle,
    posting: &JobPosting,
    normalized_url: &str,
) -> Option<f64> {
    let job_text = posting_job_text(posting)?;
    let store = app.try_state::<DocumentStore>()?;
    let docs = store.list();
    let resume = resolve_resume(&docs)?;
    let job_id = adhoc_job_id(normalized_url);
    let result = score_keyword_only(app, store.inner(), resume, &job_id, job_text).await;
    result.get("combined").and_then(Value::as_f64)
}

/// Wall-clock cap on the keyword-only scorer — shared by
/// [`score_import_posting`] (the import's own WS reply budget the extension
/// enforces client-side is ~30s, `bridge.ts`'s import timeout; scoring must
/// never eat meaningfully into that) AND [`score_or_timeout`] (the
/// interactive "Check fit" path — a hung/slow scorer must never block
/// `handle_connection`'s single-socket serial frame loop, which awaits each
/// verb synchronously, so a hang here would stall every subsequent frame on
/// that connection). Deliberately generous relative to the keyword-only
/// path's typical cost — a hard backstop, not a normal-path limit.
const SCORE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Race `fut` against `cap`, returning tokio's raw timeout `Result` so a
/// caller can tell "genuinely timed out" (`Err`) apart from "finished in time
/// but yielded `None`" (`Ok(None)`) — [`score_import_posting_bounded`] logs
/// the two cases at different levels. Isolated as its own generic helper
/// because `score_import_posting` has no injectable delay seam (it reaches
/// into `AppHandle`/`DocumentStore` end to end), so THIS is the testable
/// boundary for the timeout mechanism itself (see the tests below).
async fn timed<F, T>(
    cap: std::time::Duration,
    fut: F,
) -> Result<Option<T>, tokio::time::error::Elapsed>
where
    F: std::future::Future<Output = Option<T>>,
{
    tokio::time::timeout(cap, fut).await
}

/// Await `scoring` (a [`score_keyword_only`]-shaped future) bounded by
/// [`SCORE_TIMEOUT`] — the HIGH "bound interactive scoring" fix.
/// [`resolve_match_live`] used to await [`score_keyword_only`] unbounded, and
/// `handle_connection`'s frame loop awaits each verb synchronously, so a
/// hang/slowdown there would block every subsequent frame on that socket, not
/// just this request. Collapses BOTH a genuine timeout AND
/// `score_adhoc_keyword_only`'s only error branch ("job not found" —
/// unreachable here since `job_text` is always `Some`, but never let an
/// internal string reach the wire regardless of how it triggered) onto the
/// SAME fixed [`SCORE_FAILED_MESSAGE`] sentinel. Isolated as its own generic
/// helper (mirrors [`timed`]'s own isolation) because `resolve_match_live` has
/// no injectable delay seam (a real `AppHandle`/`DocumentStore` end to end) —
/// THIS is the testable boundary (see the tests below).
async fn score_or_timeout<F>(scoring: F) -> AppResult<Value>
where
    F: std::future::Future<Output = Value>,
{
    match timed(SCORE_TIMEOUT, async { Some(scoring.await) }).await {
        Ok(Some(result)) if result.get("error").is_none() => Ok(result),
        Ok(Some(_)) => {
            log::warn!("[extension_bridge] match.live scoring returned an unexpected error shape");
            Err(AppError::Validation(SCORE_FAILED_MESSAGE.to_string()))
        }
        _ => {
            log::warn!(
                "[extension_bridge] match.live scoring exceeded {:?}; refusing with the fixed sentinel",
                SCORE_TIMEOUT
            );
            Err(AppError::Validation(SCORE_FAILED_MESSAGE.to_string()))
        }
    }
}

/// [`score_import_posting`] bounded by [`SCORE_TIMEOUT`] — the function
/// `handle_import` actually calls. Logs at a level that matches how actionable
/// the outcome is: a genuine timeout (a slow/degraded scorer) is worth a
/// `warn`; an ordinary `None` — no résumé saved yet (the normal state for a
/// new user), unusable posting text, or a scoring failure — is expected noise
/// on plenty of imports and only worth a `debug`. Either way `matchScore` is
/// simply omitted; the import above has ALREADY succeeded by the time this runs.
pub(super) async fn score_import_posting_bounded(
    app: &AppHandle,
    posting: &JobPosting,
    normalized_url: &str,
) -> Option<f64> {
    match timed(
        SCORE_TIMEOUT,
        score_import_posting(app, posting, normalized_url),
    )
    .await
    {
        Err(_) => {
            log::warn!(
                "[extension_bridge] import-time match score exceeded {:?}; omitting matchScore",
                SCORE_TIMEOUT
            );
            None
        }
        Ok(None) => {
            log::debug!(
                "[extension_bridge] import-time match score unavailable (no résumé / unusable \
                 posting text / scoring failure); omitting matchScore"
            );
            None
        }
        Ok(Some(score)) => Some(score),
    }
}

/// Minimal token-bucket throttle for `match.live`. A deliberate "Check fit"
/// click can legitimately fire a few times in quick succession (a
/// double-click, a retry after fixing the résumé), but an unbounded stream of
/// clicks/automation should not be free to keep re-running the scorer. Burst
/// [`MATCH_LIVE_BURST`] requests, refilling one token every
/// [`MATCH_LIVE_REFILL_SECS`] (~1 req/2s sustained).
///
/// Lives on [`super::BridgeState`] (behind a `Mutex`, shared across EVERY
/// connection for this pairing) rather than per-connection — the MEDIUM
/// "reconnect-proof throttle" fix. A loopback reconnect is a cheap,
/// near-instant handshake (see `handle_connection`'s doc); a per-connection
/// instance would hand a fresh full burst to every reconnect, so an automated
/// client could trivially bypass the throttle just by reconnecting. The
/// bucket must outlive any single socket.
///
/// Scoped to `match.live` only this round: a future compute-heavy verb would
/// give itself its own throttle instance with its own constants (each verb's
/// cost profile differs) rather than share this one, so this struct is
/// deliberately NOT made generic/shared across verbs.
pub(super) struct MatchLiveThrottle {
    tokens: f64,
    last: std::time::Instant,
}

/// Requests allowed in quick succession before the bucket empties.
const MATCH_LIVE_BURST: f64 = 3.0;
/// Seconds to refill one token (~1 sustained request every this many seconds).
const MATCH_LIVE_REFILL_SECS: f64 = 2.0;

impl MatchLiveThrottle {
    pub(super) fn new() -> Self {
        Self {
            tokens: MATCH_LIVE_BURST,
            last: std::time::Instant::now(),
        }
    }

    /// Try to consume one token at `now` (an explicit clock so the refill
    /// math is directly unit-testable without a real sleep; production call
    /// sites always go through [`Self::try_acquire`]). Returns `true` (and
    /// consumes a token) when the request may proceed.
    fn try_acquire_at(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed / MATCH_LIVE_REFILL_SECS).min(MATCH_LIVE_BURST);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub(super) fn try_acquire(&mut self) -> bool {
        self.try_acquire_at(std::time::Instant::now())
    }
}

/// Fixed sentinel for [`MatchLiveThrottle`]'s refusal — deliberately generic
/// wording (not scorer/verb-specific) so it reads sensibly if a future verb
/// ever reuses the same shape.
pub(super) const THROTTLED_MESSAGE: &str = "Too many requests — try again shortly.";

/// Build the `match.result` reply for a throttled `match.live` request — a
/// `RateLimited` refusal is just another `ok:false` outcome on the SAME
/// discriminated reply [`match_result_reply`] already builds.
pub(super) fn throttled_reply(req_id: &str) -> String {
    match_result_reply(
        req_id,
        Err(AppError::RateLimited(THROTTLED_MESSAGE.to_string())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: &str, title: &str, is_default: bool, created_at: u64) -> DocumentRecord {
        DocumentRecord {
            id: id.to_string(),
            title: title.to_string(),
            name: String::new(),
            locale: None,
            text: "resume text".to_string(),
            pages: None,
            created_at,
            indexed: false,
            is_default,
            keywords_json: None,
        }
    }

    // ── resolve_resume ────────────────────────────────────────────────────────

    #[test]
    fn resolve_resume_prefers_is_default_over_most_recent() {
        let docs = vec![
            doc("newest", "Newest", false, 200),
            doc("default", "Default resume", true, 100),
        ];
        let picked = resolve_resume(&docs).expect("must pick a resume");
        assert_eq!(
            picked.id, "default",
            "is_default must win even though it's not first/newest"
        );
    }

    #[test]
    fn resolve_resume_falls_back_to_most_recent_when_no_default() {
        // Mirrors DocumentStore::list()'s ordering contract: already created_at
        // DESC, so the FIRST entry is the most recent — this fn must not re-sort.
        let docs = vec![
            doc("most-recent", "Most recent", false, 200),
            doc("older", "Older", false, 100),
        ];
        let picked = resolve_resume(&docs).expect("must pick a resume");
        assert_eq!(picked.id, "most-recent");
    }

    #[test]
    fn resolve_resume_none_when_no_documents() {
        assert!(resolve_resume(&[]).is_none());
    }

    // ── parse_job_text / posting_job_text ────────────────────────────────────

    #[test]
    fn parse_job_text_none_for_blank_page() {
        // `parse_from_html` always returns `Some` for a successfully-parsed
        // document (title may be an empty string — see its doc), so a blank
        // page yields `Some(JobPosting { title: "", description: None, .. })`.
        // `posting_text_blob` then has nothing usable, so `parse_job_text`
        // must propagate `None` from THAT step, not panic or fabricate text.
        assert!(parse_job_text(
            "https://example.com/not-a-job",
            "<html><body></body></html>"
        )
        .is_none());
    }

    #[test]
    fn posting_job_text_joins_title_description_requirements() {
        let posting = JobPosting {
            id: "job-1".to_string(),
            external_id: None,
            title: "Senior Rust Engineer".to_string(),
            company: "Acme".to_string(),
            location: None,
            url: "https://example.com/job/1".to_string(),
            source: "url".to_string(),
            description: Some("Build reliable systems.".to_string()),
            requirements: Some(vec!["Rust".to_string(), "Tokio".to_string()]),
            posted_at: None,
            captured_at: 0,
            extra: std::collections::HashMap::new(),
        };
        let text = posting_job_text(&posting).expect("posting has usable text");
        assert!(text.contains("Senior Rust Engineer"));
        assert!(text.contains("Build reliable systems."));
        assert!(text.contains("Rust"));
        assert!(text.contains("Tokio"));
    }

    #[test]
    fn posting_job_text_none_when_everything_blank() {
        let posting = JobPosting {
            id: "job-2".to_string(),
            external_id: None,
            title: String::new(),
            company: "Acme".to_string(),
            location: None,
            url: "https://example.com/job/2".to_string(),
            source: "url".to_string(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: 0,
            extra: std::collections::HashMap::new(),
        };
        assert!(posting_job_text(&posting).is_none());
    }

    // ── adhoc_job_id ──────────────────────────────────────────────────────────

    #[test]
    fn adhoc_job_id_is_stable_and_prefixed() {
        let a = adhoc_job_id("https://example.com/job/1");
        let b = adhoc_job_id("https://example.com/job/1");
        assert_eq!(a, b, "same url must yield the same cache key");
        assert!(
            a.starts_with("adhoc:"),
            "must be prefixed so it can never collide with a real posting id"
        );
    }

    #[test]
    fn adhoc_job_id_differs_per_url() {
        let a = adhoc_job_id("https://example.com/job/1");
        let b = adhoc_job_id("https://example.com/job/2");
        assert_ne!(a, b);
    }

    // ── canonicalized_normalized_url (cache-key parity with handle_import) ───

    /// A raw url carrying www / a trailing slash / a tracking query param, and
    /// its ALREADY-normalized form, must compute the IDENTICAL ad-hoc cache
    /// key — the MEDIUM cache-key-parity fix: a "Check fit" click and an
    /// import on the same page must hit the same `match_scores` row.
    #[test]
    fn resolve_match_live_cache_key_matches_import_normalization() {
        let raw = "https://www.acme.example/jobs/42/?utm_source=ext";
        let already_normalized = "https://acme.example/jobs/42";
        assert_eq!(
            adhoc_job_id(&canonicalized_normalized_url(raw)),
            adhoc_job_id(&canonicalized_normalized_url(already_normalized)),
            "raw and pre-normalized url variants must hit the same ad-hoc cache key"
        );

        // A #fragment variant (e.g. a same-page anchor like #apply) must also
        // collapse to the same cache key — normalize_job_url strips fragments too.
        let with_fragment = "https://www.acme.example/jobs/42/#apply";
        assert_eq!(
            adhoc_job_id(&canonicalized_normalized_url(with_fragment)),
            adhoc_job_id(&canonicalized_normalized_url(already_normalized)),
            "a #fragment url variant must hit the same ad-hoc cache key"
        );
    }

    #[test]
    fn canonicalized_normalized_url_matches_normalize_job_url_for_a_plain_url() {
        // No SPA/list-view rewrite applies to a plain job-detail-shaped url, so
        // this must equal a direct `normalize_job_url` call (no surprise host).
        let url = "https://www.acme.example/jobs/42/";
        assert_eq!(
            canonicalized_normalized_url(url),
            crate::applications::normalize_job_url(url)
        );
    }

    // ── check_autofill_gate (the match.live consent gate) ────────────────────

    #[test]
    fn check_autofill_gate_refuses_when_opt_in_off() {
        let err = check_autofill_gate(false).unwrap_err();
        assert!(
            err.to_string().contains("Autofill is off"),
            "refusal must carry the shared AUTOFILL_OFF_MESSAGE; got {err}"
        );
    }

    #[test]
    fn check_autofill_gate_allows_when_opt_in_on() {
        assert!(check_autofill_gate(true).is_ok());
    }

    // ── validate_match_live_request (LOW: gate-first ordering) ───────────────

    #[test]
    fn validate_match_live_request_prefers_the_autofill_gate_over_emptiness() {
        // Both preconditions fail (opt-in off AND url/html blank) — the gate
        // must win, so an opted-out client ALWAYS sees AUTOFILL_OFF_MESSAGE,
        // consistent with resolve_answers_save/resolve_answers_suggest (both
        // gate before parsing their own payload fields).
        let err = validate_match_live_request(false, "", "").unwrap_err();
        assert!(
            err.to_string().contains("Autofill is off"),
            "the gate failure must win over the emptiness check; got {err}"
        );
    }

    #[test]
    fn validate_match_live_request_reports_emptiness_once_the_gate_passes() {
        let err = validate_match_live_request(true, "", "").unwrap_err();
        assert_eq!(err.to_string(), "url and html are required");
    }

    #[test]
    fn validate_match_live_request_ok_when_both_pass() {
        assert!(
            validate_match_live_request(true, "https://example.com/job/1", "<html></html>").is_ok()
        );
    }

    // ── match_result_reply ───────────────────────────────────────────────────

    #[test]
    fn match_result_reply_carries_ok_payload() {
        let reply = match_result_reply(
            "req-1",
            Ok(MatchLiveOk {
                combined: 72.0,
                ats: 60.0,
                gaps: vec!["kubernetes".to_string()],
                resume_name: "My Resume".to_string(),
            }),
        );
        let v: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(v["type"], msg::MATCH_RESULT);
        assert_eq!(v["reqId"], "req-1");
        assert_eq!(v["payload"]["ok"], true);
        assert_eq!(v["payload"]["combined"], 72.0);
        assert_eq!(v["payload"]["ats"], 60.0);
        assert_eq!(v["payload"]["resumeName"], "My Resume");
        assert_eq!(v["payload"]["scoreSource"], "keyword");
        assert_eq!(v["payload"]["gaps"][0], "kubernetes");
        assert!(
            v["payload"].get("semantic").is_none(),
            "semantic is never populated by this path"
        );
    }

    #[test]
    fn match_result_reply_carries_error() {
        let reply = match_result_reply(
            "req-2",
            Err(AppError::Validation(NO_RESUME_MESSAGE.to_string())),
        );
        let v: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(v["type"], msg::MATCH_RESULT);
        assert_eq!(v["payload"]["ok"], false);
        assert_eq!(v["payload"]["error"], NO_RESUME_MESSAGE);
        assert!(
            v["payload"].get("combined").is_none(),
            "ok:false must never carry success fields"
        );
    }

    // ── build_match_ok (extraction + gap clamping) ───────────────────────────

    #[test]
    fn build_match_ok_clamps_gaps_to_max() {
        // A synthetic score_one-shaped Value with MORE than MAX_GAPS entries —
        // exercises the REAL `.take(MAX_GAPS)` clamp, not a pre-clamped mock.
        let many_gaps: Vec<String> = (0..20).map(|i| format!("kw{i}")).collect();
        let result = json!({
            "combined": 50.0,
            "ats": 40.0,
            "gaps": many_gaps,
        });
        let ok = build_match_ok(&result, "Resume".to_string());
        assert_eq!(ok.gaps.len(), MAX_GAPS, "gaps must be clamped to MAX_GAPS");
        assert_eq!(
            ok.gaps,
            many_gaps[..MAX_GAPS],
            "the clamp must keep the FIRST MAX_GAPS entries"
        );
        assert_eq!(ok.combined, 50.0);
        assert_eq!(ok.ats, 40.0);
    }

    #[test]
    fn build_match_ok_passes_through_fewer_than_max_gaps_unclamped() {
        let result = json!({ "combined": 10.0, "ats": 5.0, "gaps": ["one", "two"] });
        let ok = build_match_ok(&result, "Resume".to_string());
        assert_eq!(ok.gaps, vec!["one".to_string(), "two".to_string()]);
    }

    #[test]
    fn build_match_ok_defaults_missing_numeric_fields_to_zero() {
        let result = json!({});
        let ok = build_match_ok(&result, "Resume".to_string());
        assert_eq!(ok.combined, 0.0);
        assert_eq!(ok.ats, 0.0);
        assert!(ok.gaps.is_empty());
    }

    // ── timed (the import-reply-scoring timeout mechanism) ───────────────────
    // `score_import_posting` has no injectable delay seam (it reaches into a
    // real AppHandle/DocumentStore end to end), so these exercise the generic
    // timeout wrapper directly — the HIGH "bound the import-reply scoring"
    // fix's actual testable boundary, per the task's own fallback note.

    #[tokio::test(start_paused = true)]
    async fn timed_returns_err_when_the_future_exceeds_the_cap() {
        let cap = std::time::Duration::from_millis(100);
        let out = timed(cap, async {
            tokio::time::sleep(cap * 2).await;
            Some(42.0_f64)
        })
        .await;
        assert!(
            out.is_err(),
            "a future that exceeds the cap must yield the raw timeout Err, not the eventual value"
        );
    }

    #[tokio::test]
    async fn timed_returns_the_value_when_within_the_cap() {
        let out = timed(std::time::Duration::from_millis(50), async {
            Some(7.0_f64)
        })
        .await;
        assert_eq!(
            out.unwrap(),
            Some(7.0),
            "a fast future must pass its value through unchanged"
        );
    }

    #[tokio::test]
    async fn timed_distinguishes_a_fast_none_from_a_timeout() {
        // A None that resolves WELL within the cap must be `Ok(None)` — NOT
        // the `Err` a genuine timeout produces. This is exactly the
        // distinction `score_import_posting_bounded` logs at different levels
        // (a fast None must never be misreported as a timeout).
        let out: Result<Option<f64>, _> =
            timed(std::time::Duration::from_millis(50), async { None }).await;
        assert_eq!(out.unwrap(), None);
    }

    // ── score_or_timeout (the HIGH "bound interactive scoring" fix) ──────────
    // `resolve_match_live` has no injectable delay seam either (real
    // AppHandle/DocumentStore end to end) — same fallback boundary as `timed`
    // above: exercise the wrapper directly with a synthetic scoring future.

    #[tokio::test(start_paused = true)]
    async fn score_or_timeout_yields_the_sentinel_on_a_genuine_hang() {
        let out = score_or_timeout(async {
            tokio::time::sleep(SCORE_TIMEOUT * 2).await;
            json!({ "combined": 99.0 })
        })
        .await;
        let err = out.expect_err("a hang must never block the resolve path — must refuse");
        assert_eq!(
            err.to_string(),
            SCORE_FAILED_MESSAGE,
            "a timeout must yield the fixed sentinel, not the eventual value nor a raw timeout error"
        );
    }

    #[tokio::test]
    async fn score_or_timeout_passes_through_a_fast_result() {
        let out = score_or_timeout(async { json!({ "combined": 42.0, "ats": 30.0 }) }).await;
        let result = out.expect("a fast result within the cap must pass through");
        assert_eq!(result["combined"], 42.0);
    }

    #[tokio::test]
    async fn score_or_timeout_refuses_an_unexpected_error_shape() {
        let out = score_or_timeout(async { json!({ "error": "job not found" }) }).await;
        assert_eq!(
            out.unwrap_err().to_string(),
            SCORE_FAILED_MESSAGE,
            "an internal error shape must never leak onto the wire — same fixed sentinel as a timeout"
        );
    }

    // ── MatchLiveThrottle (per-connection compute-verb throttle) ─────────────

    #[test]
    fn throttle_allows_a_burst_then_refuses() {
        let mut t = MatchLiveThrottle::new();
        let now = std::time::Instant::now();
        assert!(t.try_acquire_at(now), "1st request in the burst");
        assert!(t.try_acquire_at(now), "2nd request in the burst");
        assert!(t.try_acquire_at(now), "3rd request in the burst");
        assert!(
            !t.try_acquire_at(now),
            "a 4th immediate request must be throttled (burst exhausted)"
        );
    }

    #[test]
    fn throttle_refills_one_token_per_interval_not_a_second_burst() {
        let mut t = MatchLiveThrottle::new();
        let t0 = std::time::Instant::now();
        for _ in 0..3 {
            assert!(t.try_acquire_at(t0));
        }
        assert!(!t.try_acquire_at(t0), "bucket is empty");

        let t1 = t0 + std::time::Duration::from_secs_f64(MATCH_LIVE_REFILL_SECS);
        assert!(
            t.try_acquire_at(t1),
            "exactly one interval later, one token must have refilled"
        );
        assert!(
            !t.try_acquire_at(t1),
            "only ONE token refilled — this must not re-open the full burst"
        );
    }

    #[test]
    fn throttle_state_is_isolated_per_instance() {
        // Two throttle INSTANCES never share tokens — this is what would
        // guarantee a future distinct-throttle verb stays unaffected by this
        // one's exhaustion. In production there is exactly ONE instance
        // (owned by `BridgeState`, shared across every connection for a
        // pairing — see this struct's doc); this test only pins that the
        // struct itself carries no hidden global state.
        let mut a = MatchLiveThrottle::new();
        let mut b = MatchLiveThrottle::new();
        let now = std::time::Instant::now();
        for _ in 0..3 {
            assert!(a.try_acquire_at(now));
        }
        assert!(!a.try_acquire_at(now), "a is exhausted");
        assert!(
            b.try_acquire_at(now),
            "b must be entirely unaffected by a's exhaustion — no shared state"
        );
    }
}
