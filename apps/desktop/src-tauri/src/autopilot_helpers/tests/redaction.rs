//! `sanitize_reason` / `redact_token`: nothing path-, URL-, host-, email- or credential-shaped
//! may survive into a user-visible step log or a log line.

use crate::observability::{redact_token, sanitize_reason, MAX_REASON_LEN};

#[test]
fn sanitize_reason_caps_overlong_input() {
    let long = "x".repeat(500);
    let out = sanitize_reason(&long);
    assert!(
        out.chars().count() <= MAX_REASON_LEN + 1, // +1 for the ellipsis
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
fn board_prefixed_posting_id_embedding_a_url_is_redacted() {
    // `documents::posting_vector_or_embed`'s failed-upsert log line redacts
    // `job_id` with `redact_token` (not `sanitize_reason` — a posting id is
    // one token, never whitespace-split prose). Most boards build an opaque
    // `board:external-id` (e.g. `"greenhouse:12345"`, left untouched below),
    // but `breezy`/`pinpoint`/`themuse` build theirs as
    // `format!("{BOARD_ID}:{url}")`, embedding the posting's full URL — this
    // pins that shape actually redacts, closing the CodeRabbit-flagged gap.
    let url_embedding_id = "pinpoint:https://boards.example.com/jobs/42?ref=abc";
    let out = redact_token(url_embedding_id);
    assert!(
        out.contains("<url-redacted>"),
        "a board:url-shaped posting id must be redacted; got: {out}"
    );
    assert!(
        !out.contains("boards.example.com"),
        "host must not survive: {out}"
    );

    // The common case — an opaque board:external-id — must pass through
    // unchanged, or the fix would cost every OTHER board's log
    // debuggability to close a leak only these three boards have.
    assert_eq!(redact_token("greenhouse:12345"), "greenhouse:12345");
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

// ── H3: multi-token credential shapes (Authorization/Bearer, spaced `=`,
//        bare-prefixed secret with no marker at all) ──────────────────────

#[test]
fn authorization_bearer_token_is_redacted() {
    for (raw, secret) in [
        (
            "request failed: Authorization: Bearer ghp_deadbeef12345 rejected",
            "ghp_deadbeef12345",
        ),
        ("Authorization: Basic dXNlcjpwYXNz denied", "dXNlcjpwYXNz"),
    ] {
        let out = sanitize_reason(raw);
        assert!(
            out.contains("<credential-redacted>"),
            "credential placeholder must appear for {raw:?}; got: {out}"
        );
        assert!(
            !out.contains(secret),
            "secret value leaked from {raw:?}; got: {out}"
        );
    }
}

#[test]
fn authorization_scheme_word_alone_is_not_a_marker() {
    // `Bearer`/`Basic` only redact the NEXT token when they directly follow an
    // `Authorization:` marker — bare prose using either word must survive.
    assert_eq!(sanitize_reason("Basic auth failed"), "Basic auth failed");
    assert_eq!(
        sanitize_reason("Bearer token missing from the request"),
        "Bearer token missing from the request"
    );
}

#[test]
fn spaced_credential_assignment_is_redacted() {
    // `redact_token` alone only catches the GLUED `token=value` shape (see
    // `redact_standalone_credential_assignments`); a SPACED assignment needs
    // the two-token lookahead in `redact_tokens`.
    for (raw, secret) in [
        ("token = abc123xyz rejected", "abc123xyz"),
        ("secret = s3cr3t-val leaked", "s3cr3t-val"),
    ] {
        let out = sanitize_reason(raw);
        assert!(
            out.contains("<credential-redacted>"),
            "credential placeholder must appear for {raw:?}; got: {out}"
        );
        assert!(
            !out.contains(secret),
            "secret value leaked from {raw:?}; got: {out}"
        );
    }
}

#[test]
fn bare_prefixed_secret_is_redacted_with_no_marker_at_all() {
    // A known credential-token PREFIX (GitHub PAT, OpenAI/Anthropic key, AWS
    // access key id) is unambiguous by shape alone — no `Authorization:`,
    // `Bearer`, or `=` needed to flag it.
    for raw in [
        "board rejected ghp_1234567890abcdef with 401",
        "provider call failed: sk-ant-abc123xyz",
        "leaked AKIAIOSFODNN7EXAMPLE in the response body",
    ] {
        let out = sanitize_reason(raw);
        assert!(
            out.contains("<credential-redacted>"),
            "bare secret must be redacted for {raw:?}; got: {out}"
        );
    }
}

#[test]
fn ordinary_log_prose_with_credential_lookalike_words_survives_untouched() {
    // Mutation guard for the new marker chains above: none of these must
    // trip — no `Authorization:` immediately before Bearer/Basic, and no
    // marker word immediately followed by a literal `=`.
    for raw in [
        "invalid auth token for board, please check your Basic auth settings",
        "Basic auth failed",
        "missing password field in the request",
        "token refresh needed before retry",
    ] {
        assert_eq!(sanitize_reason(raw), raw, "benign message altered: {raw:?}");
    }
}
