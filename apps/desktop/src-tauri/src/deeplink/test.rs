use super::*;

fn argv(s: &str) -> Vec<String> {
    // Mimic a real launch: exe path first, then the URL the OS appends.
    vec!["C:/app/ajh.exe".to_string(), s.to_string()]
}

#[test]
fn accepts_a_valid_autopilot_url() {
    assert_eq!(
        parse_focus_target(&argv("ajh://autopilot/job-1a2b3c")),
        Some(FocusTarget::Autopilot("job-1a2b3c".to_string()))
    );
    // uuid-shaped id
    assert_eq!(
        parse_focus_target(&argv(
            "ajh://autopilot/550e8400-e29b-41d4-a716-446655440000"
        )),
        Some(FocusTarget::Autopilot(
            "550e8400-e29b-41d4-a716-446655440000".to_string()
        ))
    );
}

#[test]
fn rejects_unknown_scheme_or_action() {
    assert_eq!(parse_focus_target(&argv("https://autopilot/x")), None);
    assert_eq!(parse_focus_target(&argv("ajhx://autopilot/x")), None);
    assert_eq!(parse_focus_target(&argv("AJH://autopilot/x")), None); // case-sensitive
    assert_eq!(parse_focus_target(&argv("ajh://settings/wipe")), None);
    assert_eq!(parse_focus_target(&argv("ajh://privacy/reset")), None);
}

#[test]
fn accepts_the_extension_pairing_url() {
    assert_eq!(
        parse_focus_target(&argv("ajh://settings/extension")),
        Some(FocusTarget::ExtensionPairing)
    );
}

#[test]
fn rejects_other_settings_shapes() {
    // Only `settings/extension` is allowlisted — every other settings sub-page,
    // extra segment, query/fragment, or case variant stays denied.
    assert_eq!(parse_focus_target(&argv("ajh://settings/wipe")), None);
    assert_eq!(
        parse_focus_target(&argv("ajh://settings/extension/x")),
        None // extra segment
    );
    assert_eq!(
        parse_focus_target(&argv("ajh://settings/extension?x=1")),
        None // query
    );
    assert_eq!(
        parse_focus_target(&argv("ajh://settings/extension#frag")),
        None // fragment
    );
    assert_eq!(parse_focus_target(&argv("ajh://settings")), None); // no id segment
    assert_eq!(parse_focus_target(&argv("ajh://settings/")), None); // empty id
    assert_eq!(
        parse_focus_target(&argv("ajh://settings/Extension")),
        None // case-sensitive id
    );
}

#[test]
fn rejects_path_traversal_and_injection_shapes() {
    assert_eq!(
        parse_focus_target(&argv("ajh://autopilot/../../settings")),
        None
    );
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/a/b")), None); // extra segment
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/")), None); // empty id
    assert_eq!(parse_focus_target(&argv("ajh://autopilot")), None); // no id segment
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/id?x=1")), None); // query
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/id#frag")), None); // fragment
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/a b")), None); // space
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/a\\b")), None); // backslash
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://autopilot/{}", "x".repeat(65)))),
        None
    );
}

#[test]
fn ignores_normal_launch_argv() {
    assert_eq!(parse_focus_target(&["C:/app/ajh.exe".to_string()]), None);
    assert_eq!(parse_focus_target(&[]), None);
}

#[test]
fn finds_the_url_among_other_args() {
    let v = vec![
        "C:/app/ajh.exe".to_string(),
        "--flag".to_string(),
        "ajh://autopilot/abc".to_string(),
    ];
    assert_eq!(
        parse_focus_target(&v),
        Some(FocusTarget::Autopilot("abc".to_string()))
    );
}

// ── PR2 — `ajh://generate?url=…` / `ajh://open?url=…` ───────────────────────

#[test]
fn accepts_a_valid_generate_for_job_url() {
    // Assert the FULL canonical url, not just the variant + a domain substring: the renderer
    // uses `url` as its lookup key, so a regression that truncates the path or returns a fixed
    // url (but still contains "example.com") must fail this test.
    let input = "https://example.com/job/1?ref=abc";
    let encoded = urlencoding::encode(input);
    let target = parse_focus_target(&argv(&format!("ajh://generate?url={encoded}")));
    assert_eq!(
        target,
        Some(FocusTarget::GenerateForJob(
            crate::applications::normalize_job_url(input)
        ))
    );
}

#[test]
fn accepts_a_valid_open_job_url() {
    let input = "https://example.com/job/2";
    let encoded = urlencoding::encode(input);
    let target = parse_focus_target(&argv(&format!("ajh://open?url={encoded}")));
    assert_eq!(
        target,
        Some(FocusTarget::OpenJob(
            crate::applications::normalize_job_url(input)
        ))
    );
}

#[test]
fn accepts_a_valid_prep_for_job_url() {
    let input = "https://example.com/job/3";
    let encoded = urlencoding::encode(input);
    let target = parse_focus_target(&argv(&format!("ajh://prep?url={encoded}")));
    assert_eq!(
        target,
        Some(FocusTarget::PrepForJob(
            crate::applications::normalize_job_url(input)
        ))
    );
}

#[test]
fn rejects_a_bare_scheme_with_no_authority() {
    // A scheme with nothing after it must not parse into a target — `normalize_job_url("https://")`
    // returns the non-empty literal `"https://"`, which the renderer would otherwise treat as a
    // real search/lookup url.
    for bare in ["https://", "http://"] {
        let encoded = urlencoding::encode(bare);
        assert_eq!(
            parse_focus_target(&argv(&format!("ajh://generate?url={encoded}"))),
            None,
            "bare = {bare:?}"
        );
        assert_eq!(
            parse_focus_target(&argv(&format!("ajh://open?url={encoded}"))),
            None,
            "bare = {bare:?}"
        );
        assert_eq!(
            parse_focus_target(&argv(&format!("ajh://prep?url={encoded}"))),
            None,
            "bare = {bare:?}"
        );
    }
}

#[test]
fn rejects_a_non_http_job_url() {
    let encoded = urlencoding::encode("javascript:alert(1)");
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://generate?url={encoded}"))),
        None
    );
    let encoded_ftp = urlencoding::encode("ftp://example.com/x");
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://open?url={encoded_ftp}"))),
        None
    );
    let encoded_js = urlencoding::encode("javascript:alert(1)");
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://prep?url={encoded_js}"))),
        None
    );
}

#[test]
fn rejects_an_oversized_job_url() {
    let long = format!("https://example.com/{}", "x".repeat(2100));
    let encoded = urlencoding::encode(&long);
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://generate?url={encoded}"))),
        None
    );
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://prep?url={encoded}"))),
        None
    );
}

#[test]
fn rejects_a_missing_or_malformed_url_param() {
    assert_eq!(parse_focus_target(&argv("ajh://generate")), None);
    assert_eq!(parse_focus_target(&argv("ajh://generate?")), None);
    assert_eq!(parse_focus_target(&argv("ajh://generate?url=")), None);
    assert_eq!(parse_focus_target(&argv("ajh://open?foo=bar")), None);
    assert_eq!(parse_focus_target(&argv("ajh://prep")), None);
    assert_eq!(parse_focus_target(&argv("ajh://prep?url=")), None);
    // A second query param is rejected outright rather than silently ignored.
    let encoded = urlencoding::encode("https://example.com/job/1");
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://generate?url={encoded}&x=1"))),
        None
    );
    // Unknown action with the same query shape.
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://bogus?url={encoded}"))),
        None
    );
}

// ── #1237 — Windows inserts one trailing slash: `ajh://open/?url=…` ─────────

#[test]
fn accepts_one_trailing_slash_after_the_action() {
    // Windows protocol activation appends a `/` after the action when the URL's
    // path is empty, so `ajh://open/?url=…` IS the argv a relaunch actually
    // arrives with — it must parse exactly like the canonical `ajh://open?url=…`
    // (#1237).
    let cases: [(&str, fn(String) -> FocusTarget); 3] = [
        ("generate", FocusTarget::GenerateForJob),
        ("open", FocusTarget::OpenJob),
        ("prep", FocusTarget::PrepForJob),
    ];
    for (action, make_target) in cases {
        let input = "https://example.com/job/1?ref=abc";
        let encoded = urlencoding::encode(input);
        let target = parse_focus_target(&argv(&format!("ajh://{action}/?url={encoded}")));
        assert_eq!(
            target,
            Some(make_target(crate::applications::normalize_job_url(input))),
            "action = {action:?}"
        );
    }
    // The canonical no-slash form still parses (regression guard for #1237).
    let encoded = urlencoding::encode("https://example.com/job/1");
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://open?url={encoded}"))),
        Some(FocusTarget::OpenJob(
            crate::applications::normalize_job_url("https://example.com/job/1")
        ))
    );
}

#[test]
fn rejects_more_than_one_trailing_slash_or_a_real_path_segment() {
    // Only ONE optional trailing slash is accepted: `ajh://open//?url=…` (two
    // slashes) and `ajh://open/x?url=…` (a real extra path segment) both still
    // parse to `None` (#1237).
    let encoded = urlencoding::encode("https://example.com/job/1");
    for action in ["generate", "open", "prep"] {
        assert_eq!(
            parse_focus_target(&argv(&format!("ajh://{action}//?url={encoded}"))),
            None,
            "two trailing slashes, action = {action:?}"
        );
        assert_eq!(
            parse_focus_target(&argv(&format!("ajh://{action}/x?url={encoded}"))),
            None,
            "extra path segment, action = {action:?}"
        );
    }
    // A non-allowlisted action with the same query shape stays denied.
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://opened?url={encoded}"))),
        None
    );
}

// ── log sanitiser (`sanitize_action_for_log`) ───────────────────────────────

#[test]
fn sanitize_action_for_log_collapses_control_chars_and_newlines() {
    // A hostile argv can carry control characters / newlines straight into the
    // action segment (`ajh://open\x00?url=…`) — the log line must never receive
    // them: the run collapses to a single `?`.
    assert_eq!(
        sanitize_action_for_log("open\x00\n\u{1b}evil"),
        Some("open?evil".to_string())
    );
    // Allowlisted separators survive untouched.
    assert_eq!(
        sanitize_action_for_log("auto_pilot-1"),
        Some("auto_pilot-1".to_string())
    );
    // Nothing safe at all → no log line.
    assert_eq!(sanitize_action_for_log("\x00\n\x01"), None);
    assert_eq!(sanitize_action_for_log(""), None);
}

#[test]
fn sanitize_action_for_log_caps_an_over_long_action_at_32_chars() {
    let long = format!("open{}", "x".repeat(1000));
    let logged = sanitize_action_for_log(&long).expect("the safe prefix must survive");
    assert_eq!(logged.len(), 32);
    assert_eq!(logged, format!("open{}", "x".repeat(28)));
}
