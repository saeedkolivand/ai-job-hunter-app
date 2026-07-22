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
    async fn complete(&self, _system: &str, _user: &str, _temperature: f64) -> AppResult<String> {
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
        cluster_id: None,
        cluster_canonical: true,
        cluster_members: Vec::new(),
        is_agency: false,
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
    let prior: HashSet<String> = HashSet::new();
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
async fn jobs_already_seen_make_zero_calls_even_under_new_tracking_params() {
    // Every match already surfaced in a prior run → the merge preserves its
    // earlier note for free; re-generating would just burn a call for nothing.
    let env = FakeNoteEnv::ok("note");
    // Re-surfaced under new tracking params: same job to the merge.
    let mut jobs = vec![
        stub_job("https://acme.example/1?utm_source=indeed"),
        stub_job("https://acme.example/2#apply"),
    ];
    let prior: HashSet<String> = ["https://acme.example/1", "https://acme.example/2"]
        .into_iter()
        .map(|u| crate::scraping::boards::common::canonical_job_key(u, "Engineer", "Acme"))
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
async fn duplicate_url_variants_within_one_run_pay_for_only_one_note() {
    // The same NEW job surfaced twice in ONE run under different URL variants
    // (same canonical key). It used to buy a note for EACH, then
    // `merge_found_jobs` collapsed them and discarded one. Only the first
    // variant must pay; the second is skipped.
    let env = FakeNoteEnv::ok("note");
    let mut jobs = vec![
        stub_job("https://acme.example/1?utm_source=indeed"),
        stub_job("https://acme.example/1#apply"),
    ];
    let prior: HashSet<String> = HashSet::new();
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
        "only one note for the two variants of one job"
    );
    assert_eq!(
        *env.calls.lock(),
        1,
        "the duplicate variant makes no provider call"
    );
    assert!(jobs[0].assistant_notes.is_some());
    assert!(
        jobs[1].assistant_notes.is_none(),
        "the second variant is skipped, not annotated"
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
    let prior: HashSet<String> = HashSet::new();
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
    let prior: HashSet<String> = HashSet::new();
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

// ── watched-companies resolution (ADR-030 §e) ──────────────────────────────

fn watched(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(a, s)| (a.to_string(), s.to_string()))
        .collect()
}

fn boards(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[test]
fn watched_resolution_maps_each_ats_to_its_own_slugs() {
    // Stars for greenhouse + ashby, both boards selected → each board maps to
    // ONLY its own ATS's slugs (no cross-ATS mixing).
    let map = resolve_watched_companies(
        &watched(&[
            ("greenhouse", "stripe"),
            ("ashby", "Linear"),
            ("greenhouse", "airbnb"),
        ]),
        &boards(&["greenhouse", "ashby"]),
    );
    assert_eq!(
        map.get("greenhouse"),
        Some(&vec!["stripe".to_string(), "airbnb".to_string()])
    );
    assert_eq!(map.get("ashby"), Some(&vec!["Linear".to_string()]));
}

#[test]
fn watched_resolution_dedups_within_an_ats_preserving_order() {
    let map = resolve_watched_companies(
        &watched(&[("greenhouse", "acme"), ("greenhouse", "acme")]),
        &boards(&["greenhouse"]),
    );
    assert_eq!(map.get("greenhouse"), Some(&vec!["acme".to_string()]));
}

#[test]
fn watched_resolution_omits_boards_with_no_matching_star() {
    // A lever star and a greenhouse star, but the run selected greenhouse+ashby:
    // greenhouse maps to its slug; ashby is ABSENT (skipped by the engine); the
    // lever star is irrelevant (not a selected board).
    let map = resolve_watched_companies(
        &watched(&[("lever", "spotify"), ("greenhouse", "stripe")]),
        &boards(&["greenhouse", "ashby"]),
    );
    assert_eq!(map.get("greenhouse"), Some(&vec!["stripe".to_string()]));
    assert!(!map.contains_key("ashby"), "ashby has no star → absent");
    assert!(!map.contains_key("lever"), "lever isn't a selected board");
}

#[test]
fn watched_resolution_empty_star_set_yields_empty_map() {
    // Flag on but nothing starred → empty map → the engine skips every
    // company-scoped board `needs-company` (never the curated seed).
    let map = resolve_watched_companies(&[], &boards(&["greenhouse", "ashby"]));
    assert!(map.is_empty());
}

/// Store→resolver seam (ADR-030 §e): stars in a REAL `DiscoveredCompanyStore`
/// (incl. one starred cold → materialized `source='seed'` row) resolve — via the
/// same `store.watched()` + registry `requires_company` filter `autopilot_scrape`
/// composes — into a per-board override map. The missing link between the store
/// tests (start from tuples) and the pure-resolver tests (hand-built `watched()`).
#[test]
fn watched_stars_resolve_from_the_store_into_per_board_targets() {
    use crate::discovered::DiscoveredCompanyStore;
    let dir = tempfile::TempDir::new().unwrap();
    let store = DiscoveredCompanyStore::open(dir.path()).unwrap();

    let name = Some("Stripe".to_string());
    store
        .upsert_batch(&[("greenhouse".into(), "stripe".into(), name, "scrape".into())])
        .unwrap();
    store.set_starred("greenhouse", "stripe", true).unwrap();
    store.set_starred("ashby", "Linear", true).unwrap(); // cold star → seed row
    store.set_starred("lever", "spotify", true).unwrap(); // board not selected below

    // Filter the selected boards to company-scoped ones via the SAME predicate.
    let selected: Vec<String> = ["greenhouse", "ashby", "linkedin"]
        .iter()
        .filter(|b| crate::scraping::boards::get(b).is_some_and(|s| s.requires_company()))
        .map(|s| s.to_string())
        .collect();

    let map = resolve_watched_companies(&store.watched(), &selected);

    assert_eq!(map.get("greenhouse"), Some(&vec!["stripe".to_string()]));
    // A cold-starred (materialized-seed) company still routes to its own board.
    assert_eq!(map.get("ashby"), Some(&vec!["Linear".to_string()]));
    // Non-company board filtered out; the unselected board's star is irrelevant.
    assert!(!map.contains_key("linkedin"));
    assert!(!map.contains_key("lever"));
}
