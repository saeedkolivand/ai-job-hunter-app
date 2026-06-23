use crate::autopilot::AutopilotTarget;
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, SCRAPE_ITEM, SCRAPE_PROGRESS};
use crate::scraping::{BoardScrapeSummary, BoardSearchInput, JobPosting, ScraperEngine};
use tauri::AppHandle;

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
        country_code: target.country_code.clone(),
        latitude: None,
        longitude: None,
        radius_km: None,
        // Autopilot has no per-company target; ATS company slugs are a manual
        // search affordance, so this stays empty (a no-op for every board).
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
fn redact_token(token: &str) -> String {
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
    let is_credential = [
        "app_key=",
        "app_id=",
        "apikey=",
        "api_key=",
        "key=",
        "secret=",
        "token=",
        "password=",
        "pwd=",
        "auth=",
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

    if is_url {
        token.replace(trimmed, "<url-redacted>")
    } else if is_credential {
        token.replace(trimmed, "<credential-redacted>")
    } else if is_windows_path || is_unix_path || is_homeish_path {
        token.replace(trimmed, "<path-redacted>")
    } else if is_host_port {
        token.replace(trimmed, "<host-redacted>")
    } else {
        token.to_string()
    }
}

/// Turn the per-board scrape summaries into a single human-readable reason string
/// explaining why a run may have come up short — `"<board>: <error>"` for each
/// board that errored or was skipped, joined with `"; "`. The per-board reason is
/// run through [`sanitize_reason`] so absolute paths / URLs never leak into the
/// user-visible step log. Returns an empty string when no board reported a
/// problem. Pure + unit-testable.
pub(crate) fn scrape_diagnostics(summaries: &[BoardScrapeSummary]) -> String {
    summaries
        .iter()
        .filter_map(|s| {
            s.error
                .as_deref()
                .or(s.skipped.as_deref())
                .map(|reason| format!("{}: {}", s.board, sanitize_reason(reason)))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scraping::BoardScrapeSummary;

    fn summary(board: &str, error: Option<&str>, skipped: Option<&str>) -> BoardScrapeSummary {
        BoardScrapeSummary {
            board: board.into(),
            count: 0,
            error: error.map(String::from),
            skipped: skipped.map(String::from),
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
}
