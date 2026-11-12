use super::*;
use crate::error::AppError;

// IP-literal unsafe URLs must be rejected WITHOUT any network/DNS — the
// literal path is hermetic. One table: cloud metadata, IPv4 loopback, RFC-1918
// private, IPv6 loopback, then a non-http scheme.
#[tokio::test]
async fn get_guarded_rejects_unsafe_literals_and_non_http_schemes() {
    for url in [
        "http://169.254.169.254/",
        "http://127.0.0.1/",
        "http://10.0.0.1/",
        "http://[::1]/",
        "ftp://1.1.1.1/",
    ] {
        let err = get_guarded(url).await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }
}

// ── get_guarded: literal-branch validation actually runs ──────────────────

// A private IP literal must be rejected by get_guarded's OWN validation
// (not merely by the ssrf classifier in isolation) — this exercises the
// literal branch's `is_safe_ip` gate and the Validation error it returns.
#[tokio::test]
async fn get_guarded_private_literal_returns_validation_error() {
    let err = get_guarded("http://192.168.1.1/job").await.unwrap_err();
    assert!(
        matches!(err, AppError::Validation(_)),
        "private IP literal must be a Validation error, got {err:?}"
    );
}

// A public IP literal passes get_guarded's validation step. We use TEST-NET-3
// (203.0.113.0/24, RFC 5737 — documentation/example range, guaranteed
// unroutable) so the connect cannot reach a real host; whatever the send
// resolves to, the ONE thing we assert is that get_guarded did NOT reject it
// at the validation gate. A subsequent connection/timeout error is expected
// and acceptable — it proves we got *past* validation.
#[tokio::test]
async fn get_guarded_public_literal_passes_validation() {
    let result = get_guarded("http://203.0.113.1/job").await;
    if let Err(AppError::Validation(msg)) = &result {
        panic!("public IP literal must pass validation, but was rejected: {msg}");
    }
    // Ok(_) (unlikely against TEST-NET-3) or a non-Validation transport error
    // both confirm validation was passed.
}

// ── validate_resolved_addrs: the hostname-branch security core ─────────────
// This is the post-`lookup_host` gate that closes the DNS-rebinding TOCTOU on
// the hostname path. Tested hermetically with synthetic resolved sets — no
// real DNS — so the security logic is asserted independent of the network.

fn sa(s: &str) -> std::net::SocketAddr {
    s.parse().unwrap()
}

#[test]
fn validate_resolved_addrs_rejects_empty_set() {
    let err = validate_resolved_addrs(&[]).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
}

#[test]
fn validate_resolved_addrs_rejects_any_private_addr() {
    // A host that resolves to a public AND a private address (a classic
    // rebinding payload) must be rejected because ANY unsafe addr fails.
    let addrs = [sa("1.1.1.1:80"), sa("192.168.1.10:80")];
    let err = validate_resolved_addrs(&addrs).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
}

#[test]
fn validate_resolved_addrs_rejects_loopback_addr() {
    let err = validate_resolved_addrs(&[sa("127.0.0.1:80")]).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
}

#[test]
fn validate_resolved_addrs_rejects_metadata_addr() {
    // 169.254.169.254 — cloud metadata endpoint reached via a rebinding host.
    let err = validate_resolved_addrs(&[sa("169.254.169.254:80")]).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
}

#[test]
fn validate_resolved_addrs_accepts_all_public_set() {
    let addrs = [
        sa("1.1.1.1:443"),
        sa("8.8.8.8:443"),
        sa("[2606:4700:4700::1111]:443"),
    ];
    assert!(
        validate_resolved_addrs(&addrs).is_ok(),
        "an all-public resolved set must pass"
    );
}

// The redirect-follower must apply get_guarded's IP validation on the FIRST
// hop too — an unsafe literal is rejected before any redirect could be followed.
#[tokio::test]
async fn following_redirects_rejects_unsafe_first_hop() {
    let err = get_guarded_following_redirects("http://127.0.0.1/", 5)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
}

// ── is_allowed_redirect_target: fleet-wide redirect SSRF guard ─────────────
// Hermetic — pure URL parsing + the `net::ssrf` classifier, no network.

fn url(s: &str) -> reqwest::Url {
    reqwest::Url::parse(s).unwrap()
}

/// Redirect decision for a request that started on a public host.
fn allowed(target: &reqwest::Url) -> bool {
    is_allowed_redirect(&url("https://example.com/"), target)
}

fn allowed_from(origin: &str, target: &str) -> bool {
    is_allowed_redirect(&url(origin), &url(target))
}

#[test]
fn allowed_redirect_same_host_scheme_upgrade_on_local_hosts() {
    assert!(allowed_from(
        "http://127.0.0.1:11434/",
        "https://127.0.0.1:11434/"
    ));
    assert!(allowed_from(
        "http://192.168.1.50/",
        "https://192.168.1.50/x"
    ));
    assert!(allowed_from(
        "http://localhost:1234/",
        "https://localhost:1234/"
    ));
}

#[test]
fn allowed_redirect_within_same_private_class() {
    assert!(allowed_from("http://192.168.1.50/", "http://10.0.0.2/"));
    assert!(allowed_from("http://127.0.0.1/", "http://localhost/"));
}

#[test]
fn allowed_redirect_refuses_public_to_private_and_cross_class() {
    for t in [
        "http://127.0.0.1/",
        "http://192.168.1.1/",
        "http://[::ffff:10.0.0.1]/",
    ] {
        assert!(!allowed_from("https://example.com/", t), "{t}");
    }
    assert!(!allowed_from("http://127.0.0.1/", "http://192.168.1.1/"));
    assert!(!allowed_from("http://192.168.1.1/", "http://127.0.0.1/"));
}

#[test]
fn allowed_redirect_refuses_https_to_http_downgrade_on_public_host() {
    assert!(!allowed_from(
        "https://example.com/",
        "http://example.com/x"
    ));
    assert!(!allowed_from(
        "https://example.com/",
        "http://other.example.org/"
    ));
    assert!(allowed_from("http://example.com/", "https://example.com/x"));
    assert!(allowed_from("http://example.com/", "http://example.com/x"));
    assert!(allowed_from(
        "https://example.com/",
        "https://example.com/x"
    ));
    // local same-host https -> http is fine
    assert!(allowed_from(
        "https://127.0.0.1:8080/",
        "http://127.0.0.1:8080/"
    ));
}

#[test]
fn allowed_redirect_link_local_always_blocked() {
    for t in [
        "http://169.254.169.254/",
        "http://169.254.170.2/",
        "http://[::ffff:169.254.169.254]/",
        "http://[fe80::1]/",
        "http://[fd00:ec2::254]/",
    ] {
        assert!(!allowed_from("https://example.com/", t), "{t}");
        // even from the same host / a link-local origin
        assert!(!allowed_from(t, t), "{t} same-host");
    }
}

#[test]
fn allowed_redirect_accepts_public_https() {
    assert!(allowed(&url("https://boards.greenhouse.io/acme/jobs/1")));
}

#[test]
fn allowed_redirect_rejects_loopback_literal() {
    assert!(!allowed(&url("http://127.0.0.1/steal")));
    assert!(!allowed(&url("http://[::1]/steal")));
}

#[test]
fn allowed_redirect_rejects_cloud_metadata_link_local() {
    assert!(!allowed(&url("http://169.254.169.254/latest/meta-data/")));
}

#[test]
fn allowed_redirect_rejects_rfc1918_private_ranges() {
    for u in [
        "http://10.0.0.1/",
        "http://172.16.0.1/",
        "http://192.168.1.1/",
    ] {
        assert!(!allowed(&url(u)), "{u} must be blocked");
    }
}

#[test]
fn allowed_redirect_rejects_ipv6_unique_local() {
    assert!(!allowed(&url("http://[fc00::1]/")));
}

#[test]
fn allowed_redirect_rejects_non_http_scheme() {
    assert!(!allowed(&url("file:///etc/passwd")));
    assert!(!allowed(&url("ftp://files.example.com/x")));
}

#[test]
fn allowed_redirect_allows_arbitrary_public_hostname() {
    // A public-looking hostname is allowed here even though it could still
    // resolve to a private IP via DNS rebinding — this synchronous policy
    // cannot resolve DNS. See the `redirect_policy` doc comment.
    assert!(allowed(&url("https://example.com/x")));
}

// ── redirect_policy: actually wired onto the pooled client ─────────────────
// `reqwest::redirect::Attempt` has no public constructor, so the decision is
// unit-tested via `is_allowed_redirect`; these round-trips prove the policy is
// the one `shared()` really uses. The loopback mock server is the ORIGIN, so a
// same-host hop must be followed (200) while a hop to link-local must fail with
// our block message (not a connect error: that message is what we assert on).
async fn redirect_via_shared(
    location: Option<&str>,
) -> (wiremock::MockServer, reqwest::Result<reqwest::Response>) {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let location = location.map_or_else(|| format!("{}/landed", server.uri()), str::to_string);
    Mock::given(method("GET"))
        .and(path("/landed"))
        .respond_with(ResponseTemplate::new(200).set_body_string("landed"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", location.as_str()))
        .mount(&server)
        .await;
    let res = shared().get(format!("{}/start", server.uri())).send().await;
    (server, res)
}

#[tokio::test]
async fn shared_client_follows_same_host_redirect_on_loopback() {
    let (_s, res) = redirect_via_shared(None).await;
    assert_eq!(res.expect("same-host hop must be followed").status(), 200);
}

#[tokio::test]
async fn shared_client_blocks_redirect_to_link_local_target() {
    let (_s, res) = redirect_via_shared(Some("http://169.254.169.254/landed")).await;
    let debug_msg = format!("{:?}", res.expect_err("link-local hop must be blocked"));
    assert!(
        debug_msg.contains("blocked redirect"),
        "must be the redirect-policy block, got: {debug_msg}"
    );
}

// ── read_text_capped / read_bytes_capped / read_json_capped ────────────────
// Moved here from `scraping::http` (this is now the fleet-wide chokepoint).

#[tokio::test]
async fn read_text_capped_returns_the_body_under_the_cap() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Hello World"))
        .mount(&mock_server)
        .await;

    let response = reqwest::get(mock_server.uri()).await.unwrap();
    assert_eq!(
        read_text_capped(response, 1024).await.unwrap(),
        "Hello World"
    );
}

#[tokio::test]
async fn read_bytes_capped_returns_the_body_under_the_cap() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2, 3, 4]))
        .mount(&mock_server)
        .await;

    let response = reqwest::get(mock_server.uri()).await.unwrap();
    let bytes = read_bytes_capped(response, 64).await.unwrap();
    assert_eq!(bytes, vec![1, 2, 3, 4]);
}

/// The former `read_text_capped`/`read_bytes_capped` "rejects over the cap"
/// tests each fed a SINGLE 4096-byte chunk against a 64-byte cap — the
/// first (and only) chunk already exceeded the cap on its own, so the
/// assertion passed identically against a wrong `if chunk.len() > cap`
/// guard and never actually exercised the running-total accumulation
/// (`buf.len().saturating_add(chunk.len()) > cap`). `read_text_capped` and
/// `read_bytes_capped` now both delegate their entire size guard to
/// [`accumulate_capped`], so testing it once here covers both — driven
/// directly with a synthetic multi-chunk stream (no network, no
/// `reqwest::Response` construction needed) so the chunk boundaries are
/// deterministic rather than at the mercy of OS/TCP fragmentation.
#[tokio::test]
async fn accumulate_capped_rejects_when_running_total_exceeds_cap_across_multiple_chunks() {
    // Three 40-byte chunks: no SINGLE chunk exceeds the 100-byte cap, so a
    // guard changed to a per-chunk check (`chunk.len() > cap`) would let
    // all 120 bytes through untouched. Only the running-total guard
    // (`buf.len().saturating_add(chunk.len()) > cap`) rejects this — it
    // fires while accumulating the 3rd chunk (40 + 40 + 40 = 120 > 100).
    let chunks: Vec<reqwest::Result<Vec<u8>>> =
        vec![Ok(vec![0u8; 40]), Ok(vec![0u8; 40]), Ok(vec![0u8; 40])];
    let stream = futures::stream::iter(chunks);

    let err = accumulate_capped(stream, None, 100).await.expect_err(
        "120 bytes across 3 chunks of 40 must be rejected even though no single \
             chunk exceeds the 100-byte cap",
    );
    assert!(
        matches!(&err, AppError::Validation(msg) if msg.contains("too large")),
        "expected a size Validation error, got: {err:?}"
    );
}

/// Mirrors `scraping::http::tests::fetch_basic::test_fetch_json_invalid` — a body that
/// doesn't deserialize into the target type returns the static no-leak
/// `AppError::Parse` message, never the serde detail or the body itself.
#[tokio::test]
async fn read_json_capped_returns_generic_error_on_parse_failure() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&mock_server)
        .await;

    let response = reqwest::get(mock_server.uri()).await.unwrap();
    let err = read_json_capped::<serde_json::Value>(response, DEFAULT_MAX_BODY_BYTES)
        .await
        .expect_err("invalid json must fail to parse");
    match err {
        AppError::Parse(msg) => {
            assert_eq!(msg, "response body did not match the expected schema");
        }
        other => panic!("expected a Parse error on schema drift, got {other:?}"),
    }
}
