//! Unit tests for `error_map.rs`.

use super::*;

mod seam_strip;
mod wiring_guard;

// ── map_completion_transport_error (is_timeout() → Timeout/Network) ────────
//
// The load-bearing classification the timeout-diagnostics chain this batch
// exists for depends on: is_timeout() → AppError::Timeout →
// StoppedReason::Timeout → the banner telling the user a stage timed out and
// to try a faster model. If this helper silently reclassified a real timeout
// to Network, that whole chain reverts to the original bug (a spinner with no
// explanation) — and a mutation test against a prior, pre-extraction version
// of this helper (forced to always return Network) found NOTHING red
// anywhere in the workspace, which is exactly the gap this test closes.
//
// `reqwest::Error` has no public constructor (see `anthropic/tests/list_models.rs`'s
// `reqwest_is_timeout_fires_for_the_clients_own_deadline_and_never_for_a_connect_failure`,
// which pins the underlying `is_timeout()` assumption this helper relies on
// but never drives the classification itself), so both branches below are
// built from REAL local connections rather than a mock: a socket that
// accepts but never writes back is a genuine client-side timeout; a loopback
// port nothing is bound to refuses the connection immediately and is a
// genuine non-timeout transport error.
//
// `refused_connection_error` (below) binds an ephemeral loopback port, drops
// it to free it, then immediately connects to the same port and expects a
// refusal — but this file and `anthropic/tests/list_models.rs` both run that exact
// pattern in the SAME test binary, in parallel by default, so a sibling test
// can reclaim the just-freed port before this one reconnects. A bounded
// retry (fresh port each attempt) rather than a single `expect_err` keeps
// that flake from being misread as a `map_completion_transport_error`
// regression instead of a shared, unrelated test seam.
async fn refused_connection_error() -> reqwest::Error {
    for _ in 0..5 {
        let refused_addr = {
            let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            probe.local_addr().unwrap()
            // `probe` drops here — the port is released with nothing bound to it.
        };
        match crate::net::http::shared()
            .get(format!("http://{refused_addr}"))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Err(e) => return e,
            // Another test's listener claimed the freed port in the gap
            // between drop and connect — retry with a fresh one rather than
            // let an accidental success pass (or panic for the wrong reason).
            Ok(_) => continue,
        }
    }
    panic!("could not observe a refused loopback connection after 5 attempts");
}

#[tokio::test]
async fn map_completion_transport_error_classifies_a_real_timeout_and_a_real_connect_failure() {
    // A REAL timeout: the server accepts the connection but writes nothing
    // back before the client's own short deadline elapses.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((socket, _)) = listener.accept().await {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            drop(socket);
        }
    });
    let timed_out = crate::net::http::shared()
        .get(format!("http://{addr}"))
        .timeout(std::time::Duration::from_millis(50))
        .send()
        .await
        .expect_err("a 50ms deadline against a silent server must fail");
    assert!(timed_out.is_timeout(), "precondition: {timed_out}");

    match map_completion_transport_error(
        timed_out,
        "Anthropic",
        std::time::Duration::from_secs(120),
    ) {
        AppError::Timeout(msg) => {
            assert!(msg.contains("Anthropic"), "must name the provider: {msg}");
            assert!(msg.contains("120"), "must carry the deadline: {msg}");
        }
        other => panic!("a real is_timeout() error must map to AppError::Timeout, got {other:?}"),
    }

    // A REAL non-timeout transport failure: a loopback port nothing is bound
    // to refuses the connection immediately (no firewall/DNS involved).
    let refused = refused_connection_error().await;
    assert!(!refused.is_timeout(), "precondition: {refused}");

    let mapped =
        map_completion_transport_error(refused, "Anthropic", std::time::Duration::from_secs(120));
    assert!(
        matches!(mapped, AppError::Network(_)),
        "a non-timeout transport error must map to AppError::Network, got {mapped:?}"
    );
}

// ── redact_stream_error_message (generation-failure privacy boundary) ──────
//
// `emit_stream_error` is the ONE place every generation failure (`ai_generate`
// + `generate_pipeline`) funnels through before the renderer shows the text
// verbatim (`TailorFlow`'s `ErrorState description={gen.error}`). These pin
// the #935 shape (query-string auth in a base_url) and the path-privacy rule,
// AND the property that stops someone "fixing" this by flattening every
// message to a generic string.

#[test]
fn redact_stream_error_message_scrubs_query_string_auth_in_a_url() {
    // The #935 shape: a user-supplied base_url carrying its API key in the
    // query string, echoed into a network/provider error.
    let msg =
        "error sending request to https://gw.example.com/v1?api-key=SECRET123: connection reset";
    let redacted = redact_stream_error_message(msg);
    assert!(
        !redacted.contains("SECRET123"),
        "credential must not survive: {redacted}"
    );
    assert!(
        !redacted.contains("gw.example.com"),
        "host must not survive: {redacted}"
    );
    assert!(
        redacted.contains("<url-redacted>"),
        "expected the url placeholder; got: {redacted}"
    );
    // MUTATION GUARD: a no-op redactor (`message.to_string()`) would leave the
    // secret in place — this assertion only passes when redaction actually ran.
    assert_ne!(redacted, msg);
}

#[test]
fn redact_stream_error_message_scrubs_an_absolute_filesystem_path() {
    // Path-privacy: a filesystem error (e.g. a local CLI-agent adapter, or a
    // storage failure surfaced through the same `AppError::to_string()` path)
    // must never leak an absolute path with the user's name in it.
    let msg = r"failed to read C:\Users\alice\AppData\Local\ajh\config.json: access denied";
    let redacted = redact_stream_error_message(msg);
    assert!(
        !redacted.contains("alice"),
        "username must not survive: {redacted}"
    );
    assert!(
        redacted.contains("<path-redacted>"),
        "expected the path placeholder; got: {redacted}"
    );
}

#[test]
fn redact_stream_error_message_leaves_an_ordinary_provider_error_unchanged() {
    // The assertion that stops a later "fix" from flattening every message to
    // a generic string: an ordinary provider error carries no credential/
    // path/host/email shape and must survive BYTE-FOR-BYTE, exactly as
    // `friendly_api_error` built it.
    for msg in [
        "openai: rate limit or quota reached. Wait a moment or check your plan.",
        "429 Too Many Requests",
        "anthropic: model or endpoint not found — model not found",
        "Ollama unreachable: connection refused",
    ] {
        let redacted = redact_stream_error_message(msg);
        // MUTATION GUARD: an over-eager redactor (e.g. collapsing every
        // message to a fixed string, or stripping digits/punctuation) fails
        // this exact-equality check — only a targeted, shape-based redactor
        // passes.
        assert_eq!(redacted, msg, "an ordinary message must pass through as-is");
    }
}

// ── redact_provider_error (model-list / key-probe → settings UI) ───────────

#[test]
fn redact_provider_error_strips_a_key_echoed_by_the_upstream_body() {
    let body = r#"{"error":{"message":"bad request for Authorization: Bearer sk-TESTKEY123456 at https://gw.example.com/v1/models?key=TESTKEY123456"}}"#;
    // `friendly_api_error` now redacts at the source (#1346), so build the raw
    // upstream-shaped error by hand to keep this exercising the edge pass alone.
    let err = AppError::Provider(format!(
        "openai: request rejected — {}",
        extract_error_message(body)
    ));
    // Precondition: the unredacted text really does carry the key.
    assert!(err.to_string().contains("sk-TESTKEY123456"));
    let text = redact_provider_error(err, &[]).to_string();
    assert!(!text.contains("TESTKEY"), "key survived: {text}");
    assert!(!text.contains("gw.example.com"), "host survived: {text}");
}

#[test]
fn redact_provider_error_bounds_length_and_keeps_an_ordinary_message() {
    let long = redact_provider_error(AppError::Provider("x".repeat(5000)), &[]).to_string();
    assert!(long.chars().count() <= 201, "unbounded: {}", long.len());
    let msg = "openai: invalid or unauthorized API key.";
    assert_eq!(
        redact_provider_error(AppError::Config(msg.into()), &[]).to_string(),
        msg
    );
}

#[test]
fn redact_provider_error_strips_known_secrets_verbatim_and_skips_short_ones() {
    let key = "AIzaSyTESTKEYabcdefghijklmnop";
    let e = AppError::Provider(format!("x-goog-api-key: {key} rejected; pw hunter22, id 7"));
    let text = redact_provider_error(e, &[key, "", "7", "hunter22"]).to_string();
    assert!(!text.contains("TESTKEY"), "bare key survived: {text}");
    assert!(!text.contains("hunter22"), "secret survived: {text}");
    assert!(
        text.contains("id 7"),
        "short needle must be skipped: {text}"
    );
}

#[test]
fn finish_provider_result_collects_key_and_base_url_secrets_and_passes_ok_through() {
    let e =
        AppError::Provider("bad AIzaSyTESTKEYabcdefghijklmnop and gwsecret99 and pass12345".into());
    let res: AppResult<()> = Err(e);
    let out = finish_provider_result(
        res,
        Some("  AIzaSyTESTKEYabcdefghijklmnop "),
        Some("https://u:pass12345@gw.example.com/v1?token=gwsecret99"),
    )
    .unwrap_err()
    .to_string();
    for s in ["TESTKEY", "gwsecret99", "pass12345"] {
        assert!(!out.contains(s), "{s} survived: {out}");
    }
    assert_eq!(finish_provider_result(Ok(3), Some("k"), None).unwrap(), 3);
}

// ── source-level redaction in friendly_api_error (#1346) ───────────────────

fn detail_of(status: u16, body: &str) -> String {
    friendly_api_error(
        ProviderId::OpenAi,
        reqwest::StatusCode::from_u16(status).unwrap(),
        body,
    )
    .to_string()
}

#[test]
fn friendly_api_error_redacts_the_upstream_detail_at_the_source() {
    let body = r#"{"error":{"message":"bad Authorization: Bearer sk-TESTKEY123456 at https://gw.example.com/v1/models?key=AIzaSyTESTKEYabcdefghijklmnop"}}"#;
    // Precondition: the raw extracted detail really carries both secrets.
    let raw = extract_error_message(body);
    assert!(raw.contains("sk-TESTKEY123456") && raw.contains("AIzaSyTESTKEY"));
    for status in [400, 404, 422, 418] {
        let text = detail_of(status, body);
        assert!(!text.contains("TESTKEY"), "{status}: key survived: {text}");
        assert!(
            !text.contains("gw.example.com"),
            "{status}: host survived: {text}"
        );
    }
}

#[test]
fn friendly_api_error_leaves_ordinary_text_byte_identical() {
    assert_eq!(
        detail_of(400, r#"{"error":{"message":"model not found"}}"#),
        "openai: request rejected — model not found"
    );
    assert_eq!(
        detail_of(404, "plain body, no json"),
        "openai: model or endpoint not found — plain body, no json"
    );
}

#[test]
fn upstream_text_is_bounded_without_panicking_on_a_multibyte_char_at_the_ceiling() {
    // 3-byte char straddling the ceiling: every offset mod 3 is exercised.
    for pad in 0..3 {
        let body = format!(
            "{}{}",
            "a".repeat(MAX_UPSTREAM_TEXT_BYTES - 1 - pad),
            "€".repeat(50)
        );
        let out = redact_upstream_text(&body);
        assert!(
            out.len() <= MAX_UPSTREAM_TEXT_BYTES,
            "unbounded: {}",
            out.len()
        );
    }
    // Hostile JSON detail, far past the ceiling, through the real entry point.
    let huge = format!(r#"{{"error":{{"message":"{}"}}}}"#, "x ".repeat(2_000_000));
    let text = detail_of(400, &huge);
    assert!(
        text.len() < MAX_UPSTREAM_TEXT_BYTES + 100,
        "unbounded: {}",
        text.len()
    );
}

#[test]
fn redact_body_for_log_redacts_and_caps() {
    let body = format!(
        "denied Authorization: Bearer sk-TESTKEY123456 {}",
        "word ".repeat(5000)
    );
    let out = redact_body_for_log(&body);
    assert!(!out.contains("TESTKEY"), "key survived: {out}");
    assert!(out.chars().count() <= crate::observability::MAX_REASON_LEN + 1);
    assert_eq!(
        redact_body_for_log("429 Too Many Requests"),
        "429 Too Many Requests"
    );
}

#[test]
fn edge_helper_strips_a_bare_stored_key_the_shape_pass_cannot_see() {
    // No marker, no known prefix: only the verbatim pass can catch it.
    let key = "AIzaSyTESTKEYabcdefghijklmnop";
    let body = format!(r#"{{"error":{{"message":"invalid credential {key} supplied"}}}}"#);
    let err = friendly_api_error(ProviderId::Gemini, reqwest::StatusCode::BAD_REQUEST, &body);
    assert!(
        err.to_string().contains(key),
        "precondition: shape pass keeps it"
    );
    let out = finish_provider_result::<()>(Err(err), Some(key), None)
        .unwrap_err()
        .to_string();
    assert!(!out.contains("TESTKEY"), "bare key survived: {out}");
}
