//! `is_authed_cookies` predicate tests for every board config.

// ── HIGH 1: is_authed_cookies predicate ──────────────────────────────────────

fn make_cookie(
    name: &str,
    value: &str,
    domain: &str,
) -> crate::scraping::board_login::StoredCookie {
    crate::scraping::board_login::StoredCookie {
        name: name.to_owned(),
        value: value.to_owned(),
        domain: domain.to_owned(),
        path: "/".to_owned(),
        expires: None,
        http_only: false,
        secure: true,
    }
}

/// linkedin: a valid `li_at` (value.len() > 10, domain contains "linkedin.com") → true.
#[test]
fn is_authed_cookies_linkedin_valid_li_at_returns_true() {
    let predicate = crate::scraping::board_login::get_config("linkedin")
        .unwrap()
        .is_authed_cookies
        .unwrap();

    let cookies = vec![make_cookie("li_at", "a_valid_token_1234", ".linkedin.com")];
    assert!(
        predicate(&cookies),
        "valid li_at with linkedin.com domain must return true"
    );
}

/// linkedin: cookies present but NO `li_at` → false.
#[test]
fn is_authed_cookies_linkedin_no_li_at_returns_false() {
    let predicate = crate::scraping::board_login::get_config("linkedin")
        .unwrap()
        .is_authed_cookies
        .unwrap();

    let cookies = vec![
        make_cookie("JSESSIONID", "some_session_value_xyz", ".linkedin.com"),
        make_cookie("bscookie", "another_cookie_value_abc", ".linkedin.com"),
    ];
    assert!(
        !predicate(&cookies),
        "absent li_at must return false even with other linkedin cookies"
    );
}

/// linkedin: `li_at` present but value too short (len <= 10) → false.
#[test]
fn is_authed_cookies_linkedin_li_at_too_short_returns_false() {
    let predicate = crate::scraping::board_login::get_config("linkedin")
        .unwrap()
        .is_authed_cookies
        .unwrap();

    // Exactly 10 chars — the condition is `value.len() > 10`, so this must fail.
    let cookies = vec![make_cookie("li_at", "0123456789", ".linkedin.com")];
    assert!(
        !predicate(&cookies),
        "li_at value.len() == 10 must return false (condition is > 10, not >= 10)"
    );

    // 5 chars — also too short.
    let short = vec![make_cookie("li_at", "short", ".linkedin.com")];
    assert!(
        !predicate(&short),
        "li_at value.len() < 10 must return false"
    );
}

/// linkedin: `li_at` present but wrong domain → false.
#[test]
fn is_authed_cookies_linkedin_wrong_domain_returns_false() {
    let predicate = crate::scraping::board_login::get_config("linkedin")
        .unwrap()
        .is_authed_cookies
        .unwrap();

    // Value is long enough but domain does not contain "linkedin.com".
    let cookies = vec![make_cookie(
        "li_at",
        "a_valid_token_1234",
        ".evil-linkedin.io",
    )];
    assert!(
        !predicate(&cookies),
        "li_at with wrong domain must return false"
    );
}

/// indeed / xing / glassdoor: `is_authed_cookies` must be `None` — documents the
/// "any non-empty cookie set → Imported" None-arm policy in `import_cookies`.
#[test]
fn is_authed_cookies_is_none_for_indeed_xing_glassdoor() {
    for board_id in ["indeed", "xing", "glassdoor"] {
        let cfg = crate::scraping::board_login::get_config(board_id)
            .unwrap_or_else(|| panic!("get_config({board_id}) must return Some"));
        assert!(
            cfg.is_authed_cookies.is_none(),
            "is_authed_cookies for {board_id} must be None (any non-empty cookie set → Imported)"
        );
    }
}
