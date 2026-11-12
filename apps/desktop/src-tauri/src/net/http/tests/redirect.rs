use super::*;

// ── is_allowed_redirect_target: fleet-wide redirect SSRF guard ─────────────
// Hermetic — pure URL parsing + the `net::ssrf` classifier, no network.

fn url(s: &str) -> reqwest::Url {
    reqwest::Url::parse(s).unwrap()
}

/// Redirect decision for a request that started on a public host.
fn allowed(target: &reqwest::Url) -> bool {
    is_allowed_redirect(&[url("https://example.com/")], target)
}

fn allowed_chain(chain: &[&str], target: &str) -> bool {
    let prev: Vec<_> = chain.iter().map(|u| url(u)).collect();
    is_allowed_redirect(&prev, &url(target))
}

fn allowed_from(origin: &str, target: &str) -> bool {
    allowed_chain(&[origin], target)
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
        "https://LOCALHOST.:1234/"
    ));
    assert!(allowed_chain(
        &["http://192.168.1.50/", "https://192.168.1.50/a"],
        "https://192.168.1.50/b"
    ));
}

#[test]
fn allowed_redirect_refuses_other_local_host_even_in_same_class() {
    assert!(!allowed_from("http://192.168.1.50/", "http://10.0.0.2/"));
    assert!(!allowed_from("http://127.0.0.1/", "http://localhost/"));
    assert!(!allowed_from("http://127.0.0.1/", "http://192.168.1.1/"));
    assert!(!allowed_from("http://192.168.1.1/", "http://127.0.0.1/"));
}

#[test]
fn allowed_redirect_refuses_public_to_private() {
    for t in [
        "http://127.0.0.1/",
        "http://192.168.1.1/",
        "http://[::ffff:10.0.0.1]/",
    ] {
        assert!(!allowed_from("https://example.com/", t), "{t}");
    }
}

#[test]
fn allowed_redirect_refuses_trailing_dot_loopback_names() {
    for t in [
        "http://localhost.:8080/",
        "http://foo.localhost.:8080/",
        "http://LOCALHOST./",
        "http://x.local./",
    ] {
        assert!(!allowed_from("http://example.com/", t), "{t}");
    }
}

#[test]
fn allowed_redirect_refuses_chain_pivot_through_public_hop() {
    assert!(!allowed_chain(
        &["http://192.168.1.50/", "https://example.com/"],
        "http://192.168.1.50/"
    ));
    assert!(!allowed_chain(
        &["http://127.0.0.1/", "http://example.com/"],
        "http://127.0.0.1/x"
    ));
}

#[test]
fn allowed_redirect_refuses_unspecified_targets() {
    for t in [
        "http://0.0.0.0/",
        "http://[::]/",
        "http://[::ffff:0.0.0.0]/",
    ] {
        assert!(!allowed_from("https://example.com/", t), "{t}");
        assert!(!allowed_from(t, t), "{t} same-host");
    }
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
    // judged against the immediately previous hop, not the origin
    assert!(!allowed_chain(
        &["http://a.example.org/", "https://example.com/"],
        "http://example.com/x"
    ));
    assert!(allowed_chain(
        &["https://a.example.org/", "http://example.com/"],
        "http://example.com/x"
    ));
    assert!(allowed_from("http://example.com/", "https://example.com/x"));
    assert!(allowed_from("http://example.com/", "http://example.com/x"));
    assert!(allowed_from(
        "https://example.com/",
        "https://example.com/x"
    ));
    // local same-host https -> http is a cleartext downgrade too
    assert!(!allowed_from(
        "https://127.0.0.1:8080/",
        "http://127.0.0.1:8080/"
    ));
    assert!(!allowed_from(
        "https://192.168.1.50/",
        "http://192.168.1.50/"
    ));
    assert!(allowed_from(
        "http://192.168.1.50/",
        "https://192.168.1.50/"
    ));
    assert!(allowed_from(
        "http://192.168.1.50/",
        "http://192.168.1.50/x"
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
        "http://100.100.100.200/",
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
