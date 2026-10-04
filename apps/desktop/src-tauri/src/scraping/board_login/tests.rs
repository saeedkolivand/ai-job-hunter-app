use super::*;

#[test]
fn test_get_config_linkedin() {
    let config = get_config("linkedin");
    assert!(config.is_some());
    assert_eq!(config.unwrap().id, "linkedin");
}

#[test]
fn test_get_config_invalid() {
    let config = get_config("invalid_board");
    assert!(config.is_none());
}

#[test]
fn test_get_config_indeed() {
    let config = get_config("indeed");
    assert!(config.is_some());
    assert_eq!(config.unwrap().display_name, "Indeed");
}

#[test]
fn test_default_is_authed_url_login() {
    assert!(!default_is_authed_url("https://example.com/login"));
}

#[test]
fn test_default_is_authed_url_auth() {
    assert!(!default_is_authed_url("https://example.com/auth"));
}

#[test]
fn test_default_is_authed_url_normal() {
    assert!(default_is_authed_url("https://example.com/dashboard"));
}

#[test]
fn test_default_is_authed_url_checkpoint() {
    assert!(!default_is_authed_url("https://example.com/checkpoint"));
}

#[test]
fn test_board_state_dir() {
    let dir = board_state_dir(Path::new("/tmp"), "linkedin");
    assert!(dir.ends_with("browser-state/linkedin"));
}

#[test]
fn test_profile_dir() {
    let dir = profile_dir(Path::new("/tmp"), "linkedin");
    assert!(dir.ends_with("browser-state/linkedin/profile"));
}

#[test]
fn test_cookies_path() {
    let path = cookies_path(Path::new("/tmp"), "linkedin");
    assert!(path.ends_with("browser-state/linkedin/cookies.json"));
}

#[test]
fn test_auth_status_path() {
    let path = auth_status_path(Path::new("/tmp"), "linkedin");
    assert!(path.ends_with("browser-state/linkedin/auth-status.json"));
}

#[test]
fn test_session_max_age_constant() {
    assert_eq!(SESSION_MAX_AGE_MS, 7 * 24 * 60 * 60 * 1000);
}

#[test]
fn test_stored_cookie_serialization() {
    let cookie = StoredCookie {
        name: "test".to_string(),
        value: "value".to_string(),
        domain: "example.com".to_string(),
        path: "/".to_string(),
        expires: Some(1234567890.0),
        http_only: true,
        secure: true,
    };
    let json = serde_json::to_string(&cookie);
    assert!(json.is_ok());
}

#[test]
fn test_stored_cookie_deserialization() {
    let json = r#"{"name":"test","value":"value","domain":"example.com","path":"/","expires":1234567890.0,"http_only":true,"secure":true}"#;
    let cookie: StoredCookie = serde_json::from_str(json).unwrap();
    assert_eq!(cookie.name, "test");
    assert_eq!(cookie.domain, "example.com");
}

#[test]
fn test_auth_status_serialization() {
    let status = AuthStatus {
        connected: true,
        connected_at: Some(1234567890),
    };
    let json = serde_json::to_string(&status);
    assert!(json.is_ok());
}

#[test]
fn test_get_config_xing() {
    let config = get_config("xing");
    assert!(config.is_some());
    assert_eq!(config.unwrap().id, "xing");
}

#[test]
fn test_get_config_glassdoor() {
    let config = get_config("glassdoor");
    assert!(config.is_some());
    assert_eq!(config.unwrap().id, "glassdoor");
}

/// epoch-0 connected_at → age > SESSION_MAX_AGE_MS → session_is_stale returns true.
/// This covers the stale branch of the engine's skip predicate.
#[test]
fn stale_session_is_detected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();
    let board = "stale-test-board";

    // Write a cookie so load_cookies is non-empty (skip fires on staleness, not absence).
    let cookie = StoredCookie {
        name: "sess".into(),
        value: "tok".into(),
        domain: "example.com".into(),
        path: "/".into(),
        expires: None,
        http_only: false,
        secure: false,
    };
    write_cookies(data_dir, board, &[cookie]).expect("write_cookies");

    // connected_at = 0 → age = now_ms → always > 7 days in ms.
    write_auth_status(data_dir, board, true);
    // Overwrite with epoch-0 connected_at to force staleness.
    let apath = auth_status_path(data_dir, board);
    std::fs::write(&apath, r#"{"connected":true,"connected_at":0}"#)
        .expect("overwrite auth-status");

    assert!(
        !load_cookies(data_dir, board).is_empty(),
        "cookies must be non-empty"
    );
    let age = session_age_ms(data_dir, board).expect("session_age_ms must be Some");
    assert!(
        age > SESSION_MAX_AGE_MS,
        "epoch-0 session age ({age} ms) must exceed SESSION_MAX_AGE_MS ({SESSION_MAX_AGE_MS} ms)"
    );
    assert!(
        session_is_stale(data_dir, board),
        "session_is_stale must return true for epoch-0 connected_at"
    );
}

/// Fresh connected_at (now) → age < SESSION_MAX_AGE_MS → session_is_stale returns false.
#[test]
fn fresh_session_is_not_stale() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();
    let board = "fresh-test-board";

    let cookie = StoredCookie {
        name: "sess".into(),
        value: "tok".into(),
        domain: "example.com".into(),
        path: "/".into(),
        expires: None,
        http_only: false,
        secure: false,
    };
    write_cookies(data_dir, board, &[cookie]).expect("write_cookies");
    write_auth_status(data_dir, board, true); // connected_at = now

    assert!(
        !load_cookies(data_dir, board).is_empty(),
        "cookies must be non-empty"
    );
    assert!(
        !session_is_stale(data_dir, board),
        "fresh session must not be stale"
    );
}

#[test]
fn test_default_is_authed_url_signin() {
    assert!(!default_is_authed_url("https://example.com/signin"));
}

#[test]
fn test_default_is_authed_url_uas() {
    assert!(!default_is_authed_url("https://example.com/uas/something"));
}

#[test]
fn test_default_is_authed_url_with_query() {
    assert!(default_is_authed_url(
        "https://example.com/dashboard?param=value"
    ));
}

#[test]
fn test_default_is_authed_url_with_fragment() {
    assert!(default_is_authed_url(
        "https://example.com/dashboard#section"
    ));
}

#[test]
fn test_stored_cookie_defaults() {
    let cookie = StoredCookie {
        name: "test".to_string(),
        value: "value".to_string(),
        domain: "example.com".to_string(),
        path: "/".to_string(),
        expires: None,
        http_only: false,
        secure: false,
    };
    assert!(!cookie.http_only);
    assert!(!cookie.secure);
    assert!(cookie.expires.is_none());
}

#[test]
fn test_auth_status_defaults() {
    let status = AuthStatus {
        connected: false,
        connected_at: None,
    };
    assert!(!status.connected);
    assert!(status.connected_at.is_none());
}

#[test]
fn test_board_login_config_copy() {
    let config = get_config("linkedin").unwrap();
    let copied = *config;
    assert_eq!(copied.id, "linkedin");
    assert_eq!(copied.display_name, "LinkedIn");
}

// ── build_cookie_jar: Domain-attribute fidelity ────────────────────────────
//
// RFC 6265 §5.3: a cookie added with no Domain attribute is host-only, bound
// to the exact host it was added against. These assert the rebuilt jar
// honours each stored cookie's own scope (leading-dot domain vs host-only)
// instead of collapsing every cookie to host-only.

fn stored_cookie(name: &str, domain: &str) -> StoredCookie {
    StoredCookie {
        name: name.to_string(),
        value: "tok".to_string(),
        domain: domain.to_string(),
        path: "/".to_string(),
        expires: None,
        http_only: true,
        secure: true,
    }
}

/// `.linkedin.com` (leading dot = captured with an explicit Domain attribute)
/// must reach both the apex and any subdomain — this is the bug being fixed.
#[test]
fn domain_cookie_reaches_apex_and_subdomain() {
    use reqwest::cookie::CookieStore;
    let jar = build_cookie_jar(&[stored_cookie("li_at", ".linkedin.com")]);

    let apex = reqwest::Url::parse("https://linkedin.com/").unwrap();
    let sub = reqwest::Url::parse("https://www.linkedin.com/").unwrap();

    let apex_header = jar.cookies(&apex).expect("must be sent to the apex domain");
    assert!(apex_header.to_str().unwrap().contains("li_at=tok"));
    let sub_header = jar
        .cookies(&sub)
        .expect("must be sent to a subdomain (the bug this fixes)");
    assert!(sub_header.to_str().unwrap().contains("li_at=tok"));
}

/// `linkedin.com` (no leading dot = host-only when captured) must stay bound
/// to exactly that host and NOT leak to a sibling subdomain.
#[test]
fn host_only_cookie_stays_host_only() {
    use reqwest::cookie::CookieStore;
    let jar = build_cookie_jar(&[stored_cookie("sess", "linkedin.com")]);

    let exact = reqwest::Url::parse("https://linkedin.com/").unwrap();
    let sub = reqwest::Url::parse("https://www.linkedin.com/").unwrap();

    assert!(
        jar.cookies(&exact).is_some(),
        "must still be sent to its exact host"
    );
    assert!(
        jar.cookies(&sub).is_none(),
        "must NOT widen to a subdomain it wasn't issued for"
    );
}

/// Safety property: regardless of stored scope, the cookie must never reach
/// an unrelated host. Regression guard — fails if scope is ever widened
/// beyond what the stored cookie's own Domain says.
#[test]
fn cookie_never_sent_to_unrelated_host() {
    use reqwest::cookie::CookieStore;
    let jar = build_cookie_jar(&[
        stored_cookie("li_at", ".linkedin.com"),
        stored_cookie("sess", "linkedin.com"),
    ]);

    let unrelated = reqwest::Url::parse("https://evil.example/").unwrap();
    assert!(
        jar.cookies(&unrelated).is_none(),
        "cookie must never reach a host it wasn't issued for"
    );
}
