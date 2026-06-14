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
