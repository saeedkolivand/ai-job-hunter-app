use super::*;

// ── parse_profile: the strictness gate ────────────────────────────────────

fn public_profile_html(name: &str) -> String {
    format!(
        r#"<html><head>
            <script type="application/ld+json">
            {{
                "@type": "Person",
                "name": "{name}",
                "jobTitle": "Senior Engineer at Acme",
                "description": "Building great software.",
                "address": {{ "addressLocality": "Berlin, Germany" }},
                "knowsAbout": ["Rust", "TypeScript"]
            }}
            </script>
        </head><body></body></html>"#
    )
}

#[test]
fn parses_a_real_public_profile() {
    let profile = parse_profile(&public_profile_html("Jane Doe")).unwrap();
    assert_eq!(profile.name.as_deref(), Some("Jane Doe"));
    assert_eq!(profile.headline.as_deref(), Some("Senior Engineer at Acme"));
    assert_eq!(profile.location.as_deref(), Some("Berlin, Germany"));
    assert_eq!(profile.skills, vec!["Rust", "TypeScript"]);
}

#[test]
fn rejects_pages_without_a_non_empty_ld_json_person_name() {
    for html in [
        // No `ld+json` Person block at all — only an og:title, the way LinkedIn's
        // login/authwall page looks. Must NOT fall back to og:title for `name`
        // (that was the "successful import of nothing" regression).
        r#"<html><head>
        <meta property="og:title" content="LinkedIn Login, Sign in | LinkedIn">
    </head><body></body></html>"#,
        // Person block present but `name` is an empty string — must not pass the
        // gate just because the field exists.
        r#"<html><head>
        <script type="application/ld+json">
        { "@type": "Person", "name": "" }
        </script>
    </head><body></body></html>"#,
    ] {
        let err = parse_profile(html).unwrap_err();
        assert!(
            matches!(err, AppError::Parse(_)),
            "expected Parse, got {err:?}"
        );
    }
}

#[test]
fn falls_back_to_og_title_for_headline_only() {
    // Headline fallback to og:title is still fine — only `name` is gated on
    // ld+json.
    let html = r#"<html><head>
        <script type="application/ld+json">
        { "@type": "Person", "name": "Jane Doe" }
        </script>
        <meta property="og:title" content="Jane Doe - Senior Engineer | LinkedIn">
    </head><body></body></html>"#;
    let profile = parse_profile(html).unwrap();
    assert_eq!(profile.name.as_deref(), Some("Jane Doe"));
    assert_eq!(profile.headline.as_deref(), Some("Jane Doe"));
}

// ── map_status: the honest-error-mapping that comes with fetching anon ────

#[test]
fn map_status_429_is_rate_limited() {
    assert!(matches!(map_status(429), AppError::RateLimited(_)));
}

#[test]
fn map_status_other_non_2xx_is_provider_not_auth_advice() {
    for status in [403, 404, 500, 999] {
        let err = map_status(status);
        assert!(
            matches!(err, AppError::Provider(_)),
            "status {status} should map to Provider, got {err:?}"
        );
        // Regression guard: the message must never tell the user to log in —
        // we fetch anonymously on purpose, so that advice would be actively
        // wrong (see `fetch_page`).
        let msg = err.to_string().to_lowercase();
        assert!(
            !msg.contains("log in") && !msg.contains("sign in"),
            "status {status} message must not suggest logging in: {msg:?}"
        );
    }
}

// ── fetch_page: the fetch path must be SSRF-guarded ───────────────────────
//
// `url` is user-pasted, attacker-influenced input, so `fetch_page` must
// route it through `net::http::get_guarded_following_redirects` (IP
// validation before connecting) rather than the plain pooled `shared()`
// client. Asserting only `Err(_)` here would pass for the wrong reason —
// an unguarded client hitting an unlistened loopback port also errors
// (connection refused). Instead this test proves the stronger property: a
// REAL, listening loopback socket never receives a connection attempt at
// all, which only a pre-connect SSRF rejection (not a failed connect)
// explains.

#[tokio::test]
#[serial_test::serial]
async fn fetch_page_never_dials_the_loopback_literal_it_rejects() {
    // Scope the data dir to an empty temp dir so has_linkedin_session()'s
    // real-disk read never touches the developer's or CI runner's actual data
    // dir. The guard lives in platform/config.rs because that module is the
    // sole owner of that env var — this file never names it, which
    // is what R4 is actually asking for. It restores on drop, so a panicking
    // assertion below cannot leak the override into the next test.
    // `#[serial]` is still required: the variable is process-global and the
    // resolver's own test mutates it directly.
    let tmp = tempfile::TempDir::new().expect("create temp data dir");
    let _data_dir = crate::platform::config::DataDirGuard::set(tmp.path());

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
    listener
        .set_nonblocking(true)
        .expect("set listener non-blocking");
    let port = listener.local_addr().unwrap().port();

    let result = fetch_page(&format!("http://127.0.0.1:{port}/in/x")).await;

    let err = result.unwrap_err();
    assert!(matches!(err, AppError::Network(_)), "got {err:?}");

    // Poll briefly for a connection a guarded fetch must never make. The
    // guarded rejection is a synchronous, pre-network check, so under
    // correct code this returns false almost immediately; 300ms is ample
    // margin without making the test slow.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(300);
    let mut connected = false;
    while std::time::Instant::now() < deadline {
        if listener.accept().is_ok() {
            connected = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        !connected,
        "fetch_page dialed the rejected loopback socket — the SSRF guard was bypassed"
    );
}
